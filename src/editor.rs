//! Editor state and input handling. Rendering lives in ui.rs.

use crate::buffer::{Buffer, KIND_DELETE, KIND_OTHER, KIND_TYPE};
use crate::fileio;
use crate::syntax::{self, Lang, State, Tok};
use crate::ui::{all_themes, Theme};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use unicode_width::UnicodeWidthChar;

pub const TAB: usize = 4;

// ------------------------------------------------------------------ helpers

/// Invisible / direction-changing characters used in "Trojan Source" attacks.
/// They are drawn as a red `?` so they can never hide in code.
pub fn suspicious(c: char) -> bool {
    matches!(c,
        '\u{200B}' | '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{FEFF}'
        | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}')
}

/// Width of a character on screen when it starts at display column `col`.
pub fn cell_width(c: char, col: usize) -> usize {
    if c == '\t' {
        TAB - col % TAB
    } else if suspicious(c) {
        1
    } else if c.is_control() {
        2 // drawn as ^X
    } else {
        UnicodeWidthChar::width(c).unwrap_or(0)
    }
}

pub fn disp_col(line: &[char], cx: usize) -> usize {
    let mut col = 0;
    for &c in line.iter().take(cx) {
        col += cell_width(c, col);
    }
    col
}

pub fn cx_from_col(line: &[char], target: usize) -> usize {
    let mut col = 0;
    for (i, &c) in line.iter().enumerate() {
        let w = cell_width(c, col);
        if target < col + w.max(1) {
            return i;
        }
        col += w;
    }
    line.len()
}

/// Make arbitrary text safe to print in the status line (no escape sequences).
pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_control() || suspicious(c) { '?' } else { c }).collect()
}

fn class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

fn word_left(line: &[char], cx: usize) -> usize {
    let mut i = cx.min(line.len());
    while i > 0 && line[i - 1].is_whitespace() {
        i -= 1;
    }
    if i > 0 {
        let k = class(line[i - 1]);
        while i > 0 && class(line[i - 1]) == k {
            i -= 1;
        }
    }
    i
}

fn word_right(line: &[char], cx: usize) -> usize {
    let n = line.len();
    let mut i = cx.min(n);
    if i < n {
        let k = class(line[i]);
        while i < n && class(line[i]) == k {
            i += 1;
        }
    }
    while i < n && line[i].is_whitespace() {
        i += 1;
    }
    i
}

fn lc(c: char) -> char {
    if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

fn smart_ci(q: &[char]) -> bool {
    !q.iter().any(|c| c.is_uppercase())
}

fn match_at(line: &[char], x: usize, q: &[char], ci: bool) -> bool {
    if x + q.len() > line.len() {
        return false;
    }
    q.iter().zip(&line[x..]).all(|(a, b)| a == b || (ci && lc(*a) == lc(*b)))
}

pub fn find_fwd(line: &[char], q: &[char], from: usize, ci: bool) -> Option<usize> {
    if q.is_empty() {
        return None;
    }
    let mut x = from;
    while x + q.len() <= line.len() {
        if match_at(line, x, q, ci) {
            return Some(x);
        }
        x += 1;
    }
    None
}

/// Last match whose start is strictly before `before`.
fn find_back(line: &[char], q: &[char], before: usize, ci: bool) -> Option<usize> {
    if q.is_empty() || line.len() < q.len() || before == 0 {
        return None;
    }
    let mut x = (line.len() - q.len()).min(before - 1);
    loop {
        if match_at(line, x, q, ci) {
            return Some(x);
        }
        if x == 0 {
            return None;
        }
        x -= 1;
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() / 3 * 4 + 4);
    for ch in data.chunks(3) {
        let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if ch.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if ch.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    s
}

/// Pasted text may come from anywhere: drop control characters (terminal
/// escape injection), normalise line endings.
fn clean_paste(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut it = t.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push('\n');
            }
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

// ------------------------------------------------------------------ types

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumberMode {
    Off,
    Absolute,
    Relative,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    SaveAs,
    Find,
    ReplaceFrom,
    ReplaceTo,
    Goto,
    ConfirmQuit,
    ConfirmOverwrite,
}

pub struct Prompt {
    pub kind: PromptKind,
    pub label: String,
    pub input: Vec<char>,
    fresh: bool, // first typed char replaces the prefilled text
}

#[derive(Default)]
pub struct Find {
    pub query: Vec<char>,
    pub show: bool,
    origin: (usize, usize),
}

pub struct Editor {
    pub(crate) buf: Buffer,
    pub(crate) path: Option<PathBuf>,
    pub(crate) cy: usize,
    pub(crate) cx: usize,
    want_x: usize,
    pub(crate) top: usize,
    pub(crate) left: usize,
    pub(crate) sel: Option<(usize, usize)>,
    drag: Option<(usize, usize)>,
    last_click: Option<(Instant, usize, usize, u8)>,
    clip: String,
    clip_line: bool,
    osc52_enabled: bool,
    pub(crate) osc52: Option<String>,
    pub(crate) msg: String,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) find: Find,
    replace_from: Vec<char>,
    pending_path: Option<PathBuf>,
    pub(crate) lang: &'static Lang,
    states: Vec<State>,
    pub(crate) tok: Vec<Tok>,
    pub(crate) tok_cache: Vec<Option<Vec<Tok>>>,
    pub(crate) crlf: bool,
    final_newline: bool,
    bom: bool,
    mtime: Option<SystemTime>,
    pub(crate) number_mode: NumberMode,
    auto_indent: bool,
    pub(crate) quit: bool,
    quit_after_save: bool,
    pub(crate) help: bool,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) theme: Theme,
    pub(crate) theme_idx: usize,
    pub(crate) bracket_match_pos: Option<(usize, usize)>,
    pub(crate) scrollbar: bool,
}

