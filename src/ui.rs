//! Rendering. One buffered write per frame; the only thing drawn is what is visible.

use crate::editor::{cell_width, disp_col, sanitize, suspicious, Editor, NumberMode};
use crate::syntax::Tok;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

const HELP: &[&str] = &[
    " LEKH  (lk)  -  keys",
    "",
    " FILE    Ctrl+S save          Ctrl+O save as        Ctrl+Q quit",
    " EDIT    Ctrl+Z undo          Ctrl+Y redo",
    "         Ctrl+C copy          Ctrl+X cut            Ctrl+V paste",
    "         Ctrl+K cut line      Ctrl+D duplicate line Ctrl+A select all",
    "         Tab / Shift+Tab indent / dedent (also on a selection)",
    "         Ctrl+Backspace / Ctrl+Delete  delete word",
    " FIND    Ctrl+F find (type to search live)   F3 / Shift+F3 next / prev",
    "         Ctrl+R replace all   (smart case: lowercase query ignores case)",
    " MOVE    Arrows, Home (smart), End, PgUp, PgDn",
    "         Ctrl+Left/Right word   Ctrl+Home/End start/end of file",
    "         Ctrl+L go to line[:col]",
    " SELECT  Shift+movement, Ctrl+A, mouse drag, double-click word,",
    "         triple-click line",
    " MOUSE   click to place the cursor, wheel to scroll",
    " VIEW    Ctrl+N line numbers   Ctrl+P auto-indent   Ctrl+T theme",
    "         Ctrl+G / F1 this help",
    "",
    " Paste from other programs with your terminal's paste (Ctrl+Shift+V /",
    " Cmd+V). Ctrl+C also copies to the system clipboard on terminals that",
    " support OSC 52.                                  Press any key to close.",
];

// ------------------------------------------------------------------ theme

/// Full 24-bit RGB color theme.
#[derive(Clone)]
pub struct Theme {
    pub name: &'static str,
    // token colors
    pub normal:   Color,
    pub keyword:  Color,
    pub typ:      Color,
    pub func:     Color,
    pub string:   Color,
    pub comment:  Color,
    pub number:   Color,
    pub constant: Color,
    pub macro_c:  Color,
    pub tag:      Color,
    pub attr:     Color,
    pub var:      Color,
    // ui colors
    pub bg:           Color,
    pub curline_bg:   Color,
    pub gutter_fg:    Color,
    pub gutter_cur:   Color,
    pub statusbar_bg: Color,
    pub statusbar_fg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub search_bg:    Color,
    pub search_fg:    Color,
    pub bracket_bg:   Color,
    pub bracket_fg:   Color,
    pub scrollbar_fg: Color,
    pub scrollbar_thumb: Color,
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb { r, g, b }
}

fn theme_default() -> Theme {
    Theme {
        name: "Default",
        normal:   Color::Reset,
        keyword:  Color::Magenta,
        typ:      Color::Cyan,
        func:     Color::Blue,
        string:   Color::Green,
        comment:  Color::DarkGrey,
        number:   Color::Yellow,
        constant: Color::DarkYellow,
        macro_c:  Color::DarkMagenta,
        tag:      Color::Red,
        attr:     Color::DarkYellow,
        var:      Color::AnsiValue(215),
        bg:           Color::Reset,
        curline_bg:   Color::Reset,
        gutter_fg:    Color::DarkGrey,
        gutter_cur:   Color::White,
        statusbar_bg: Color::Reset,
        statusbar_fg: Color::Reset,
        selection_bg: Color::DarkBlue,
        selection_fg: Color::White,
        search_bg:    Color::DarkYellow,
        search_fg:    Color::Black,
        bracket_bg:   Color::Cyan,
        bracket_fg:   Color::Black,
        scrollbar_fg: Color::DarkGrey,
        scrollbar_thumb: Color::White,
    }
}

