//! Text buffer with compact, group-based undo/redo.
//! Pure data: no terminal code, no I/O, no panics on bad input
//! (every operation validates its coordinates and returns `None` if invalid).

pub type Line = Vec<char>;

/// Edit "kinds" decide how undo groups form.
pub const KIND_OTHER: u8 = 0; // always starts a new undo step
pub const KIND_TYPE: u8 = 1; // consecutive typing = one undo step
pub const KIND_DELETE: u8 = 2; // consecutive deleting = one undo step

const MAX_HISTORY: usize = 100_000;

#[derive(Debug)]
enum Op {
    Ins(usize, usize, Vec<char>),
    Del(usize, usize, Vec<char>),
    Split(usize, usize),
    Join(usize, usize),
    InsLines(usize, Vec<Line>),
    DelLines(usize, Vec<Line>),
}

struct Rec {
    op: Op,
    group: u64,
}

pub struct Buffer {
    pub lines: Vec<Line>,
    undo: Vec<Rec>,
    redo: Vec<Rec>,
    group: u64,
    last_kind: u8,
    atomic: u32,
    save_len: usize,
    dirty: usize,
}

impl Buffer {
    pub fn new(mut lines: Vec<Line>) -> Self {
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        Buffer {
            lines,
            undo: Vec::new(),
            redo: Vec::new(),
            group: 0,
            last_kind: KIND_OTHER,
            atomic: 0,
            save_len: 0,
            dirty: usize::MAX,
        }
    }

    // ---------- state ----------

    pub fn is_modified(&self) -> bool {
        self.undo.len() != self.save_len
    }

    pub fn mark_saved(&mut self) {
        self.save_len = self.undo.len();
        self.last_kind = KIND_OTHER;
    }

    /// Next edit starts a new undo step (call after cursor moves).
    pub fn break_group(&mut self) {
        self.last_kind = KIND_OTHER;
    }

    /// Everything between begin/end is ONE undo step. Nestable.
    pub fn begin_atomic(&mut self) {
        if self.atomic == 0 {
            self.group += 1;
        }
        self.atomic += 1;
    }

    pub fn end_atomic(&mut self) {
        self.atomic = self.atomic.saturating_sub(1);
        if self.atomic == 0 {
            self.last_kind = KIND_OTHER;
        }
    }

    /// First line changed since the last call (for the syntax cache).
    pub fn take_dirty(&mut self) -> usize {
        std::mem::replace(&mut self.dirty, usize::MAX)
    }

    // ---------- public edit API ----------

    pub fn insert(&mut self, y: usize, x: usize, text: &[char], kind: u8) -> Option<(usize, usize)> {
        if text.is_empty() {
            return Some((y, x));
        }
        self.exec(Op::Ins(y, x, text.to_vec()), kind)
    }

    pub fn delete(&mut self, y: usize, x: usize, n: usize, kind: u8) -> Option<(usize, usize)> {
        if n == 0 {
            return Some((y, x));
        }
        let end = x.checked_add(n)?;
        let line = self.lines.get(y)?;
        if end > line.len() {
            return None;
        }
        let s = line[x..end].to_vec();
        self.exec(Op::Del(y, x, s), kind)
    }

    pub fn split(&mut self, y: usize, x: usize) -> Option<(usize, usize)> {
        self.exec(Op::Split(y, x), KIND_OTHER)
    }

    /// Join line `y+1` onto line `y`.
    pub fn join(&mut self, y: usize) -> Option<(usize, usize)> {
        let x = self.lines.get(y)?.len();
        self.exec(Op::Join(y, x), KIND_OTHER)
    }

    pub fn insert_lines(&mut self, y: usize, lines: Vec<Line>) -> Option<(usize, usize)> {
        if lines.is_empty() {
            return Some((y, 0));
        }
        self.exec(Op::InsLines(y, lines), KIND_OTHER)
    }