impl Editor {
    pub fn new(path: Option<PathBuf>, max_bytes: u64) -> Result<Editor, String> {
        let loaded = match &path {
            Some(p) => fileio::load(p, max_bytes)
                .map_err(|e| format!("{}: {}", sanitize(&p.to_string_lossy()), e))?,
            None => fileio::Loaded::empty(),
        };
        let hidden = loaded.lines.iter().any(|l| l.iter().any(|&c| suspicious(c)));
        let msg = if hidden {
            "WARNING: file contains invisible/bidirectional Unicode characters (shown as red ?)".to_string()
        } else if path.is_some() && !loaded.existed {
            "New file".to_string()
        } else if !loaded.writable {
            "File is read-only: saving will fail".to_string()
        } else {
            String::new()
        };
        let lang = syntax::detect(path.as_deref());
        let themes = all_themes();
        let theme = themes.into_iter().next().unwrap();
        Ok(Editor {
            buf: Buffer::new(loaded.lines),
            path,
            cy: 0,
            cx: 0,
            want_x: 0,
            top: 0,
            left: 0,
            sel: None,
            drag: None,
            last_click: None,
            clip: String::new(),
            clip_line: false,
            osc52_enabled: std::env::var_os("LEKH_NO_OSC52").is_none(),
            osc52: None,
            msg,
            prompt: None,
            find: Find::default(),
            replace_from: Vec::new(),
            pending_path: None,
            lang,
            states: vec![State::default()],
            tok: Vec::new(),
            tok_cache: Vec::new(),
            crlf: loaded.crlf,
            final_newline: loaded.final_newline,
            bom: loaded.bom,
            mtime: loaded.mtime,
            number_mode: NumberMode::Absolute,
            auto_indent: true,
            quit: false,
            quit_after_save: false,
            help: false,
            width: 80,
            height: 24,
            theme,
            theme_idx: 0,
            bracket_match_pos: None,
            scrollbar: false,
        })
    }

    pub fn goto_line(&mut self, line: usize) {
        let y = line.saturating_sub(1).min(self.buf.lines.len() - 1);
        self.cy = y;
        self.cx = 0;
        self.update_want();
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
    }

    pub fn is_modified(&self) -> bool {
        self.buf.is_modified()
    }

    pub(crate) fn text_h(&self) -> usize {
        (self.height as usize).saturating_sub(2)
    }

    pub(crate) fn gutter_width(&self) -> usize {
        if self.number_mode == NumberMode::Off {
            return 0;
        }
        let mut d = 1;
        let mut n = self.buf.lines.len();
        while n >= 10 {
            n /= 10;
            d += 1;
        }
        d.max(3) + 1
    }

    pub(crate) fn text_w(&self) -> usize {
        (self.width as usize).saturating_sub(self.gutter_width()).max(1)
    }

    fn say<S: AsRef<str>>(&mut self, s: S) {
        self.msg = sanitize(s.as_ref());
    }

    fn line_len(&self, y: usize) -> usize {
        self.buf.lines.get(y).map_or(0, |l| l.len())
    }