fn theme_dracula() -> Theme {
    Theme {
        name: "Dracula",
        normal:   rgb(248, 248, 242),
        keyword:  rgb(255, 121, 198),
        typ:      rgb(139, 233, 253),
        func:     rgb( 80, 250, 123),
        string:   rgb(241, 250, 140),
        comment:  rgb(130, 148, 204),
        number:   rgb(189, 147, 249),
        constant: rgb(255, 184, 108),
        macro_c:  rgb(255, 121, 198),
        tag:      rgb(255,  95,  95),
        attr:     rgb(255, 184, 108),
        var:      rgb(139, 233, 253),
        bg:           Color::Reset,
        curline_bg:   Color::Reset,
        gutter_fg:    rgb(118, 134, 186),
        gutter_cur:   rgb(255, 121, 198),
        statusbar_bg: rgb(189, 147, 249),
        statusbar_fg: rgb( 18,  19,  26),
        selection_bg: rgb( 78,  92, 138),
        selection_fg: rgb(255, 255, 255),
        search_bg:    rgb(255, 184, 108),
        search_fg:    rgb( 20,  20,  28),
        bracket_bg:   rgb(139, 233, 253),
        bracket_fg:   rgb( 20,  20,  28),
        scrollbar_fg: rgb( 98, 114, 164),
        scrollbar_thumb: rgb(189, 147, 249),
    }
}

fn theme_nord() -> Theme {
    Theme {
        name: "Nord",
        normal:   rgb(236, 239, 244),
        keyword:  rgb(110, 178, 240),
        typ:      rgb(120, 218, 235),
        func:     rgb(126, 224, 208),
        string:   rgb(168, 220, 136),
        comment:  rgb(128, 144, 172),
        number:   rgb(208, 152, 214),
        constant: rgb(245, 208, 125),
        macro_c:  rgb(110, 178, 240),
        tag:      rgb(240, 108, 120),
        attr:     rgb(245, 208, 125),
        var:      rgb(120, 218, 235),
        bg:           Color::Reset,
        curline_bg:   Color::Reset,
        gutter_fg:    rgb(112, 128, 154),
        gutter_cur:   rgb(120, 218, 235),
        statusbar_bg: rgb(120, 206, 226),
        statusbar_fg: rgb( 16,  22,  28),
        selection_bg: rgb( 54,  88, 128),
        selection_fg: rgb(255, 255, 255),
        search_bg:    rgb(245, 208, 125),
        search_fg:    rgb( 18,  22,  28),
        bracket_bg:   rgb(120, 218, 235),
        bracket_fg:   rgb( 18,  22,  28),
        scrollbar_fg: rgb( 92, 108, 134),
        scrollbar_thumb: rgb(120, 206, 226),
    }
}

fn theme_onedark() -> Theme {
    Theme {
        name: "OneDark",
        normal:   rgb(238, 242, 250),
        keyword:  rgb(213, 123, 255),
        typ:      rgb( 86, 212, 226),
        func:     rgb( 97, 185, 255),
        string:   rgb(152, 216, 115),
        comment:  rgb(132, 142, 160),
        number:   rgb(240, 158,  92),
        constant: rgb(242, 202, 118),
        macro_c:  rgb(255, 107, 118),
        tag:      rgb(255, 107, 118),
        attr:     rgb(242, 202, 118),
        var:      rgb(255, 128, 138),
        bg:           Color::Reset,
        curline_bg:   Color::Reset,
        gutter_fg:    rgb(116, 126, 144),
        gutter_cur:   rgb( 97, 185, 255),
        statusbar_bg: rgb( 97, 175, 239),
        statusbar_fg: rgb( 16,  19,  26),
        selection_bg: rgb( 52,  82, 126),
        selection_fg: rgb(255, 255, 255),
        search_bg:    rgb(242, 202, 118),
        search_fg:    rgb( 18,  20,  26),
        bracket_bg:   rgb( 86, 212, 226),
        bracket_fg:   rgb( 18,  20,  26),
        scrollbar_fg: rgb( 92, 102, 120),
        scrollbar_thumb: rgb( 97, 175, 239),
    }
}