    /// Delete `n` whole lines. Never deletes every line.
    pub fn delete_lines(&mut self, y: usize, n: usize) -> Option<(usize, usize)> {
        let end = y.checked_add(n)?;
        if n == 0 || end > self.lines.len() || n >= self.lines.len() {
            return None;
        }
        let removed = self.lines[y..end].to_vec();
        self.exec(Op::DelLines(y, removed), KIND_OTHER)
    }

    /// Delete the text between two positions (`a` < `b`) as one undo step.
    pub fn delete_range(&mut self, a: (usize, usize), b: (usize, usize)) {
        if a >= b || b.0 >= self.lines.len() {
            return;
        }
        if a.1 > self.lines[a.0].len() || b.1 > self.lines[b.0].len() {
            return;
        }
        self.begin_atomic();
        if a.0 == b.0 {
            self.delete(a.0, a.1, b.1 - a.1, KIND_OTHER);
        } else {
            if b.1 > 0 {
                self.delete(b.0, 0, b.1, KIND_OTHER);
            }
            let inner = b.0 - a.0 - 1;
            if inner > 0 {
                self.delete_lines(a.0 + 1, inner);
            }
            let tail = self.lines[a.0].len() - a.1;
            if tail > 0 {
                self.delete(a.0, a.1, tail, KIND_OTHER);
            }
            self.join(a.0);
        }
        self.end_atomic();
    }

    /// Insert multi-line text; returns the cursor position after it.
    pub fn insert_text(&mut self, y: usize, x: usize, text: &str) -> (usize, usize) {
        let y = y.min(self.lines.len() - 1);
        let x = x.min(self.lines[y].len());
        let parts: Vec<Vec<char>> = text
            .split('\n')
            .map(|s| s.chars().filter(|c| *c != '\r').collect())
            .collect();
        self.begin_atomic();
        let result;
        if parts.len() == 1 {
            self.insert(y, x, &parts[0], KIND_OTHER);
            result = (y, x + parts[0].len());
        } else {
            self.split(y, x);
            self.insert(y, x, &parts[0], KIND_OTHER);
            let last = parts.len() - 1;
            let mid: Vec<Line> = parts[1..last].to_vec();
            let mid_n = mid.len();
            if mid_n > 0 {
                self.insert_lines(y + 1, mid);
            }
            let ly = y + 1 + mid_n;
            self.insert(ly, 0, &parts[last], KIND_OTHER);
            result = (ly, parts[last].len());
        }
        self.end_atomic();
        result
    }

    pub fn text_range(&self, a: (usize, usize), b: (usize, usize)) -> String {
        let mut s = String::new();
        if a >= b || b.0 >= self.lines.len() {
            return s;
        }
        for y in a.0..=b.0 {
            let line = &self.lines[y];
            let from = if y == a.0 { a.1.min(line.len()) } else { 0 };
            let to = if y == b.0 { b.1.min(line.len()) } else { line.len() };
            if from < to {
                s.extend(line[from..to].iter());
            }
            if y != b.0 {
                s.push('\n');
            }
        }
        s
    }

    // ---------- undo / redo ----------

    pub fn undo(&mut self) -> Option<(usize, usize)> {
        let g = self.undo.last()?.group;
        let mut cur = None;
        while self.undo.last().map_or(false, |r| r.group == g) {
            let rec = self.undo.pop()?;
            if let Some(c) = self.apply(&rec.op, true) {
                cur = Some(c);
            }
            self.redo.push(rec);
        }
        self.last_kind = KIND_OTHER;
        cur
    }

    pub fn redo(&mut self) -> Option<(usize, usize)> {
        let g = self.redo.last()?.group;
        let mut cur = None;
        while self.redo.last().map_or(false, |r| r.group == g) {
            let rec = self.redo.pop()?;
            if let Some(c) = self.apply(&rec.op, false) {
                cur = Some(c);
            }
            self.undo.push(rec);
        }
        self.last_kind = KIND_OTHER;
        cur
    }

    // ---------- internals ----------

