//! A single-line text editor with readline (Emacs) keybindings and optional input history.

use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

// ── InputHistory ─────────────────────────────────────────────────────────

/// Stores past inputs and supports Ctrl+P / Ctrl+N navigation.
#[derive(Debug, Default)]
pub struct InputHistory {
    entries: Vec<String>,
    /// Current position in history. `None` means we are editing fresh input.
    cursor: Option<usize>,
    /// The draft text the user was typing before navigating history.
    draft: String,
}

impl InputHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry to the history. Resets navigation state.
    pub fn push(&mut self, entry: String) {
        if !entry.is_empty() {
            self.entries.push(entry);
        }
        self.cursor = None;
        self.draft.clear();
    }

    /// Navigate to the previous (older) history entry.
    /// `current_text` is the text currently in the editor (saved as draft on first call).
    /// Returns the text to display, or `None` if history is empty or already at oldest.
    pub fn up(&mut self, current_text: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        match self.cursor {
            None => {
                // First time navigating: save current text as draft
                self.draft = current_text.to_string();
                let idx = self.entries.len() - 1;
                self.cursor = Some(idx);
                Some(&self.entries[idx])
            }
            Some(0) => {
                // Already at oldest entry
                None
            }
            Some(ref mut idx) => {
                *idx -= 1;
                Some(&self.entries[*idx])
            }
        }
    }

    /// Navigate to the next (newer) history entry.
    /// Returns the text to display, or `None` if not navigating.
    pub fn down(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(idx) if idx >= self.entries.len() - 1 => {
                // Past the newest entry: restore draft
                self.cursor = None;
                Some(&self.draft)
            }
            Some(ref mut idx) => {
                *idx += 1;
                Some(&self.entries[*idx])
            }
        }
    }

    /// Reset navigation state without clearing entries.
    #[allow(dead_code)] // Public API, useful for callers
    pub fn reset(&mut self) {
        self.cursor = None;
        self.draft.clear();
    }
}

// ── LineEditorAction ─────────────────────────────────────────────────────

/// Result of `LineEditor::handle_key`.
#[derive(Debug, PartialEq, Eq)]
pub enum LineEditorAction {
    /// Text content changed (caller should sync / refilter).
    Changed,
    /// Enter was pressed (caller should submit).
    Submitted,
    /// Key was consumed but text did not change (e.g. cursor movement).
    Consumed,
    /// Key was not handled (caller can process it, e.g. Esc to close).
    Unhandled,
}

// ── LineEditor ───────────────────────────────────────────────────────────

/// A single-line text editor with Emacs/readline keybindings.
pub struct LineEditor {
    buf: String,
    /// Byte offset of the cursor within `buf`. Always on a char boundary.
    cursor: usize,
    /// Shared kill ring (Ctrl+K/U/W fill it, Ctrl+Y yanks from it).
    kill_ring: Rc<RefCell<String>>,
}

impl LineEditor {
    pub fn new(kill_ring: Rc<RefCell<String>>) -> Self {
        Self {
            buf: String::new(),
            cursor: 0,
            kill_ring,
        }
    }

    // ── Public accessors ─────────────────────────────────────────────

    pub fn text(&self) -> &str {
        &self.buf
    }

    /// Replace the buffer contents and move cursor to end.
    pub fn set_text(&mut self, s: &str) {
        self.buf = s.to_string();
        self.cursor = self.buf.len();
    }