fn theme_gruvbox() -> Theme {
    Theme {
        name: "Gruvbox",
        normal:   rgb(251, 241, 199),
        keyword:  rgb(254,  82,  60),
        typ:      rgb(250, 196,  47),
        func:     rgb(142, 208, 124),
        string:   rgb(196, 208,  42),
        comment:  rgb(168, 153, 132),
        number:   rgb(224, 142, 172),
        constant: rgb(254, 148,  40),
        macro_c:  rgb(250, 196,  47),
        tag:      rgb(254, 108,  52),
        attr:     rgb(254, 148,  40),
        var:      rgb(131, 192, 168),
        bg:           Color::Reset,
        curline_bg:   Color::Reset,
        gutter_fg:    rgb(146, 131, 116),
        gutter_cur:   rgb(250, 196,  47),
        statusbar_bg: rgb(250, 189,  47),
        statusbar_fg: rgb( 20,  20,  20),
        selection_bg: rgb( 96,  78,  58),
        selection_fg: rgb(255, 250, 224),
        search_bg:    rgb(254, 148,  40),
        search_fg:    rgb( 20,  20,  20),
        bracket_bg:   rgb(142, 208, 124),
        bracket_fg:   rgb( 20,  20,  20),
        scrollbar_fg: rgb(124, 111, 100),
        scrollbar_thumb: rgb(250, 189,  47),
    }
}

pub fn all_themes() -> Vec<Theme> {
    vec![
        theme_default(),
        theme_dracula(),
        theme_nord(),
        theme_onedark(),
        theme_gruvbox(),
    ]
}

fn tok_color(t: Tok, theme: &Theme) -> Color {
    match t {
        Tok::Normal  => theme.normal,
        Tok::Keyword => theme.keyword,
        Tok::Type    => theme.typ,
        Tok::Func    => theme.func,
        Tok::Str     => theme.string,
        Tok::Comment => theme.comment,
        Tok::Number  => theme.number,
        Tok::Const   => theme.constant,
        Tok::Macro   => theme.macro_c,
        Tok::Tag     => theme.tag,
        Tok::Attr    => theme.attr,
        Tok::Var     => theme.var,
    }
}

// ------------------------------------------------------------------ helpers

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Color,
    bg: Color,
    rev: bool,
}

const DEFAULT: Style = Style { fg: Color::Reset, bg: Color::Reset, rev: false };

fn fit(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cw > w {
            break;
        }
        used += cw;
        out.push(c);
    }
    out
}

fn width_of(s: &str) -> usize {
    s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
}

/// Language emoji prefix for the status bar.
fn lang_emoji(name: &str) -> &'static str {
    match name {
        "Rust"                                       => "🦀 ",
        "Python"                                     => "🐍 ",
        "JavaScript/TypeScript" | "JavaScript" | "TypeScript" => "🟨 ",
        "C/C++" | "C" | "C++"                        => "⚙ ",
        "Go"                                         => "🐹 ",
        "Shell" | "Bash"                             => "🐚 ",
        "HTML/XML" | "HTML" | "XML"                  => "🌐 ",
        "CSS" | "SCSS" | "Sass"                      => "🎨 ",
        "JSON"                                       => "{} ",
        "Markdown"                                   => "📝 ",
        "TOML/YAML/INI" | "TOML" | "YAML" | "INI"    => "⚙ ",
        "Java/C#" | "Java" | "C#" | "Kotlin"         => "☕ ",
        "Lua"                                        => "🌙 ",
        "Ruby"                                       => "💎 ",
        "SQL"                                        => "🗄️ ",
        "Zig"                                        => "⚡ ",
        _                                            => "📄 ",
    }
}

impl Editor {
    pub fn render<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        let w = self.width as usize;
        let h = self.height as usize;
        queue!(out, Hide, MoveTo(0, 0))?;
        // Keep tok_cache in sync with buffer line count
        let n = self.buf.lines.len();
        if self.tok_cache.len() > n + 64 {
            self.tok_cache.truncate(n);
        }
        if w < 24 || h < 6 {
            queue!(out, Clear(ClearType::All), Print("lekh: terminal too small"))?;
            return out.flush();
        }
        if let Some(b64) = self.osc52.take() {
            // OSC 52: ask the terminal to put the text on the system clipboard.
            queue!(out, Print(format!("\x1b]52;c;{}\x07", b64)))?;
        }
        self.scroll();
        let th = self.text_h();
        let gutter = self.gutter_width();