    fn exec(&mut self, op: Op, kind: u8) -> Option<(usize, usize)> {
        let r = self.apply(&op, false)?;
        self.push(op, kind);
        Some(r)
    }

    fn push(&mut self, op: Op, kind: u8) {
        if self.atomic == 0 && (kind == KIND_OTHER || kind != self.last_kind) {
            self.group += 1;
        }
        self.last_kind = kind;
        if !self.redo.is_empty() {
            if self.undo.len() < self.save_len {
                self.save_len = usize::MAX; // saved state is no longer reachable
            }
            self.redo.clear();
        }
        // Merge consecutive typing into one record (saves memory).
        if kind == KIND_TYPE && self.atomic == 0 {
            if let Op::Ins(y, x, s) = &op {
                if let Some(Rec { op: Op::Ins(py, px, ps), group }) = self.undo.last_mut() {
                    if *group == self.group && *py == *y && *px + ps.len() == *x {
                        ps.extend_from_slice(s);
                        return;
                    }
                }
            }
        }
        // Merge consecutive backspace/delete into one record (saves memory & undo steps)
        if kind == KIND_DELETE && self.atomic == 0 {
            if let Op::Del(y, x, s) = &op {
                if let Some(Rec { op: Op::Del(py, px, ps), group }) = self.undo.last_mut() {
                    if *group == self.group && *py == *y {
                        if *px == *x {
                            // Forward-delete: new deletion starts where last one started,
                            // meaning chars are being deleted from the same position (Delete key)
                            ps.extend_from_slice(s);
                            return;
                        } else if *px == x + s.len() {
                            // Backspace: each press deletes the char just before current,
                            // so new x = old x - 1 each time
                            let mut new_s: Vec<char> = s.clone();
                            new_s.extend_from_slice(ps);
                            *ps = new_s;
                            *px = *x;
                            return;
                        }
                    }
                }
            }
        }
        self.undo.push(Rec { op, group: self.group });
        if self.undo.len() > MAX_HISTORY {
            let n = MAX_HISTORY / 10;
            self.undo.drain(..n);
            self.save_len = if self.save_len == usize::MAX || self.save_len < n {
                usize::MAX
            } else {
                self.save_len - n
            };
        }
    }

    fn touch(&mut self, y: usize) {
        if y < self.dirty {
            self.dirty = y;
        }
    }

    fn apply(&mut self, op: &Op, rev: bool) -> Option<(usize, usize)> {
        match (op, rev) {
            (Op::Ins(y, x, s), false) | (Op::Del(y, x, s), true) => self.raw_ins(*y, *x, s),
            (Op::Del(y, x, s), false) | (Op::Ins(y, x, s), true) => self.raw_del(*y, *x, s.len()),
            (Op::Split(y, x), false) | (Op::Join(y, x), true) => self.raw_split(*y, *x),
            (Op::Join(y, x), false) | (Op::Split(y, x), true) => self.raw_join(*y, *x),
            (Op::InsLines(y, l), false) | (Op::DelLines(y, l), true) => self.raw_ins_lines(*y, l),
            (Op::DelLines(y, l), false) | (Op::InsLines(y, l), true) => self.raw_del_lines(*y, l.len()),
        }
    }

    fn raw_ins(&mut self, y: usize, x: usize, s: &[char]) -> Option<(usize, usize)> {
        let line = self.lines.get_mut(y)?;
        if x > line.len() {
            return None;
        }
        let _ = line.splice(x..x, s.iter().copied());
        self.touch(y);
        Some((y, x + s.len()))
    }

    fn raw_del(&mut self, y: usize, x: usize, n: usize) -> Option<(usize, usize)> {
        let line = self.lines.get_mut(y)?;
        let end = x.checked_add(n)?;
        if end > line.len() {
            return None;
        }
        line.drain(x..end);
        self.touch(y);
        Some((y, x))
    }

    fn raw_split(&mut self, y: usize, x: usize) -> Option<(usize, usize)> {
        let line = self.lines.get_mut(y)?;
        if x > line.len() {
            return None;
        }
        let tail = line.split_off(x);
        self.lines.insert(y + 1, tail);
        self.touch(y);
        Some((y + 1, 0))
    }