    /// Clear the buffer and reset cursor.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
    }

    // ── Rendering ────────────────────────────────────────────────────

    /// Produce spans with a reversed block cursor at the current position.
    pub fn render_spans(&self, base_style: Style) -> Vec<Span<'_>> {
        let cursor_style = base_style.add_modifier(Modifier::REVERSED);
        if self.cursor >= self.buf.len() {
            // Cursor at end: show reversed space
            vec![
                Span::styled(self.buf.as_str(), base_style),
                Span::styled(" ", cursor_style),
            ]
        } else {
            let (before, rest) = self.buf.split_at(self.cursor);
            let cursor_char = rest.chars().next().unwrap();
            let after_start = self.cursor + cursor_char.len_utf8();
            vec![
                Span::styled(before, base_style),
                Span::styled(&self.buf[self.cursor..after_start], cursor_style),
                Span::styled(&self.buf[after_start..], base_style),
            ]
        }
    }

    // ── Key handling ─────────────────────────────────────────────────

    /// Process a key event. `history` is passed by the caller (may be `None`).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        history: Option<&mut InputHistory>,
    ) -> LineEditorAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            // ── Submit ───────────────────────────────────────────────
            KeyCode::Enter => LineEditorAction::Submitted,

            // ── History ──────────────────────────────────────────────
            KeyCode::Up if !ctrl && !alt => self.history_up(history),
            KeyCode::Down if !ctrl && !alt => self.history_down(history),
            KeyCode::Char('p') if ctrl => self.history_up(history),
            KeyCode::Char('n') if ctrl => self.history_down(history),

            // ── Cursor movement ──────────────────────────────────────
            KeyCode::Home => self.move_beginning(),
            KeyCode::Char('a') if ctrl => self.move_beginning(),
            KeyCode::End => self.move_end(),
            KeyCode::Char('e') if ctrl => self.move_end(),
            KeyCode::Right if alt => self.move_forward_word(),
            KeyCode::Right => self.move_forward_char(),
            KeyCode::Char('f') if ctrl => self.move_forward_char(),
            KeyCode::Left if alt => self.move_backward_word(),
            KeyCode::Left => self.move_backward_char(),
            KeyCode::Char('b') if ctrl => self.move_backward_char(),
            KeyCode::Char('f') if alt => self.move_forward_word(),
            KeyCode::Char('b') if alt => self.move_backward_word(),

            // ── Deletion ─────────────────────────────────────────────
            KeyCode::Backspace => self.delete_backward_char(),
            KeyCode::Char('h') if ctrl => self.delete_backward_char(),
            KeyCode::Delete => self.delete_forward_char(),
            KeyCode::Char('d') if ctrl => self.delete_forward_char(),
            KeyCode::Char('k') if ctrl => self.kill_to_end(),
            KeyCode::Char('u') if ctrl => self.kill_to_beginning(),
            KeyCode::Char('w') if ctrl => self.kill_word_backward(),
            KeyCode::Char('d') if alt => self.kill_word_forward(),

            // ── Transpose / Yank ─────────────────────────────────────
            KeyCode::Char('t') if ctrl => self.transpose_chars(),
            KeyCode::Char('y') if ctrl => self.yank(),

            // ── Character insertion ──────────────────────────────────
            KeyCode::Char(c) if !ctrl && !alt => {
                self.insert_char(c);
                LineEditorAction::Changed
            }

            // ── Unhandled ────────────────────────────────────────────
            _ => LineEditorAction::Unhandled,
        }
    }

    // ── Cursor movement helpers ──────────────────────────────────────

    fn move_beginning(&mut self) -> LineEditorAction {
        if self.cursor == 0 {
            return LineEditorAction::Consumed;
        }
        self.cursor = 0;
        LineEditorAction::Consumed
    }

    fn move_end(&mut self) -> LineEditorAction {
        if self.cursor == self.buf.len() {
            return LineEditorAction::Consumed;
        }
        self.cursor = self.buf.len();
        LineEditorAction::Consumed
    }

    fn move_forward_char(&mut self) -> LineEditorAction {
        if self.cursor >= self.buf.len() {
            return LineEditorAction::Consumed;
        }
        let c = self.buf[self.cursor..].chars().next().unwrap();
        self.cursor += c.len_utf8();
        LineEditorAction::Consumed
    }

    fn move_backward_char(&mut self) -> LineEditorAction {
        if self.cursor == 0 {
            return LineEditorAction::Consumed;
        }
        let c = self.buf[..self.cursor].chars().next_back().unwrap();
        self.cursor -= c.len_utf8();
        LineEditorAction::Consumed
    }

    fn move_forward_word(&mut self) -> LineEditorAction {
        self.cursor = self.next_word_end();
        LineEditorAction::Consumed
    }

    fn move_backward_word(&mut self) -> LineEditorAction {
        self.cursor = self.prev_word_start();
        LineEditorAction::Consumed
    }

    // ── Editing helpers ──────────────────────────────────────────────

    fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    fn delete_backward_char(&mut self) -> LineEditorAction {
        if self.cursor == 0 {
            return LineEditorAction::Consumed;
        }
        let c = self.buf[..self.cursor].chars().next_back().unwrap();
        let new_cursor = self.cursor - c.len_utf8();
        self.buf.remove(new_cursor);
        self.cursor = new_cursor;
        LineEditorAction::Changed
    }

    fn delete_forward_char(&mut self) -> LineEditorAction {
        if self.cursor >= self.buf.len() {
            return LineEditorAction::Consumed;
        }
        self.buf.remove(self.cursor);
        LineEditorAction::Changed
    }

    fn kill_to_end(&mut self) -> LineEditorAction {
        if self.cursor >= self.buf.len() {
            return LineEditorAction::Consumed;
        }
        let killed: String = self.buf[self.cursor..].to_string();
        self.buf.truncate(self.cursor);
        *self.kill_ring.borrow_mut() = killed;
        LineEditorAction::Changed
    }

    fn kill_to_beginning(&mut self) -> LineEditorAction {
        if self.cursor == 0 {
            return LineEditorAction::Consumed;
        }
        let killed: String = self.buf[..self.cursor].to_string();
        self.buf = self.buf[self.cursor..].to_string();
        self.cursor = 0;
        *self.kill_ring.borrow_mut() = killed;
        LineEditorAction::Changed
    }

    fn kill_word_backward(&mut self) -> LineEditorAction {
        if self.cursor == 0 {
            return LineEditorAction::Consumed;
        }
        let start = self.prev_word_start();
        let killed: String = self.buf[start..self.cursor].to_string();
        self.buf.replace_range(start..self.cursor, "");
        self.cursor = start;
        *self.kill_ring.borrow_mut() = killed;
        LineEditorAction::Changed
    }

    fn kill_word_forward(&mut self) -> LineEditorAction {
        if self.cursor >= self.buf.len() {
            return LineEditorAction::Consumed;
        }
        let end = self.next_word_end();
        let killed: String = self.buf[self.cursor..end].to_string();
        self.buf.replace_range(self.cursor..end, "");
        *self.kill_ring.borrow_mut() = killed;
        LineEditorAction::Changed
    }

    fn transpose_chars(&mut self) -> LineEditorAction {
        // Need at least 2 chars and cursor not at position 0
        if self.buf.len() < 2 {
            return LineEditorAction::Consumed;
        }
        // If cursor at end, transpose the two chars before cursor
        // If cursor in middle, transpose char before cursor with char at cursor
        let (a_start, a_end, b_start, b_end) = if self.cursor >= self.buf.len() {
            // Cursor at end: swap last two chars
            let last = self.buf[..self.cursor].chars().next_back().unwrap();
            let a_start = self.cursor - last.len_utf8();
            let penult = self.buf[..a_start].chars().next_back().unwrap();
            let b_start = a_start - penult.len_utf8();
            (b_start, b_start + penult.len_utf8(), a_start, self.cursor)
        } else if self.cursor == 0 {
            return LineEditorAction::Consumed;
        } else {
            let before = self.buf[..self.cursor].chars().next_back().unwrap();
            let a_start = self.cursor - before.len_utf8();
            let at_cursor = self.buf[self.cursor..].chars().next().unwrap();
            let b_end = self.cursor + at_cursor.len_utf8();
            (a_start, self.cursor, self.cursor, b_end)
        };

        let a: String = self.buf[a_start..a_end].to_string();
        let b: String = self.buf[b_start..b_end].to_string();
        let mut new = String::with_capacity(self.buf.len());
        new.push_str(&self.buf[..a_start]);
        new.push_str(&b);
        new.push_str(&a);
        new.push_str(&self.buf[b_end..]);
        self.buf = new;
        self.cursor = b_end;
        LineEditorAction::Changed
    }

    fn yank(&mut self) -> LineEditorAction {
        let text = self.kill_ring.borrow().clone();
        if text.is_empty() {
            return LineEditorAction::Consumed;
        }
        self.buf.insert_str(self.cursor, &text);
        self.cursor += text.len();
        LineEditorAction::Changed
    }

    // ── History helpers ──────────────────────────────────────────────

    fn history_up(&mut self, history: Option<&mut InputHistory>) -> LineEditorAction {
        let Some(history) = history else {
            return LineEditorAction::Unhandled;
        };
        if let Some(text) = history.up(&self.buf) {
            self.buf = text.to_string();
            self.cursor = self.buf.len();
            LineEditorAction::Changed
        } else {
            LineEditorAction::Consumed
        }
    }

    fn history_down(&mut self, history: Option<&mut InputHistory>) -> LineEditorAction {
        let Some(history) = history else {
            return LineEditorAction::Unhandled;
        };
        if let Some(text) = history.down() {
            self.buf = text.to_string();
            self.cursor = self.buf.len();
            LineEditorAction::Changed
        } else {
            LineEditorAction::Consumed
        }
    }

    // ── Word boundary helpers ────────────────────────────────────────

    /// Find the byte offset of the end of the next word from `self.cursor`.
    fn next_word_end(&self) -> usize {
        let s = &self.buf[self.cursor..];
        let mut chars = s.char_indices();
        // Skip non-word characters
        let mut offset = 0;
        for (i, c) in chars.by_ref() {
            if is_word_char(c) {
                offset = i;
                break;
            }
            offset = i + c.len_utf8();
        }
        // Skip word characters
        for (i, c) in self.buf[self.cursor + offset..].char_indices() {
            if !is_word_char(c) {
                return self.cursor + offset + i;
            }
        }
        self.buf.len()
    }

    /// Find the byte offset of the start of the previous word from `self.cursor`.
    fn prev_word_start(&self) -> usize {
        let s = &self.buf[..self.cursor];
        let mut chars = s.char_indices().rev();
        // Skip non-word characters (going backwards)
        let mut offset = self.cursor;
        for (i, c) in chars.by_ref() {
            if is_word_char(c) {
                offset = i + c.len_utf8();
                break;
            }
            offset = i;
        }
        // Skip word characters (going backwards)
        for (i, c) in self.buf[..offset].char_indices().rev() {
            if !is_word_char(c) {
                return i + c.len_utf8();
            }
        }
        0
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn kill_ring() -> Rc<RefCell<String>> {
        Rc::new(RefCell::new(String::new()))
    }

    fn editor(text: &str) -> LineEditor {
        let mut e = LineEditor::new(kill_ring());
        e.set_text(text);
        e
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn alt(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
    }

    // ── Character insertion ──────────────────────────────────────────

    #[test]
    fn insert_at_end() {
        let mut e = editor("abc");
        assert_eq!(
            e.handle_key(key(KeyCode::Char('d')), None),
            LineEditorAction::Changed
        );
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn insert_at_middle() {
        let mut e = editor("ac");
        e.cursor = 1;
        e.handle_key(key(KeyCode::Char('b')), None);
        assert_eq!(e.text(), "abc");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn insert_at_beginning() {
        let mut e = editor("bc");
        e.cursor = 0;
        e.handle_key(key(KeyCode::Char('a')), None);
        assert_eq!(e.text(), "abc");
        assert_eq!(e.cursor, 1);
    }

    // ── Cursor movement ──────────────────────────────────────────────

    #[test]
    fn ctrl_a_moves_to_beginning() {
        let mut e = editor("hello");
        assert_eq!(e.handle_key(ctrl('a'), None), LineEditorAction::Consumed);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn ctrl_e_moves_to_end() {
        let mut e = editor("hello");
        e.cursor = 0;
        assert_eq!(e.handle_key(ctrl('e'), None), LineEditorAction::Consumed);
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn ctrl_f_forward_char() {
        let mut e = editor("abc");
        e.cursor = 0;
        e.handle_key(ctrl('f'), None);
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn ctrl_b_backward_char() {
        let mut e = editor("abc");
        e.cursor = 2;
        e.handle_key(ctrl('b'), None);
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn right_arrow_forward() {
        let mut e = editor("abc");
        e.cursor = 0;
        e.handle_key(key(KeyCode::Right), None);
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn left_arrow_backward() {
        let mut e = editor("abc");
        e.cursor = 2;
        e.handle_key(key(KeyCode::Left), None);
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn forward_at_end_is_noop() {
        let mut e = editor("abc");
        e.handle_key(ctrl('f'), None);
        assert_eq!(e.cursor, 3);
    }

    #[test]
    fn backward_at_start_is_noop() {
        let mut e = editor("abc");
        e.cursor = 0;
        e.handle_key(ctrl('b'), None);
        assert_eq!(e.cursor, 0);
    }

    // ── Word movement ────────────────────────────────────────────────

    #[test]
    fn alt_f_forward_word() {
        let mut e = editor("hello world");
        e.cursor = 0;
        e.handle_key(alt('f'), None);
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn alt_b_backward_word() {
        let mut e = editor("hello world");
        e.handle_key(alt('b'), None);
        assert_eq!(e.cursor, 6);
    }

    #[test]
    fn alt_f_skips_non_word_chars() {
        let mut e = editor("hello   world");
        e.cursor = 5;
        e.handle_key(alt('f'), None);
        assert_eq!(e.cursor, 13);
    }

    #[test]
    fn alt_b_skips_non_word_chars() {
        let mut e = editor("hello   world");
        e.cursor = 8;
        e.handle_key(alt('b'), None);
        assert_eq!(e.cursor, 0);
    }

    // ── Deletion ─────────────────────────────────────────────────────

    #[test]
    fn backspace_deletes_backward() {
        let mut e = editor("abc");
        assert_eq!(
            e.handle_key(key(KeyCode::Backspace), None),
            LineEditorAction::Changed
        );
        assert_eq!(e.text(), "ab");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn backspace_mid_string() {
        let mut e = editor("abc");
        e.cursor = 2;
        e.handle_key(key(KeyCode::Backspace), None);
        assert_eq!(e.text(), "ac");
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut e = editor("abc");
        e.cursor = 0;
        assert_eq!(
            e.handle_key(key(KeyCode::Backspace), None),
            LineEditorAction::Consumed
        );
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn ctrl_d_deletes_forward() {
        let mut e = editor("abc");
        e.cursor = 1;
        assert_eq!(e.handle_key(ctrl('d'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "ac");
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn delete_key_forward() {
        let mut e = editor("abc");
        e.cursor = 0;
        e.handle_key(key(KeyCode::Delete), None);
        assert_eq!(e.text(), "bc");
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn ctrl_d_at_end_is_noop() {
        let mut e = editor("abc");
        assert_eq!(e.handle_key(ctrl('d'), None), LineEditorAction::Consumed);
        assert_eq!(e.text(), "abc");
    }

    // ── Kill / Yank ──────────────────────────────────────────────────

    #[test]
    fn ctrl_k_kills_to_end() {
        let kr = kill_ring();
        let mut e = LineEditor::new(kr.clone());
        e.set_text("hello world");
        e.cursor = 5;
        assert_eq!(e.handle_key(ctrl('k'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "hello");
        assert_eq!(*kr.borrow(), " world");
    }

    #[test]
    fn ctrl_u_kills_to_beginning() {
        let kr = kill_ring();
        let mut e = LineEditor::new(kr.clone());
        e.set_text("hello world");
        e.cursor = 5;
        assert_eq!(e.handle_key(ctrl('u'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), " world");
        assert_eq!(e.cursor, 0);
        assert_eq!(*kr.borrow(), "hello");
    }

    #[test]
    fn ctrl_w_kills_word_backward() {
        let kr = kill_ring();
        let mut e = LineEditor::new(kr.clone());
        e.set_text("hello world");
        assert_eq!(e.handle_key(ctrl('w'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "hello ");
        assert_eq!(*kr.borrow(), "world");
    }

    #[test]
    fn alt_d_kills_word_forward() {
        let kr = kill_ring();
        let mut e = LineEditor::new(kr.clone());
        e.set_text("hello world");
        e.cursor = 6;
        assert_eq!(e.handle_key(alt('d'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "hello ");
        assert_eq!(*kr.borrow(), "world");
    }

    #[test]
    fn ctrl_y_yanks() {
        let kr = kill_ring();
        *kr.borrow_mut() = "yanked".to_string();
        let mut e = LineEditor::new(kr);
        e.set_text("hello ");
        assert_eq!(e.handle_key(ctrl('y'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "hello yanked");
        assert_eq!(e.cursor, 12);
    }

    #[test]
    fn kill_then_yank_roundtrip() {
        let kr = kill_ring();
        let mut e = LineEditor::new(kr);
        e.set_text("hello world");
        e.cursor = 5;
        e.handle_key(ctrl('k'), None);
        assert_eq!(e.text(), "hello");
        e.handle_key(ctrl('y'), None);
        assert_eq!(e.text(), "hello world");
    }

    // ── Transpose ────────────────────────────────────────────────────

    #[test]
    fn transpose_at_end() {
        let mut e = editor("ab");
        assert_eq!(e.handle_key(ctrl('t'), None), LineEditorAction::Changed);
        assert_eq!(e.text(), "ba");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn transpose_in_middle() {
        let mut e = editor("abc");
        e.cursor = 1;
        e.handle_key(ctrl('t'), None);
        assert_eq!(e.text(), "bac");
        assert_eq!(e.cursor, 2);
    }

    #[test]
    fn transpose_single_char_is_noop() {
        let mut e = editor("a");
        assert_eq!(e.handle_key(ctrl('t'), None), LineEditorAction::Consumed);
        assert_eq!(e.text(), "a");
    }

    #[test]
    fn transpose_at_beginning_is_noop() {
        let mut e = editor("abc");
        e.cursor = 0;
        assert_eq!(e.handle_key(ctrl('t'), None), LineEditorAction::Consumed);
    }

    // ── UTF-8 safety ─────────────────────────────────────────────────

    #[test]
    fn multibyte_char_navigation() {
        let mut e = editor("aéb");
        assert_eq!(e.cursor, 4); // é is 2 bytes, total 4 bytes
        e.handle_key(ctrl('b'), None);
        assert_eq!(e.cursor, 3); // before 'b'
        e.handle_key(ctrl('b'), None);
        assert_eq!(e.cursor, 1); // before 'é'
        e.handle_key(ctrl('b'), None);
        assert_eq!(e.cursor, 0); // before 'a'
    }

    #[test]
    fn multibyte_delete_backward() {
        let mut e = editor("aé");
        e.handle_key(key(KeyCode::Backspace), None);
        assert_eq!(e.text(), "a");
        assert_eq!(e.cursor, 1);
    }

    #[test]
    fn multibyte_delete_forward() {
        let mut e = editor("éb");
        e.cursor = 0;
        e.handle_key(ctrl('d'), None);
        assert_eq!(e.text(), "b");
        assert_eq!(e.cursor, 0);
    }

    // ── History ──────────────────────────────────────────────────────

    #[test]
    fn history_up_and_down() {
        let mut hist = InputHistory::new();
        hist.push("first".into());
        hist.push("second".into());

        let mut e = editor("current");
        // Up: get "second"
        assert_eq!(
            e.handle_key(ctrl('p'), Some(&mut hist)),
            LineEditorAction::Changed
        );
        assert_eq!(e.text(), "second");

        // Up: get "first"
        e.handle_key(ctrl('p'), Some(&mut hist));
        assert_eq!(e.text(), "first");

        // Down: get "second"
        e.handle_key(ctrl('n'), Some(&mut hist));
        assert_eq!(e.text(), "second");

        // Down: restore draft "current"
        e.handle_key(ctrl('n'), Some(&mut hist));
        assert_eq!(e.text(), "current");
    }

    #[test]
    fn history_up_empty_is_consumed() {
        let mut hist = InputHistory::new();
        let mut e = editor("");
        assert_eq!(
            e.handle_key(ctrl('p'), Some(&mut hist)),
            LineEditorAction::Consumed
        );
    }

    #[test]
    fn history_without_history_is_unhandled() {
        let mut e = editor("");
        assert_eq!(
            e.handle_key(key(KeyCode::Up), None),
            LineEditorAction::Unhandled
        );
    }

    #[test]
    fn history_up_at_oldest_is_consumed() {
        let mut hist = InputHistory::new();
        hist.push("only".into());

        let mut e = editor("");
        e.handle_key(ctrl('p'), Some(&mut hist));
        assert_eq!(e.text(), "only");

        // Already at oldest
        assert_eq!(
            e.handle_key(ctrl('p'), Some(&mut hist)),
            LineEditorAction::Consumed
        );
        assert_eq!(e.text(), "only");
    }

    #[test]
    fn history_preserves_draft() {
        let mut hist = InputHistory::new();
        hist.push("old".into());

        let mut e = editor("my draft");
        e.handle_key(ctrl('p'), Some(&mut hist));
        assert_eq!(e.text(), "old");

        e.handle_key(ctrl('n'), Some(&mut hist));
        assert_eq!(e.text(), "my draft");
    }

    // ── Submit ───────────────────────────────────────────────────────

    #[test]
    fn enter_returns_submitted() {
        let mut e = editor("test");
        assert_eq!(
            e.handle_key(key(KeyCode::Enter), None),
            LineEditorAction::Submitted
        );
    }

    // ── Unhandled keys ───────────────────────────────────────────────

    #[test]
    fn esc_is_unhandled() {
        let mut e = editor("");
        assert_eq!(
            e.handle_key(key(KeyCode::Esc), None),
            LineEditorAction::Unhandled
        );
    }

    #[test]
    fn tab_is_unhandled() {
        let mut e = editor("");
        assert_eq!(
            e.handle_key(key(KeyCode::Tab), None),
            LineEditorAction::Unhandled
        );
    }

    // ── Rendering ────────────────────────────────────────────────────

    #[test]
    fn render_cursor_at_end() {
        let e = editor("hello");
        let spans = e.render_spans(Style::default());
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "hello");
        assert_eq!(spans[1].content, " ");
    }

    #[test]
    fn render_cursor_in_middle() {
        let mut e = editor("hello");
        e.cursor = 2;
        let spans = e.render_spans(Style::default());
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "he");
        assert_eq!(spans[1].content, "l");
        assert_eq!(spans[2].content, "lo");
    }

    #[test]
    fn render_cursor_at_start() {
        let mut e = editor("hello");
        e.cursor = 0;
        let spans = e.render_spans(Style::default());
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "");
        assert_eq!(spans[1].content, "h");
        assert_eq!(spans[2].content, "ello");
    }

    #[test]
    fn render_empty() {
        let e = editor("");
        let spans = e.render_spans(Style::default());
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "");
        assert_eq!(spans[1].content, " ");
    }

    // ── Home / End keys ──────────────────────────────────────────────

    #[test]
    fn home_moves_to_beginning() {
        let mut e = editor("hello");
        e.handle_key(key(KeyCode::Home), None);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn end_moves_to_end() {
        let mut e = editor("hello");
        e.cursor = 0;
        e.handle_key(key(KeyCode::End), None);
        assert_eq!(e.cursor, 5);
    }

    // ── set_text / clear ─────────────────────────────────────────────

    #[test]
    fn set_text_puts_cursor_at_end() {
        let mut e = editor("");
        e.set_text("new text");
        assert_eq!(e.text(), "new text");
        assert_eq!(e.cursor, 8);
    }

    #[test]
    fn clear_resets_everything() {
        let mut e = editor("hello");
        e.clear();
        assert_eq!(e.text(), "");
        assert_eq!(e.cursor, 0);
    }
}