        // Determine if we show the scrollbar
        let total_lines = self.buf.lines.len();
        let show_scrollbar = total_lines > th;
        self.scrollbar = show_scrollbar;

        if self.help {
            for r in 0..th {
                let l = HELP.get(r).copied().unwrap_or("");
                let shown = fit(l, w);
                let pad = w.saturating_sub(width_of(&shown));
                queue!(
                    out,
                    MoveTo(0, r as u16),
                    SetAttribute(Attribute::Reset),
                    SetForegroundColor(self.theme.normal),
                    SetBackgroundColor(self.theme.bg),
                    Print(&shown),
                    Print(" ".repeat(pad)),
                    Clear(ClearType::UntilNewLine),
                    ResetColor
                )?;
            }
        } else {
            let sb_adjust = if show_scrollbar { 1 } else { 0 };
            let fill_w = w.saturating_sub(sb_adjust);
            for r in 0..th {
                queue!(out, MoveTo(0, r as u16), ResetColor)?;
                let y = self.top + r;
                if y < self.buf.lines.len() {
                    self.draw_row(out, y, w, gutter)?;
                } else {
                    // Paint full empty row with theme background so True-Color themes cover the canvas seamlessly
                    queue!(
                        out,
                        SetBackgroundColor(self.theme.bg),
                        Print(" ".repeat(fill_w))
                    )?;
                    if !show_scrollbar {
                        queue!(out, Clear(ClearType::UntilNewLine))?;
                    }
                    queue!(out, ResetColor)?;
                }
            }

            // Draw scrollbar on the right edge if needed
            if show_scrollbar && w > 1 {
                let track_h = th;
                let thumb_size = ((track_h * track_h) / total_lines.max(1)).max(1).min(track_h);
                let thumb_top = (self.top * (track_h - thumb_size)) / (total_lines - th).max(1);
                let thumb_top = thumb_top.min(track_h.saturating_sub(thumb_size));

                for r in 0..track_h {
                    let is_thumb = r >= thumb_top && r < thumb_top + thumb_size;
                    let ch = if is_thumb { '█' } else { '│' };
                    let fg = if is_thumb {
                        self.theme.scrollbar_thumb
                    } else {
                        self.theme.scrollbar_fg
                    };
                    queue!(
                        out,
                        MoveTo((w - 1) as u16, r as u16),
                        SetForegroundColor(fg),
                        SetBackgroundColor(self.theme.bg),
                        Print(ch),
                        ResetColor
                    )?;
                }
            }
        }

        // status bar
        let rx = disp_col(&self.buf.lines[self.cy], self.cx);
        let name = match &self.path {
            Some(p) => sanitize(&p.to_string_lossy()),
            None => "[No Name]".to_string(),
        };
        let left = format!(" {}{}", name, if self.is_modified() { " [+]" } else { "" });
        let emoji = lang_emoji(self.lang.name);
        let pct = if self.buf.lines.is_empty() { 100 } else { ((self.cy + 1) * 100) / self.buf.lines.len() };
        let right = format!(
            "{}{}  {}%  Ln {}/{}  Col {}  {} ",
            emoji,
            self.lang.name,
            pct,
            self.cy + 1,
            self.buf.lines.len(),
            rx + 1,
            if self.crlf { "CRLF" } else { "LF" }
        );
        let rw = width_of(&right);
        let left = fit(&left, w.saturating_sub(rw + 1));
        let pad = w.saturating_sub(width_of(&left) + rw);

        let sb_bg = self.theme.statusbar_bg;
        let sb_fg = self.theme.statusbar_fg;
        let use_custom_status = !matches!(sb_bg, Color::Reset);