    fn raw_join(&mut self, y: usize, x: usize) -> Option<(usize, usize)> {
        if y + 1 >= self.lines.len() {
            return None;
        }
        let next = self.lines.remove(y + 1);
        self.lines[y].extend(next);
        self.touch(y);
        Some((y, x))
    }

    fn raw_ins_lines(&mut self, y: usize, l: &[Line]) -> Option<(usize, usize)> {
        if y > self.lines.len() {
            return None;
        }
        let _ = self.lines.splice(y..y, l.iter().cloned());
        self.touch(y);
        Some((y, 0))
    }

    fn raw_del_lines(&mut self, y: usize, n: usize) -> Option<(usize, usize)> {
        let end = y.checked_add(n)?;
        if end > self.lines.len() || n >= self.lines.len() {
            return None;
        }
        self.lines.drain(y..end);
        self.touch(y);
        Some((y.min(self.lines.len() - 1), 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> Buffer {
        Buffer::new(s.split('\n').map(|l| l.chars().collect()).collect())
    }
    fn text(b: &Buffer) -> String {
        b.lines.iter().map(|l| l.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn typing_is_one_undo_step() {
        let mut b = buf("");
        for (i, c) in "hello".chars().enumerate() {
            b.insert(0, i, &[c], KIND_TYPE);
        }
        assert_eq!(text(&b), "hello");
        b.undo();
        assert_eq!(text(&b), "");
        b.redo();
        assert_eq!(text(&b), "hello");
    }

    #[test]
    fn backspace_is_one_undo_step() {
        let mut b = buf("hello world");
        // Simulate 5 backspaces from the end
        for i in 0..5 {
            b.delete(0, 11 - 1 - i, 1, KIND_DELETE);
        }
        assert_eq!(text(&b), "hello ");
        b.undo();
        assert_eq!(text(&b), "hello world");
        b.redo();
        assert_eq!(text(&b), "hello ");
    }

    #[test]
    fn delete_range_multiline_and_undo() {
        let mut b = buf("abc\ndef\nghi\njkl");
        b.delete_range((0, 1), (3, 2));
        assert_eq!(text(&b), "al");
        b.undo();
        assert_eq!(text(&b), "abc\ndef\nghi\njkl");
        b.redo();
        assert_eq!(text(&b), "al");
    }

    #[test]
    fn insert_text_multiline() {
        let mut b = buf("ab");
        let pos = b.insert_text(0, 1, "X\nY\r\nZ");
        assert_eq!(text(&b), "aX\nY\nZb");
        assert_eq!(pos, (2, 1));
        b.undo();
        assert_eq!(text(&b), "ab");
    }

    #[test]
    fn modified_tracking() {
        let mut b = buf("x");
        assert!(!b.is_modified());
        b.insert(0, 1, &['y'], KIND_TYPE);
        assert!(b.is_modified());
        b.mark_saved();
        assert!(!b.is_modified());
        b.insert(0, 2, &['z'], KIND_TYPE); // must NOT merge into pre-save record
        assert!(b.is_modified());
        b.undo();
        assert!(!b.is_modified());
        assert_eq!(text(&b), "xy");
    }

    #[test]
    fn invalid_ops_do_not_panic() {
        let mut b = buf("abc");
        assert!(b.insert(5, 0, &['x'], 0).is_none());
        assert!(b.delete(0, 2, 9, 0).is_none());
        assert!(b.split(0, 99).is_none());
        assert!(b.join(0).is_none());
        assert!(b.delete_lines(0, 1).is_none()); // never delete the last line
        b.delete_range((9, 9), (10, 10));
        assert_eq!(text(&b), "abc");
    }

    #[test]
    fn text_range_works() {
        let b = buf("abc\ndef\nghi");
        assert_eq!(b.text_range((0, 1), (2, 2)), "bc\ndef\ngh");
    }
}