    pub(crate) fn sel_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.sel?;
        let b = (self.cy, self.cx);
        if a == b {
            None
        } else if a < b {
            Some((a, b))
        } else {
            Some((b, a))
        }
    }

    fn clamp(&mut self) {
        let n = self.buf.lines.len();
        if self.cy >= n {
            self.cy = n - 1;
        }
        let l = self.line_len(self.cy);
        if self.cx > l {
            self.cx = l;
        }
        if let Some((y, x)) = self.sel {
            let y = y.min(n - 1);
            self.sel = Some((y, x.min(self.line_len(y))));
        }
    }

    fn update_want(&mut self) {
        self.want_x = disp_col(&self.buf.lines[self.cy], self.cx);
    }

    pub(crate) fn scroll(&mut self) {
        let th = self.text_h();
        let tw = self.text_w();
        if th == 0 {
            return;
        }
        self.clamp();
        if self.cy < self.top {
            self.top = self.cy;
        } else if self.cy >= self.top + th {
            self.top = self.cy + 1 - th;
        }
        let rx = disp_col(&self.buf.lines[self.cy], self.cx);
        if rx < self.left {
            self.left = rx;
        } else if rx >= self.left + tw {
            self.left = rx + 1 - tw;
        }
    }

    // ----------------------------------------------------- syntax cache

    /// Highlighter state at the start of line `y` (cached).
    pub(crate) fn state_at(&mut self, y: usize) -> State {
        if !self.lang.multiline() {
            return State::default();
        }
        let y = y.min(self.buf.lines.len() - 1);
        if self.states.is_empty() {
            self.states.push(State::default());
        }
        while self.states.len() <= y {
            let i = self.states.len() - 1;
            let mut st = self.states[i];
            syntax::highlight(self.lang, &self.buf.lines[i], &mut st, &mut self.tok);
            self.states.push(st);
        }
        self.states[y]
    }

    /// Ensure tok_cache has a valid entry for line y; returns a reference to the tokens.
    pub(crate) fn ensure_tok(&mut self, y: usize) -> &[syntax::Tok] {
        // Grow cache to cover y
        let n = self.buf.lines.len();
        if self.tok_cache.len() < n {
            self.tok_cache.resize(n, None);
        }
        if y >= self.tok_cache.len() {
            return &[];
        }
        if self.tok_cache[y].is_none() {
            let mut st = self.state_at(y);
            // Clone the line to avoid simultaneous &mut self and &self.buf.lines[y]
            let line = self.buf.lines[y].clone();
            let mut toks = Vec::new();
            syntax::highlight(self.lang, &line, &mut st, &mut toks);
            self.tok_cache[y] = Some(toks);
        }
        self.tok_cache[y].as_deref().unwrap_or(&[])
    }

    /// Ranges of search hits in a line (for highlighting).
    pub(crate) fn line_hits(&self, y: usize) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        let q = &self.find.query;
        if !self.find.show || q.is_empty() {
            return v;
        }
        let ci = smart_ci(q);
        let line = &self.buf.lines[y];
        let mut x = 0;
        while let Some(p) = find_fwd(line, q, x, ci) {
            v.push((p, p + q.len()));
            x = p + q.len();
        }
        v
    }

    // ----------------------------------------------------- bracket match

    /// Find the matching bracket for the character under the cursor.
    pub(crate) fn compute_bracket_match(&self) -> Option<(usize, usize)> {
        let line = self.buf.lines.get(self.cy)?;
        let ch = *line.get(self.cx)?;
        let (open, close, forward) = match ch {
            '(' => ('(', ')', true),
            '[' => ('[', ']', true),
            '{' => ('{', '}', true),
            ')' => (')', '(', false),
            ']' => (']', '[', false),
            '}' => ('}', '{', false),
            _ => return None,
        };
        let mut depth: isize = 0;
        let mut scanned = 0usize;
        const MAX_SCAN: usize = 10_000;

        if forward {
            let mut y = self.cy;
            let mut x = self.cx;
            loop {
                let row = match self.buf.lines.get(y) {
                    Some(r) => r,
                    None => return None,
                };
                while x < row.len() {
                    if scanned >= MAX_SCAN {
                        return None;
                    }
                    scanned += 1;
                    let c = row[x];
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some((y, x));
                        }
                    }
                    x += 1;
                }
                y += 1;
                x = 0;
                if y >= self.buf.lines.len() {
                    return None;
                }
            }
        } else {
            let mut y = self.cy;
            let mut xi = self.cx as isize;
            loop {
                let row = match self.buf.lines.get(y) {
                    Some(r) => r,
                    None => return None,
                };
                while xi >= 0 {
                    if scanned >= MAX_SCAN {
                        return None;
                    }
                    scanned += 1;
                    let c = row[xi as usize];
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some((y, xi as usize));
                        }
                    }
                    xi -= 1;
                }
                if y == 0 {
                    return None;
                }
                y -= 1;
                xi = self.buf.lines.get(y).map_or(0, |r| r.len() as isize - 1);
            }
        }
    }

    // ----------------------------------------------------- events

    /// Returns true when the screen needs a redraw.
    pub fn handle_event(&mut self, ev: Event) -> bool {
        match ev {
            Event::Key(k) => {
                if k.kind == KeyEventKind::Release {
                    return false;
                }
                self.msg.clear();
                self.on_key(k);
            }
            Event::Mouse(m) => {
                if matches!(m.kind, MouseEventKind::Moved) {
                    return false;
                }
                self.on_mouse(m);
            }
            Event::Paste(s) => self.on_paste(&s),
            Event::Resize(w, h) => self.resize(w, h),
            _ => return false,
        }
        let d = self.buf.take_dirty();
        if d != usize::MAX {
            self.states.truncate(d + 1);
            // Invalidate syntax cache from dirty line onward
            if d < self.tok_cache.len() {
                for entry in &mut self.tok_cache[d..] {
                    *entry = None;
                }
            }
        }
        self.clamp();
        self.bracket_match_pos = self.compute_bracket_match();
        true
    }

    fn on_key(&mut self, k: KeyEvent) {
        if self.help {
            self.help = false;
            return;
        }
        if self.prompt.is_some() {
            self.on_prompt_key(k);
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let word = ctrl || alt;
        match k.code {
            KeyCode::Char(c) if ctrl && !alt => self.ctrl_key(c),
            KeyCode::Char(c) if alt && !ctrl && c.is_ascii() => match c {
                'b' => self.key_left(shift, true),
                'f' => self.key_right(shift, true),
                _ => {}
            },
            KeyCode::Char(c) => self.type_char(c),
            KeyCode::Enter => self.newline(),
            KeyCode::Tab => self.tab(false),
            KeyCode::BackTab => self.tab(true),
            KeyCode::Backspace => {
                if word {
                    self.delete_word(false)
                } else {
                    self.backspace()
                }
            }
            KeyCode::Delete => {
                if word {
                    self.delete_word(true)
                } else {
                    self.delete_fwd()
                }
            }
            KeyCode::Left => self.key_left(shift, word),
            KeyCode::Right => self.key_right(shift, word),
            KeyCode::Up => self.vertical(-1, shift),
            KeyCode::Down => self.vertical(1, shift),
            KeyCode::Home => {
                if ctrl {
                    self.move_to(0, 0, shift);
                } else {
                    let line = &self.buf.lines[self.cy];
                    let first = line.iter().position(|c| !c.is_whitespace()).unwrap_or(0);
                    let x = if self.cx == first { 0 } else { first };
                    self.move_to(self.cy, x, shift);
                }
                self.update_want();
            }
            KeyCode::End => {
                if ctrl {
                    let y = self.buf.lines.len() - 1;
                    self.move_to(y, self.line_len(y), shift);
                } else {
                    self.move_to(self.cy, self.line_len(self.cy), shift);
                }
                self.update_want();
            }
            KeyCode::PageUp => self.page(-1, shift),
            KeyCode::PageDown => self.page(1, shift),
            KeyCode::Esc => {
                self.sel = None;
                self.find.show = false;
            }
            KeyCode::F(1) => self.help = true,
            KeyCode::F(3) => self.find_again(!shift),
            _ => {}
        }
    }

    fn ctrl_key(&mut self, c: char) {
        match c.to_ascii_lowercase() {
            's' => self.save(),
            'o' => {
                let init: Vec<char> = self
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().chars().filter(|c| !c.is_control()).collect())
                    .unwrap_or_default();
                self.open_prompt(PromptKind::SaveAs, "Save as: ", init);
            }
            'q' => self.request_quit(),
            'f' => self.open_find(),
            'r' => {
                let init = self.selection_text_oneline().unwrap_or_else(|| self.find.query.clone());
                self.open_prompt(PromptKind::ReplaceFrom, "Replace all: ", init);
            }
            'z' => match self.buf.undo() {
                Some((y, x)) => self.after_history(y, x),
                None => self.say("Nothing to undo"),
            },
            'y' => match self.buf.redo() {
                Some((y, x)) => self.after_history(y, x),
                None => self.say("Nothing to redo"),
            },
            'c' => self.copy(false),
            'x' => self.copy(true),
            'v' => {
                let t = self.clip.clone();
                let lw = self.clip_line;
                if t.is_empty() {
                    self.say("Clipboard empty (use your terminal's paste, e.g. Ctrl+Shift+V)");
                } else {
                    self.paste_text(&t, lw);
                }
            }
            'a' => {
                let y = self.buf.lines.len() - 1;
                self.sel = Some((0, 0));
                self.cy = y;
                self.cx = self.line_len(y);
            }
            'k' => {
                self.sel = None; // Ctrl+K always cuts the whole line
                self.copy(true)
            }
            'd' => self.duplicate_line(),
            'l' => self.open_prompt(PromptKind::Goto, "Go to line[:col]: ", Vec::new()),
            'g' => self.help = true,
            'n' => {
                self.number_mode = match self.number_mode {
                    NumberMode::Absolute => {
                        self.say("Line numbers: Relative");
                        NumberMode::Relative
                    }
                    NumberMode::Relative => {
                        self.say("Line numbers: Off");
                        NumberMode::Off
                    }
                    NumberMode::Off => {
                        self.say("Line numbers: Absolute");
                        NumberMode::Absolute
                    }
                };
            }
            'p' => {
                self.auto_indent = !self.auto_indent;
                let s = if self.auto_indent { "Auto-indent ON" } else { "Auto-indent OFF" };
                self.say(s);
            }
            't' => {
                let themes = all_themes();
                self.theme_idx = (self.theme_idx + 1) % themes.len();
                let name = themes[self.theme_idx].name;
                self.theme = themes.into_iter().nth(self.theme_idx).unwrap();
                self.say(format!("Theme: {}", name));
            }
            'h' => self.delete_word(false), // many terminals send ^H for Ctrl+Backspace
            _ => {}
        }
    }

    fn after_history(&mut self, y: usize, x: usize) {
        self.cy = y;
        self.cx = x;
        self.sel = None;
        self.clamp();
        self.update_want();
    }

    // ----------------------------------------------------- movement

    fn move_to(&mut self, y: usize, x: usize, extend: bool) {
        if extend {
            if self.sel.is_none() {
                self.sel = Some((self.cy, self.cx));
            }
        } else {
            self.sel = None;
        }
        self.cy = y;
        self.cx = x;
        self.buf.break_group();
    }

    fn key_left(&mut self, shift: bool, word: bool) {
        if !shift {
            if let Some((a, _)) = self.sel_range() {
                self.move_to(a.0, a.1, false);
                self.update_want();
                return;
            }
        }
        let (y, x) = if word {
            if self.cx == 0 && self.cy > 0 {
                (self.cy - 1, self.line_len(self.cy - 1))
            } else {
                (self.cy, word_left(&self.buf.lines[self.cy], self.cx))
            }
        } else if self.cx > 0 {
            (self.cy, self.cx - 1)
        } else if self.cy > 0 {
            (self.cy - 1, self.line_len(self.cy - 1))
        } else {
            (0, 0)
        };
        self.move_to(y, x, shift);
        self.update_want();
    }

    fn key_right(&mut self, shift: bool, word: bool) {
        if !shift {
            if let Some((_, b)) = self.sel_range() {
                self.move_to(b.0, b.1, false);
                self.update_want();
                return;
            }
        }
        let len = self.line_len(self.cy);
        let last = self.buf.lines.len() - 1;
        let (y, x) = if self.cx >= len {
            if self.cy < last {
                (self.cy + 1, 0)
            } else {
                (self.cy, len)
            }
        } else if word {
            (self.cy, word_right(&self.buf.lines[self.cy], self.cx))
        } else {
            (self.cy, self.cx + 1)
        };
        self.move_to(y, x, shift);
        self.update_want();
    }

    fn vertical(&mut self, dy: isize, shift: bool) {
        let last = self.buf.lines.len() - 1;
        let (y, x) = if dy < 0 {
            if self.cy == 0 {
                (0, 0)
            } else {
                let ny = self.cy - 1;
                (ny, cx_from_col(&self.buf.lines[ny], self.want_x))
            }
        } else if self.cy >= last {
            (last, self.line_len(last))
        } else {
            let ny = self.cy + 1;
            (ny, cx_from_col(&self.buf.lines[ny], self.want_x))
        };
        self.move_to(y, x, shift);
    }

    fn page(&mut self, dir: isize, shift: bool) {
        let th = self.text_h().max(2) - 1;
        let last = self.buf.lines.len() - 1;
        let ny = if dir < 0 { self.cy.saturating_sub(th) } else { (self.cy + th).min(last) };
        self.top = if dir < 0 { self.top.saturating_sub(th) } else { (self.top + th).min(last) };
        let x = cx_from_col(&self.buf.lines[ny], self.want_x);
        self.move_to(ny, x, shift);
    }

    // ----------------------------------------------------- editing

    fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.sel_range() {
            self.buf.delete_range(a, b);
            self.cy = a.0;
            self.cx = a.1;
            self.sel = None;
            self.update_want();
            true
        } else {
            self.sel = None;
            false
        }
    }

    fn type_char(&mut self, c: char) {
        if c.is_control() && c != '\t' {
            return;
        }
        let s = [c];
        if self.sel_range().is_some() {
            self.buf.begin_atomic();
            self.delete_selection();
            self.buf.insert(self.cy, self.cx, &s, KIND_OTHER);
            self.buf.end_atomic();
        } else {
            self.sel = None;
            self.buf.insert(self.cy, self.cx, &s, KIND_TYPE);
        }
        self.cx += 1;
        self.update_want();
    }

    fn indent_unit(&self) -> Vec<char> {
        if self.lang.hard_tabs {
            vec!['\t']
        } else {
            vec![' '; TAB]
        }
    }

    fn newline(&mut self) {
        self.buf.begin_atomic();
        self.delete_selection();
        let line = &self.buf.lines[self.cy];
        let mut indent: Vec<char> = Vec::new();
        if self.auto_indent {
            indent = line.iter().take(self.cx).take_while(|c| **c == ' ' || **c == '\t').copied().collect();
            let before = line[..self.cx].iter().rev().find(|c| !c.is_whitespace()).copied();
            if let Some(b) = before {
                if "{[(".contains(b) || (b == ':' && self.lang.name == "Python") {
                    indent.extend(self.indent_unit());
                }
            }
        }
        self.buf.split(self.cy, self.cx);
        self.cy += 1;
        self.buf.insert(self.cy, 0, &indent, KIND_OTHER);
        self.cx = indent.len();
        self.buf.end_atomic();
        self.update_want();
    }

    fn tab(&mut self, back: bool) {
        if let Some((a, b)) = self.sel_range() {
            if a.0 != b.0 || back {
                self.indent_lines(a, b, back);
                return;
            }
        } else if back {
            let p = (self.cy, self.cx);
            self.indent_lines(p, p, true);
            return;
        }
        self.buf.begin_atomic();
        self.delete_selection();
        let ins: Vec<char> = if self.lang.hard_tabs {
            vec!['\t']
        } else {
            let col = disp_col(&self.buf.lines[self.cy], self.cx);
            vec![' '; TAB - col % TAB]
        };
        self.buf.insert(self.cy, self.cx, &ins, KIND_OTHER);
        self.cx += ins.len();
        self.buf.end_atomic();
        self.update_want();
    }

    fn indent_lines(&mut self, a: (usize, usize), b: (usize, usize), back: bool) {
        let first = a.0;
        let mut last = b.0;
        if last > first && b.1 == 0 {
            last -= 1;
        }
        let unit = self.indent_unit();
        let mut deltas: Vec<(usize, usize)> = Vec::new(); // (line, chars added/removed)
        self.buf.begin_atomic();
        for y in first..=last {
            if !back {
                if self.buf.lines[y].is_empty() {
                    continue;
                }
                self.buf.insert(y, 0, &unit, KIND_OTHER);
                deltas.push((y, unit.len()));
            } else {
                let line = &self.buf.lines[y];
                let n = if line.first() == Some(&'\t') {
                    1
                } else {
                    line.iter().take(TAB).take_while(|c| **c == ' ').count()
                };
                if n > 0 {
                    self.buf.delete(y, 0, n, KIND_OTHER);
                    deltas.push((y, n));
                }
            }
        }
        self.buf.end_atomic();
        let adj = |y: usize, x: usize| -> usize {
            match deltas.iter().find(|d| d.0 == y) {
                Some(&(_, k)) => {
                    if back {
                        x.saturating_sub(k)
                    } else {
                        x + k
                    }
                }
                None => x,
            }
        };
        self.cx = adj(self.cy, self.cx);
        if let Some((sy, sx)) = self.sel {
            self.sel = Some((sy, adj(sy, sx)));
        }
        self.update_want();
    }

    fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cx > 0 {
            let line = &self.buf.lines[self.cy];
            // In pure indentation, delete back to the previous tab stop.
            let n = if !self.lang.hard_tabs && line[..self.cx].iter().all(|c| *c == ' ') {
                (self.cx - 1) % TAB + 1
            } else {
                1
            };
            self.buf.delete(self.cy, self.cx - n, n, KIND_DELETE);
            self.cx -= n;
        } else if self.cy > 0 {
            let l = self.line_len(self.cy - 1);
            self.buf.join(self.cy - 1);
            self.cy -= 1;
            self.cx = l;
        }
        self.update_want();
    }

    fn delete_fwd(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cx < self.line_len(self.cy) {
            self.buf.delete(self.cy, self.cx, 1, KIND_DELETE);
        } else if self.cy + 1 < self.buf.lines.len() {
            self.buf.join(self.cy);
        }
    }

    fn delete_word(&mut self, forward: bool) {
        if self.delete_selection() {
            return;
        }
        let line = &self.buf.lines[self.cy];
        if forward {
            let e = word_right(line, self.cx);
            if e > self.cx {
                self.buf.delete(self.cy, self.cx, e - self.cx, KIND_OTHER);
            } else {
                self.delete_fwd();
            }
        } else {
            let s = word_left(line, self.cx);
            if s < self.cx {
                self.buf.delete(self.cy, s, self.cx - s, KIND_OTHER);
                self.cx = s;
            } else {
                self.backspace();
            }
        }
        self.update_want();
    }

    fn duplicate_line(&mut self) {
        let l = self.buf.lines[self.cy].clone();
        self.buf.insert_lines(self.cy + 1, vec![l]);
        self.cy += 1;
        self.sel = None;
    }

    // ----------------------------------------------------- clipboard

    fn selection_text_oneline(&self) -> Option<Vec<char>> {
        let (a, b) = self.sel_range()?;
        if a.0 != b.0 || b.1 - a.1 > 200 {
            return None;
        }
        Some(self.buf.lines[a.0][a.1..b.1].to_vec())
    }

    /// Copy (or cut). With no selection the whole line is used.
    fn copy(&mut self, cut: bool) {
        let had_sel = self.sel_range().is_some();
        let (text, linewise) = if let Some((a, b)) = self.sel_range() {
            (self.buf.text_range(a, b), false)
        } else {
            let mut t: String = self.buf.lines[self.cy].iter().collect();
            t.push('\n');
            (t, true)
        };
        if text.len() > 32 * 1024 * 1024 {
            self.say("Selection too large to copy");
            return;
        }
        if self.osc52_enabled && text.len() <= 70_000 {
            self.osc52 = Some(base64(text.as_bytes()));
        }
        let n = text.chars().count();
        self.clip = text;
        self.clip_line = linewise;
        if cut {
            if had_sel {
                self.delete_selection();
            } else if self.buf.lines.len() > 1 {
                self.buf.delete_lines(self.cy, 1);
                self.cy = self.cy.min(self.buf.lines.len() - 1);
                self.cx = self.cx.min(self.line_len(self.cy));
            } else {
                let l = self.line_len(0);
                self.buf.delete(0, 0, l, KIND_OTHER);
                self.cx = 0;
            }
            self.update_want();
            self.say(format!("Cut {} chars", n));
        } else {
            self.say(format!("Copied {} chars", n));
        }
    }

    fn paste_text(&mut self, text: &str, linewise: bool) {
        let clean = clean_paste(text);
        if clean.is_empty() {
            return;
        }
        self.buf.begin_atomic();
        if linewise && self.sel_range().is_none() {
            let (y, _) = self.buf.insert_text(self.cy, 0, &clean);
            self.cy = y;
        } else {
            self.delete_selection();
            let (y, x) = self.buf.insert_text(self.cy, self.cx, &clean);
            self.cy = y;
            self.cx = x;
        }
        self.buf.end_atomic();
        self.sel = None;
        self.clamp();
        self.update_want();
    }

    fn on_paste(&mut self, s: &str) {
        if let Some(p) = &mut self.prompt {
            if !matches!(p.kind, PromptKind::ConfirmQuit | PromptKind::ConfirmOverwrite) {
                for c in s.chars().filter(|c| !c.is_control()) {
                    p.input.push(c);
                }
            }
            return;
        }
        self.paste_text(s, false);
    }

    // ----------------------------------------------------- mouse

    fn pos_at(&self, col: usize, row: usize) -> (usize, usize) {
        let y = (self.top + row).min(self.buf.lines.len() - 1);
        let g = self.gutter_width();
        let x = if col < g { 0 } else { cx_from_col(&self.buf.lines[y], col - g + self.left) };
        (y, x)
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        if self.prompt.is_some() || self.help {
            return;
        }
        let (col, row) = (m.column as usize, m.row as usize);
        let th = self.text_h();
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if row >= th {
                    return;
                }
                let (y, x) = self.pos_at(col, row);
                let now = Instant::now();
                let count = match self.last_click {
                    Some((t, ly, lx, c)) if now.duration_since(t) < Duration::from_millis(450) && ly == y && lx == x => {
                        c % 3 + 1
                    }
                    _ => 1,
                };
                self.last_click = Some((now, y, x, count));
                self.buf.break_group();
                match count {
                    2 => {
                        let line = &self.buf.lines[y];
                        let mut s = x.min(line.len());
                        let mut e = s;
                        if s < line.len() {
                            let k = class(line[s]);
                            while s > 0 && class(line[s - 1]) == k {
                                s -= 1;
                            }
                            while e < line.len() && class(line[e]) == k {
                                e += 1;
                            }
                        }
                        self.sel = Some((y, s));
                        self.cy = y;
                        self.cx = e;
                        self.drag = None;
                    }
                    3 => {
                        self.sel = Some((y, 0));
                        self.cy = y;
                        self.cx = self.line_len(y);
                        self.drag = None;
                    }
                    _ => {
                        self.sel = None;
                        self.cy = y;
                        self.cx = x;
                        self.drag = Some((y, x));
                    }
                }
                self.update_want();
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(anchor) = self.drag {
                    let (y, x) = self.pos_at(col, row);
                    self.sel = Some(anchor);
                    self.cy = y;
                    self.cx = x;
                    self.update_want();
                }
            }
            MouseEventKind::Up(_) => self.drag = None,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let last = self.buf.lines.len() - 1;
                if matches!(m.kind, MouseEventKind::ScrollUp) {
                    self.top = self.top.saturating_sub(3);
                } else {
                    self.top = (self.top + 3).min(last);
                }
                let bottom = (self.top + th.max(1) - 1).min(last);
                let ny = self.cy.clamp(self.top, bottom);
                if ny != self.cy {
                    self.cy = ny;
                    self.cx = cx_from_col(&self.buf.lines[ny], self.want_x);
                }
            }
            MouseEventKind::ScrollLeft => self.left = self.left.saturating_sub(6),
            MouseEventKind::ScrollRight => self.left += 6,
            _ => {}
        }
    }

    // ----------------------------------------------------- prompts

    fn open_prompt(&mut self, kind: PromptKind, label: &str, input: Vec<char>) {
        let fresh = !input.is_empty();
        self.prompt = Some(Prompt { kind, label: label.to_string(), input, fresh });
    }

    fn open_find(&mut self) {
        let (init, origin) = match self.sel_range() {
            Some((a, _)) => (self.selection_text_oneline().unwrap_or_else(|| self.find.query.clone()), a),
            None => (self.find.query.clone(), (self.cy, self.cx)),
        };
        self.find.origin = origin;
        self.open_prompt(PromptKind::Find, "Find (Enter next, Up prev, Esc done): ", init);
        self.find_incremental();
    }

    fn on_prompt_key(&mut self, k: KeyEvent) {
        let Some(p) = &self.prompt else { return };
        let kind = p.kind;
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);

        // Single-key confirmations.
        match kind {
            PromptKind::ConfirmQuit => {
                match k.code {
                    KeyCode::Char('s' | 'S' | 'y' | 'Y') => {
                        self.prompt = None;
                        self.quit_after_save = true;
                        self.save();
                    }
                    KeyCode::Char('d' | 'D' | 'n' | 'N') => {
                        self.prompt = None;
                        self.quit = true;
                    }
                    KeyCode::Esc | KeyCode::Char('c' | 'C') => self.prompt = None,
                    _ => {}
                }
                return;
            }
            PromptKind::ConfirmOverwrite => {
                match k.code {
                    KeyCode::Char('y' | 'Y') => {
                        self.prompt = None;
                        if let Some(p) = self.pending_path.take() {
                            self.lang = syntax::detect(Some(&p));
                            self.states.truncate(1);
                            self.path = Some(p);
                        }
                        self.do_save(true);
                    }
                    KeyCode::Esc | KeyCode::Char('n' | 'N' | 'c' | 'C') => {
                        self.prompt = None;
                        self.pending_path = None;
                        self.quit_after_save = false;
                        self.say("Save cancelled");
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }

        match k.code {
            KeyCode::Esc => {
                self.prompt = None;
                self.quit_after_save = false;
            }
            KeyCode::Enter => {
                if kind == PromptKind::Find {
                    self.find_again(true);
                } else {
                    self.submit_prompt();
                }
            }
            KeyCode::Up if kind == PromptKind::Find => self.find_again(false),
            KeyCode::Down if kind == PromptKind::Find => self.find_again(true),
            KeyCode::F(3) if kind == PromptKind::Find => self.find_again(!shift),
            KeyCode::Backspace => {
                if let Some(p) = &mut self.prompt {
                    p.fresh = false;
                    p.input.pop();
                }
                self.prompt_changed();
            }
            KeyCode::Char('u') if ctrl && !alt => {
                if let Some(p) = &mut self.prompt {
                    p.input.clear();
                    p.fresh = false;
                }
                self.prompt_changed();
            }
            KeyCode::Char(c) if (!ctrl || alt) && !c.is_control() => {
                if let Some(p) = &mut self.prompt {
                    if p.fresh {
                        p.input.clear();
                        p.fresh = false;
                    }
                    if p.input.len() < 4096 {
                        p.input.push(c);
                    }
                }
                self.prompt_changed();
            }
            _ => {}
        }
    }

    fn prompt_changed(&mut self) {
        if matches!(self.prompt.as_ref().map(|p| p.kind), Some(PromptKind::Find)) {
            self.find_incremental();
        }
    }

    fn submit_prompt(&mut self) {
        let Some(p) = self.prompt.take() else { return };
        let text: String = p.input.iter().collect();
        match p.kind {
            PromptKind::SaveAs => {
                let t = text.trim();
                if t.is_empty() {
                    self.quit_after_save = false;
                    self.say("Save cancelled");
                    return;
                }
                let pb = PathBuf::from(t);
                if Some(&pb) == self.path.as_ref() {
                    self.do_save(false);
                } else if pb.exists() {
                    self.pending_path = Some(pb);
                    self.open_prompt(PromptKind::ConfirmOverwrite, "File exists. Overwrite? (y/n) ", Vec::new());
                } else {
                    self.lang = syntax::detect(Some(&pb));
                    self.states.truncate(1);
                    self.path = Some(pb);
                    self.mtime = None;
                    self.do_save(true);
                }
            }
            PromptKind::Goto => {
                let mut it = text.trim().splitn(2, ':');
                let line = it.next().and_then(|s| s.trim().parse::<usize>().ok());
                let col = it.next().and_then(|s| s.trim().parse::<usize>().ok());
                match line {
                    Some(l) => {
                        let y = l.saturating_sub(1).min(self.buf.lines.len() - 1);
                        let x = col.map_or(0, |c| c.saturating_sub(1)).min(self.line_len(y));
                        self.move_to(y, x, false);
                        self.update_want();
                    }
                    None => self.say("Not a line number"),
                }
            }
            PromptKind::ReplaceFrom => {
                if p.input.is_empty() {
                    return;
                }
                self.replace_from = p.input;
                self.open_prompt(PromptKind::ReplaceTo, "Replace with: ", Vec::new());
            }
            PromptKind::ReplaceTo => {
                let from = std::mem::take(&mut self.replace_from);
                self.replace_all(&from, &p.input);
            }
            _ => {}
        }
    }

    // ----------------------------------------------------- find / replace

    fn search(&self, forward: bool, from: (usize, usize)) -> Option<(usize, usize)> {
        let q = &self.find.query;
        if q.is_empty() {
            return None;
        }
        let ci = smart_ci(q);
        let n = self.buf.lines.len();
        let from = (from.0.min(n - 1), from.1);
        if forward {
            for k in 0..=n {
                let y = (from.0 + k) % n;
                let start = if k == 0 { from.1 } else { 0 };
                if let Some(x) = find_fwd(&self.buf.lines[y], q, start, ci) {
                    return Some((y, x));
                }
            }
        } else {
            for k in 0..=n {
                let y = (from.0 + n - (k % n)) % n;
                let before = if k == 0 { from.1 } else { usize::MAX };
                if let Some(x) = find_back(&self.buf.lines[y], q, before, ci) {
                    return Some((y, x));
                }
            }
        }
        None
    }

    fn select_match(&mut self, y: usize, x: usize) {
        self.sel = Some((y, x));
        self.cy = y;
        self.cx = x + self.find.query.len();
        self.update_want();
        self.buf.break_group();
    }

    fn find_incremental(&mut self) {
        let q = self.prompt.as_ref().map(|p| p.input.clone()).unwrap_or_default();
        self.find.query = q;
        self.find.show = !self.find.query.is_empty();
        let origin = self.find.origin;
        if self.find.query.is_empty() {
            self.sel = None;
            self.cy = origin.0.min(self.buf.lines.len() - 1);
            self.cx = origin.1.min(self.line_len(self.cy));
            return;
        }
        match self.search(true, origin) {
            Some((y, x)) => self.select_match(y, x),
            None => {
                self.sel = None;
                self.cy = origin.0.min(self.buf.lines.len() - 1);
                self.cx = origin.1.min(self.line_len(self.cy));
                self.say("No match");
            }
        }
    }

    fn find_again(&mut self, forward: bool) {
        if self.find.query.is_empty() {
            self.say("Nothing to search for (Ctrl+F)");
            return;
        }
        self.find.show = true;
        let from = match self.sel_range() {
            Some((a, b)) => {
                if forward {
                    b
                } else {
                    a
                }
            }
            None => (self.cy, self.cx),
        };
        match self.search(forward, from) {
            Some((y, x)) => {
                self.select_match(y, x);
                self.find.origin = (y, x);
            }
            None => self.say("No match"),
        }
    }

    fn replace_all(&mut self, from: &[char], to: &[char]) {
        if from.is_empty() {
            return;
        }
        let ci = smart_ci(from);
        let m = from.len();
        let mut count = 0usize;
        self.buf.begin_atomic();
        for y in 0..self.buf.lines.len() {
            let mut xs = Vec::new();
            let mut x = 0;
            while let Some(p) = find_fwd(&self.buf.lines[y], from, x, ci) {
                xs.push(p);
                x = p + m;
            }
            for &p in xs.iter().rev() {
                self.buf.delete(y, p, m, KIND_OTHER);
                self.buf.insert(y, p, to, KIND_OTHER);
                count += 1;
            }
        }
        self.buf.end_atomic();
        self.sel = None;
        self.find.query = from.to_vec();
        self.find.show = false;
        self.clamp();
        self.update_want();
        if count == 0 {
            self.say("No match");
        } else {
            self.say(format!("Replaced {} occurrence{}", count, if count == 1 { "" } else { "s" }));
        }
    }

    // ----------------------------------------------------- save / quit

    fn request_quit(&mut self) {
        if !self.buf.is_modified() {
            self.quit = true;
        } else {
            self.open_prompt(
                PromptKind::ConfirmQuit,
                "Unsaved changes!  Save (s)  Discard (d)  Cancel (Esc) ",
                Vec::new(),
            );
        }
    }

    fn save(&mut self) {
        if self.path.is_none() {
            self.open_prompt(PromptKind::SaveAs, "Save as: ", Vec::new());
        } else {
            self.do_save(false);
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let eol: &[u8] = if self.crlf { b"\r\n" } else { b"\n" };
        let mut out: Vec<u8> = Vec::new();
        if self.bom {
            out.extend_from_slice("\u{feff}".as_bytes());
        }
        let n = self.buf.lines.len();
        let mut tmp = [0u8; 4];
        for (i, line) in self.buf.lines.iter().enumerate() {
            for &c in line {
                out.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
            if i + 1 < n || self.final_newline {
                out.extend_from_slice(eol);
            }
        }
        out
    }

    fn do_save(&mut self, force: bool) {
        let Some(path) = self.path.clone() else { return };
        if !force {
            let cur = fileio::mtime_of(&path);
            if cur != self.mtime {
                self.open_prompt(
                    PromptKind::ConfirmOverwrite,
                    "File changed on disk since it was opened! Overwrite? (y/n) ",
                    Vec::new(),
                );
                return;
            }
        }
        let data = self.serialize();
        match fileio::save(&path, &data) {
            Ok(()) => {
                self.buf.mark_saved();
                self.mtime = fileio::mtime_of(&path);
                self.say(format!("Saved {} lines, {} bytes", self.buf.lines.len(), data.len()));
                if self.quit_after_save {
                    self.quit = true;
                }
            }
            Err(e) => {
                self.quit_after_save = false;
                self.say(format!("Save FAILED: {}", e));
            }
        }
    }
}