        if use_custom_status {
            queue!(
                out,
                MoveTo(0, (h - 2) as u16),
                SetBackgroundColor(sb_bg),
                SetForegroundColor(sb_fg),
                Print(&left),
                Print(" ".repeat(pad)),
                Print(&right),
                ResetColor
            )?;
        } else {
            queue!(
                out,
                MoveTo(0, (h - 2) as u16),
                SetAttribute(Attribute::Reverse),
                Print(&left),
                Print(" ".repeat(pad)),
                Print(&right),
                SetAttribute(Attribute::Reset)
            )?;
        }

        // message / prompt line
        queue!(
            out,
            MoveTo(0, (h - 1) as u16),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(self.theme.bg),
            SetForegroundColor(self.theme.normal)
        )?;
        let mut cursor = (gutter + rx.saturating_sub(self.left), self.cy.saturating_sub(self.top));
        let max_msg_w = w.saturating_sub(1);
        let msg_used;
        if let Some(p) = &self.prompt {
            let input: String = p.input.iter().map(|&c| if c.is_control() { '?' } else { c }).collect();
            let label = fit(&p.label, max_msg_w);
            let label_w = width_of(&label);
            let avail = max_msg_w.saturating_sub(label_w);
            // show the tail of long input
            let mut shown = input;
            while width_of(&shown) > avail {
                shown.remove(0);
            }
            let shown_w = width_of(&shown);
            queue!(
                out,
                SetAttribute(Attribute::Bold),
                Print(&label),
                SetAttribute(Attribute::Reset),
                SetBackgroundColor(self.theme.bg),
                SetForegroundColor(self.theme.normal),
                Print(&shown)
            )?;
            msg_used = label_w + shown_w;
            cursor = ((label_w + shown_w).min(w - 1), h - 1);
        } else if !self.msg.is_empty() {
            let shown = fit(&self.msg, max_msg_w);
            msg_used = width_of(&shown);
            let msg_fg = if matches!(self.theme.statusbar_bg, Color::Reset) {
                Color::Reset
            } else {
                self.theme.statusbar_bg
            };
            queue!(
                out,
                SetForegroundColor(msg_fg),
                Print(&shown),
                SetAttribute(Attribute::Reset),
                SetBackgroundColor(self.theme.bg)
            )?;
        } else {
            let hint_fg = if matches!(self.theme.bg, Color::Reset) {
                Color::DarkGrey
            } else {
                self.theme.comment
            };
            let shown = fit(" ^S Save  ^Q Quit  ^F Find  ^Z Undo  ^T Theme  ^G Help", max_msg_w);
            msg_used = width_of(&shown);
            queue!(out, SetForegroundColor(hint_fg), Print(&shown))?;
        }
        let msg_pad = max_msg_w.saturating_sub(msg_used);
        if msg_pad > 0 {
            queue!(out, Print(" ".repeat(msg_pad)))?;
        }
        queue!(out, Clear(ClearType::UntilNewLine), ResetColor)?;

        if self.help {
            cursor = (0, 0);
        }
        queue!(out, MoveTo(cursor.0.min(w - 1) as u16, cursor.1.min(h - 1) as u16), Show)?;
        out.flush()
    }

    fn draw_row<W: Write>(&mut self, out: &mut W, y: usize, w: usize, gutter: usize) -> io::Result<()> {
        let is_curline = y == self.cy && self.prompt.is_none();
        let curline_bg = if is_curline { self.theme.curline_bg } else { self.theme.bg };

        if gutter > 0 {
            let col = if y == self.cy { self.theme.gutter_cur } else { self.theme.gutter_fg };
            let num = match self.number_mode {
                NumberMode::Relative if y != self.cy => (y as isize - self.cy as isize).unsigned_abs(),
                _ => y + 1,
            };
            queue!(
                out,
                SetForegroundColor(col),
                SetBackgroundColor(curline_bg),
                Print(format!("{:>width$} ", num, width = gutter - 1)),
                ResetColor
            )?;
        }

        // Effective text width: reduce by 1 if scrollbar is shown
        let sb_adjust = if self.scrollbar { 1 } else { 0 };
        let text_w = w.saturating_sub(gutter).saturating_sub(sb_adjust);

        // things that need &self first
        let sel = self.sel_range();
        let (s0, s1) = match sel {
            Some((a, b)) if y >= a.0 && y <= b.0 => {
                (if y == a.0 { a.1 } else { 0 }, if y == b.0 { b.1 } else { usize::MAX })
            }
            _ => (1, 0),
        };
        let hits = self.line_hits(y);
        let bracket_match = self.bracket_match_pos;
        // Use the per-line token cache; clone so we don't hold &mut self during draw
        let toks = self.ensure_tok(y).to_vec();
        let line = self.buf.lines[y].clone();

        let left = self.left;
        let right = left + text_w;
        let mut col = 0usize;
        let mut drawn = 0usize;
        let mut cur = DEFAULT;
        let mut hit_i = 0usize;

        for (i, &c) in line.iter().enumerate() {
            let cw = cell_width(c, col);
            if col + cw <= left && !(cw == 0 && col >= left) {
                col += cw;
                continue;
            }
            if col >= right {
                break;
            }
            while hit_i < hits.len() && hits[hit_i].1 <= i {
                hit_i += 1;
            }
            let in_hit = hit_i < hits.len() && i >= hits[hit_i].0 && i < hits[hit_i].1;
            let in_sel = i >= s0 && i < s1;
            let is_bracket = bracket_match == Some((y, i));
            let tok = toks.get(i).copied().unwrap_or(Tok::Normal);
            let want = if suspicious(c) {
                Style { fg: Color::Red, bg: curline_bg, rev: true }
            } else if in_sel {
                Style { fg: self.theme.selection_fg, bg: self.theme.selection_bg, rev: false }
            } else if is_bracket {
                Style { fg: self.theme.bracket_fg, bg: self.theme.bracket_bg, rev: false }
            } else if in_hit {
                Style { fg: self.theme.search_fg, bg: self.theme.search_bg, rev: false }
            } else {
                Style { fg: tok_color(tok, &self.theme), bg: curline_bg, rev: false }
            };
            if want != cur {
                queue!(out, SetAttribute(Attribute::Reset), SetForegroundColor(want.fg), SetBackgroundColor(want.bg))?;
                if want.rev {
                    queue!(out, SetAttribute(Attribute::Reverse))?;
                }
                cur = want;
            }
            if col < left || col + cw > right {
                // wide char or tab cut by the screen edge: draw only the visible part as spaces
                let vs = col.max(left);
                let ve = (col + cw).min(right);
                let n = ve.saturating_sub(vs);
                queue!(out, Print(" ".repeat(n)))?;
                drawn += n;
            } else if c == '\t' {
                queue!(out, Print(" ".repeat(cw)))?;
                drawn += cw;
            } else if suspicious(c) {
                queue!(out, Print('?'))?;
                drawn += 1;
            } else if c.is_control() {
                let ch = if (c as u32) < 0x80 { ((c as u8) ^ 0x40) as char } else { '?' };
                queue!(out, Print('^'), Print(ch))?;
                drawn += 2;
            } else {
                queue!(out, Print(c))?;
                drawn += cw;
            }
            col += cw;
        }
        // Show selected line break as one highlighted cell.
        if s1 == usize::MAX && col >= left && col < right && drawn < text_w {
            queue!(
                out,
                SetAttribute(Attribute::Reset),
                SetBackgroundColor(self.theme.selection_bg),
                Print(' ')
            )?;
            drawn += 1;
        }
        // Fill the remainder of the row with curline_bg so theme backgrounds never look like jagged text selections.
        let rem = text_w.saturating_sub(drawn);
        queue!(out, SetAttribute(Attribute::Reset), SetBackgroundColor(curline_bg))?;
        if rem > 0 {
            queue!(out, Print(" ".repeat(rem)))?;
        }
        if !self.scrollbar {
            queue!(out, Clear(ClearType::UntilNewLine))?;
        }
        queue!(out, ResetColor)?;
        Ok(())
    }
}
