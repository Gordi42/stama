//! The fullscreen live log view.
//!
//! Opened with `L` from the job overview (or via the job actions
//! menu), it shows the log file of the selected job with scrollback
//! and follows the file as new content is appended: each refresh tick
//! the app sends the viewer's [`LogFollowRequest`] (path + consumed
//! file offset) to the background worker, which answers with the bytes
//! appended since then (see `Scheduler::log_since`). The accumulated
//! buffer is capped; when the cap drops old lines the title shows a
//! "truncated" marker.
//!
//! Search is a case-insensitive substring match (regex is out of
//! scope); long lines are clipped at the right edge (no wrapping).

use crossterm::event::{KeyCode, KeyEvent, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::menus::help::HelpContext;
use crate::menus::{Menu, OpenMenu};
use crate::mouse_input::MouseInput;
use crate::update_content::{LogFollowRequest, LogFollowUpdate};

/// The maximum number of buffered lines; the oldest are dropped.
const MAX_BUFFER_LINES: usize = 10_000;
/// The maximum total size of the buffered lines (approximate, in
/// bytes); the oldest lines are dropped first.
const MAX_BUFFER_BYTES: usize = 2 * 1024 * 1024;
/// How many lines one mouse-wheel step scrolls.
const WHEEL_SCROLL_LINES: usize = 3;

pub struct LogViewer {
    open: bool,
    /// The log file the viewer follows.
    pub path: String,
    /// The buffered log lines (the newest at the end).
    lines: Vec<String>,
    /// Whether the last buffered line is still incomplete (the file
    /// did not end with a newline); the next chunk continues it.
    partial_line: bool,
    /// The file offset consumed so far; `None` before the initial read.
    offset: Option<u64>,
    /// Whether the buffer cap dropped old lines (shown in the title).
    dropped: bool,
    /// Follow mode: auto-scroll to the bottom when new content arrives.
    follow: bool,
    /// The index of the first visible line.
    scroll: usize,
    /// The height of the log area in the last render (for paging).
    view_height: usize,
    /// Whether the last poll could not read the file (missing/unreadable).
    waiting: bool,
    /// Whether at least one successful read happened.
    loaded: bool,
    // ---- search ----
    /// Whether the search prompt is open (keys edit the input).
    search_mode: bool,
    /// The text typed into the search prompt.
    search_input: String,
    /// The confirmed search pattern (lowercased; empty = no search).
    pattern: String,
    /// The line index of the current match.
    current_match: Option<usize>,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Default for LogViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewer {
    pub fn new() -> Self {
        Self {
            open: false,
            path: String::new(),
            lines: Vec::new(),
            partial_line: false,
            offset: None,
            dropped: false,
            follow: true,
            scroll: 0,
            view_height: 0,
            waiting: false,
            loaded: false,
            search_mode: false,
            search_input: String::new(),
            pattern: String::new(),
            current_match: None,
        }
    }
}

// ====================================================================
//  METHODS (opening, data flow)
// ====================================================================

impl LogViewer {
    /// Opens the viewer on the given log file, starting in follow mode
    /// with an empty buffer; the next refresh tick loads the initial
    /// scrollback (`offset: None` requests the tail read).
    pub fn activate(&mut self, path: &str) {
        *self = Self::new();
        self.path = path.to_string();
        self.open = true;
    }

    pub fn deactivate(&mut self) {
        self.open = false;
    }

    /// What the next background worker run should read for this view;
    /// `None` while the viewer is closed.
    pub fn follow_request(&self) -> Option<LogFollowRequest> {
        self.open.then(|| LogFollowRequest {
            path: self.path.clone(),
            offset: self.offset,
        })
    }

    /// Applies an incremental read delivered by the background worker.
    /// Stale updates (viewer closed, or an answer for a previously
    /// followed file) are ignored.
    pub fn apply_update(&mut self, update: LogFollowUpdate) {
        if !self.open || update.path != self.path {
            return;
        }
        let chunk = match update.chunk {
            Some(chunk) => chunk,
            None => {
                // the file is missing or unreadable: keep the buffer,
                // show the waiting hint and keep polling
                self.waiting = true;
                return;
            }
        };
        self.waiting = false;
        self.loaded = true;
        if chunk.truncated {
            // the chunk is not contiguous with the buffer (the file
            // shrank or more than the read cap was appended)
            self.lines.clear();
            self.partial_line = false;
            self.current_match = None;
            self.scroll = 0;
        }
        self.append_text(&chunk.content);
        self.offset = Some(chunk.offset);
        if self.follow {
            self.scroll_to_bottom();
        } else {
            self.clamp_scroll();
        }
    }

    /// Appends raw chunk text to the line buffer, continuing an
    /// incomplete last line and enforcing the buffer cap.
    fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let ends_with_newline = text.ends_with('\n');
        let mut parts = text.split('\n');
        if let Some(first) = parts.next() {
            match (self.partial_line, self.lines.last_mut()) {
                (true, Some(last)) => last.push_str(first),
                _ => self.lines.push(first.to_string()),
            }
        }
        for part in parts {
            self.lines.push(part.to_string());
        }
        if ends_with_newline {
            // split("a\n") yields a trailing ""; drop that empty
            // "next line" and remember that the last line is complete
            self.lines.pop();
        }
        self.partial_line = !ends_with_newline;
        self.enforce_cap();
    }

    /// Drops the oldest lines when the buffer exceeds the line or byte
    /// cap, keeping the scroll position and current match anchored to
    /// the same content.
    fn enforce_cap(&mut self) {
        let mut drop = self.lines.len().saturating_sub(MAX_BUFFER_LINES);
        let mut bytes: usize = self.lines.iter().map(|line| line.len()).sum();
        while bytes > MAX_BUFFER_BYTES && drop < self.lines.len() {
            bytes -= self.lines[drop].len();
            drop += 1;
        }
        if drop == 0 {
            return;
        }
        self.lines.drain(..drop);
        self.dropped = true;
        self.scroll = self.scroll.saturating_sub(drop);
        self.current_match = match self.current_match {
            Some(line) if line >= drop => Some(line - drop),
            _ => None,
        };
    }

    /// The buffered log lines (used by tests).
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

// ====================================================================
//  METHODS (scrolling, search)
// ====================================================================

impl LogViewer {
    /// The largest valid scroll offset (bottom of the log).
    fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.view_height.max(1))
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.min(self.max_scroll());
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll = self.max_scroll();
    }

    /// Scrolls up; any manual upward scroll leaves follow mode.
    fn scroll_up(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
        self.follow = false;
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll = (self.scroll + amount).min(self.max_scroll());
    }

    /// Jumps to the bottom and (re-)enables follow mode (`G`).
    fn go_to_bottom(&mut self) {
        self.scroll_to_bottom();
        self.follow = true;
    }

    /// Toggles follow mode (`f`); enabling it jumps to the bottom.
    fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            self.scroll_to_bottom();
        }
    }

    /// Whether the given line matches the confirmed pattern
    /// (case-insensitive substring).
    fn line_matches(&self, index: usize) -> bool {
        self.lines[index].to_lowercase().contains(&self.pattern)
    }

    /// The number of matching lines.
    fn match_count(&self) -> usize {
        if self.pattern.is_empty() {
            return 0;
        }
        (0..self.lines.len())
            .filter(|&index| self.line_matches(index))
            .count()
    }

    /// Confirms the search prompt: stores the pattern and jumps to the
    /// first match at/after the top of the view.
    fn confirm_search(&mut self) {
        self.search_mode = false;
        self.pattern = self.search_input.to_lowercase();
        self.current_match = None;
        self.search_step(1);
    }

    /// Jumps to the next (`direction = 1`) or previous (`-1`) matching
    /// line, wrapping around the buffer.
    fn search_step(&mut self, direction: isize) {
        if self.pattern.is_empty() || self.lines.is_empty() {
            return;
        }
        let count = self.lines.len() as isize;
        // continue from the current match, or start at the top of the
        // view when there is none yet
        let start = match self.current_match {
            Some(current) => (current as isize + direction).rem_euclid(count),
            None => self.scroll.min(self.lines.len() - 1) as isize,
        };
        for step in 0..count {
            let index = (start + step * direction).rem_euclid(count) as usize;
            if self.line_matches(index) {
                self.jump_to(index);
                return;
            }
        }
        self.current_match = None;
    }

    /// Scrolls so the given line is visible and marks it as the
    /// current match (leaves follow mode: the user is inspecting).
    fn jump_to(&mut self, line: usize) {
        self.current_match = Some(line);
        self.follow = false;
        let height = self.view_height.max(1);
        if line < self.scroll || line >= self.scroll + height {
            self.scroll = line.saturating_sub(height / 2);
        }
        self.clamp_scroll();
    }
}

// ====================================================================
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for LogViewer {
    fn is_open(&self) -> bool {
        self.open
    }

    /// Renders the log view over the whole frame (not a centered
    /// popup: the log gets all the space the terminal has).
    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let rect = f.area();
        f.render_widget(Clear, rect);

        // ---- title and border ----
        let mut title = format!(" {} ", self.path);
        if self.dropped {
            title.push_str("(truncated) ");
        }
        let follow_hint = if self.follow {
            " FOLLOW (f to stop) "
        } else {
            " f/G to follow "
        };
        let mut block = Block::default()
            .title_top(Line::from(title).alignment(Alignment::Left))
            .title_top(Line::from(follow_hint).alignment(Alignment::Right))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue))
            .title_bottom(Line::from(" Esc/q/L: close  ?: help ").alignment(Alignment::Right));

        // ---- search bar (bottom title) ----
        if self.search_mode {
            let bar = format!(" /{}▏", self.search_input);
            block = block.title_bottom(
                Line::from(bar)
                    .alignment(Alignment::Left)
                    .style(Style::default().fg(Color::Yellow)),
            );
        } else if !self.pattern.is_empty() {
            let bar = format!(" /{} ({} matches, n/N) ", self.pattern, self.match_count());
            block = block.title_bottom(Line::from(bar).alignment(Alignment::Left));
        }

        let inner = block.inner(rect);
        f.render_widget(block, rect);
        self.view_height = inner.height as usize;

        // ---- body ----
        if self.lines.is_empty() {
            let text = if self.waiting || !self.loaded {
                "waiting for log file..."
            } else {
                "(the log file is empty)"
            };
            let paragraph = Paragraph::new(text)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            f.render_widget(paragraph, inner);
            return;
        }

        // keep the bottom line visible in follow mode, whatever the
        // current terminal size is
        if self.follow {
            self.scroll_to_bottom();
        }
        self.clamp_scroll();

        let end = (self.scroll + self.view_height).min(self.lines.len());
        let visible: Vec<Line> = (self.scroll..end)
            .map(|index| {
                let line = Line::from(self.lines[index].clone());
                if Some(index) == self.current_match {
                    line.style(Style::default().reversed())
                } else if !self.pattern.is_empty() && self.line_matches(index) {
                    line.style(Style::default().fg(Color::Yellow))
                } else {
                    line
                }
            })
            .collect();
        // no wrapping: long lines are clipped at the right edge
        f.render_widget(Paragraph::new(visible), inner);
    }

    /// Handle user input for the log view.
    /// Always returns true (the fullscreen view consumes all input).
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        // the search prompt captures every key while it is open
        if self.search_mode {
            match key_event.code {
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_input.clear();
                }
                KeyCode::Enter => {
                    self.confirm_search();
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                }
                _ => {}
            }
            return true;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('L') => {
                self.deactivate();
            }
            // line scrolling
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
            }
            // half/full page scrolling
            KeyCode::Char('d') => {
                self.scroll_down(self.view_height.max(2) / 2);
            }
            KeyCode::Char('u') => {
                self.scroll_up(self.view_height.max(2) / 2);
            }
            KeyCode::PageDown => {
                self.scroll_down(self.view_height.max(1));
            }
            KeyCode::PageUp => {
                self.scroll_up(self.view_height.max(1));
            }
            // top / bottom
            KeyCode::Char('g') => {
                self.scroll = 0;
                self.follow = false;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.go_to_bottom();
            }
            // follow mode
            KeyCode::Char('f') => {
                self.toggle_follow();
            }
            // search
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.search_input.clear();
            }
            KeyCode::Char('n') => {
                self.search_step(1);
            }
            KeyCode::Char('N') => {
                self.search_step(-1);
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(HelpContext::LogView));
            }
            _ => {}
        }
        true
    }

    fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        if let Some(event_kind) = mouse_input.kind() {
            match event_kind {
                MouseEventKind::ScrollUp => {
                    self.scroll_up(WHEEL_SCROLL_LINES);
                }
                MouseEventKind::ScrollDown => {
                    self.scroll_down(WHEEL_SCROLL_LINES);
                }
                // the view is fullscreen: there is no "outside" to
                // click, so clicks are simply swallowed
                _ => {}
            }
            mouse_input.handled = true;
        }
    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::LogChunk;
    use crossterm::event::KeyModifiers;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// A viewer that is open on "/logs/run.log" with the given lines
    /// applied as one initial chunk and a view height of 5.
    fn viewer_with_lines(count: usize) -> LogViewer {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        viewer.view_height = 5;
        let content: String = (0..count).map(|i| format!("line {}\n", i)).collect();
        viewer.apply_update(update_for(&viewer, &content, content.len() as u64, false));
        viewer
    }

    fn update_for(
        viewer: &LogViewer,
        content: &str,
        offset: u64,
        truncated: bool,
    ) -> LogFollowUpdate {
        LogFollowUpdate {
            path: viewer.path.clone(),
            chunk: Some(LogChunk {
                content: content.to_string(),
                offset,
                truncated,
            }),
        }
    }

    // ----------------------------------------------------------------
    // follow mode
    // ----------------------------------------------------------------

    #[test]
    fn test_follow_mode_auto_scrolls_to_new_content() {
        let mut viewer = viewer_with_lines(20);
        // the viewer opens in follow mode and sticks to the bottom
        assert!(viewer.follow);
        assert_eq!(viewer.scroll, 15);

        // new content arrives: the view follows to the new bottom
        viewer.apply_update(update_for(&viewer, "more 1\nmore 2\n", 999, false));
        assert_eq!(viewer.lines.len(), 22);
        assert_eq!(viewer.scroll, 17);
        assert_eq!(viewer.offset, Some(999));
    }

    #[test]
    fn test_manual_scroll_up_disables_follow() {
        let mut viewer = viewer_with_lines(20);
        let mut action = Action::None;

        viewer.input(&mut action, key(KeyCode::Char('k')));
        assert!(!viewer.follow);
        assert_eq!(viewer.scroll, 14);

        // new content no longer moves the view
        viewer.apply_update(update_for(&viewer, "more\n", 999, false));
        assert_eq!(viewer.scroll, 14);

        // page keys and the wheel disable follow as well
        let mut viewer = viewer_with_lines(20);
        viewer.input(&mut action, key(KeyCode::PageUp));
        assert!(!viewer.follow);
        let mut viewer = viewer_with_lines(20);
        viewer.input(&mut action, key(KeyCode::Char('u')));
        assert!(!viewer.follow);
        // g jumps to the top and leaves follow mode
        let mut viewer = viewer_with_lines(20);
        viewer.input(&mut action, key(KeyCode::Char('g')));
        assert!(!viewer.follow);
        assert_eq!(viewer.scroll, 0);
    }

    #[test]
    fn test_g_and_f_reenable_follow() {
        let mut viewer = viewer_with_lines(20);
        let mut action = Action::None;

        // G: back to the bottom, following again
        viewer.input(&mut action, key(KeyCode::Char('k')));
        assert!(!viewer.follow);
        viewer.input(&mut action, key(KeyCode::Char('G')));
        assert!(viewer.follow);
        assert_eq!(viewer.scroll, 15);

        // f: toggles follow off and on (on jumps to the bottom)
        viewer.input(&mut action, key(KeyCode::Char('f')));
        assert!(!viewer.follow);
        viewer.input(&mut action, key(KeyCode::Char('k')));
        viewer.input(&mut action, key(KeyCode::Char('f')));
        assert!(viewer.follow);
        assert_eq!(viewer.scroll, 15);
    }

    #[test]
    fn test_scrolling_stays_in_bounds() {
        let mut viewer = viewer_with_lines(20);
        let mut action = Action::None;

        // scrolling down past the bottom clamps
        viewer.input(&mut action, key(KeyCode::Char('j')));
        assert_eq!(viewer.scroll, 15);
        // scrolling up past the top clamps at 0
        for _ in 0..40 {
            viewer.input(&mut action, key(KeyCode::Char('k')));
        }
        assert_eq!(viewer.scroll, 0);
        // half and full pages
        viewer.input(&mut action, key(KeyCode::Char('d')));
        assert_eq!(viewer.scroll, 2);
        viewer.input(&mut action, key(KeyCode::PageDown));
        assert_eq!(viewer.scroll, 7);
    }

    // ----------------------------------------------------------------
    // buffer handling
    // ----------------------------------------------------------------

    #[test]
    fn test_partial_lines_are_continued_across_chunks() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");

        viewer.apply_update(update_for(&viewer, "complete\npartial", 16, false));
        assert_eq!(viewer.lines(), ["complete", "partial"]);

        // the continuation is glued to the incomplete line
        viewer.apply_update(update_for(&viewer, " continued\nnext\n", 32, false));
        assert_eq!(viewer.lines(), ["complete", "partial continued", "next"]);

        // a complete last line is not glued to
        viewer.apply_update(update_for(&viewer, "fresh\n", 38, false));
        assert_eq!(
            viewer.lines(),
            ["complete", "partial continued", "next", "fresh"]
        );
    }

    #[test]
    fn test_buffer_cap_drops_oldest_lines() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        viewer.view_height = 5;

        let content: String = (0..MAX_BUFFER_LINES + 50)
            .map(|i| format!("line {}\n", i))
            .collect();
        viewer.apply_update(update_for(&viewer, &content, content.len() as u64, false));

        // the oldest 50 lines were dropped and the drop is flagged
        assert_eq!(viewer.lines().len(), MAX_BUFFER_LINES);
        assert_eq!(viewer.lines()[0], "line 50");
        assert!(viewer.dropped);
        // the title shows the truncation marker
        let content = render_to_string(&mut viewer, 80, 10);
        assert!(content.contains("(truncated)"));
    }

    #[test]
    fn test_byte_cap_drops_oldest_lines() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");

        // 1100 lines of ~4 KiB exceed the 2 MiB byte cap well before
        // the line cap
        let line = "x".repeat(4095) + "\n";
        let content = line.repeat(1100);
        viewer.apply_update(update_for(&viewer, &content, content.len() as u64, false));

        assert!(viewer.dropped);
        let bytes: usize = viewer.lines().iter().map(|l| l.len()).sum();
        assert!(bytes <= MAX_BUFFER_BYTES);
        assert!(!viewer.lines().is_empty());
    }

    #[test]
    fn test_truncated_chunk_resets_the_buffer() {
        let mut viewer = viewer_with_lines(20);
        // the file shrank: the worker signals a non-contiguous chunk
        viewer.apply_update(update_for(&viewer, "restarted\n", 10, true));
        assert_eq!(viewer.lines(), ["restarted"]);
        assert_eq!(viewer.offset, Some(10));
    }

    #[test]
    fn test_missing_file_keeps_waiting_and_stale_updates_are_ignored() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");

        // an unreadable file: chunk None -> waiting, offset untouched
        viewer.apply_update(LogFollowUpdate {
            path: "/logs/run.log".to_string(),
            chunk: None,
        });
        assert!(viewer.waiting);
        assert_eq!(viewer.offset, None);
        // the body shows the waiting hint
        let content = render_to_string(&mut viewer, 60, 8);
        assert!(content.contains("waiting for log file..."));

        // an answer for a different (previously followed) file is ignored
        viewer.apply_update(LogFollowUpdate {
            path: "/logs/other.log".to_string(),
            chunk: Some(LogChunk {
                content: "bogus\n".to_string(),
                offset: 6,
                truncated: false,
            }),
        });
        assert!(viewer.lines().is_empty());

        // a closed viewer requests nothing and applies nothing
        viewer.deactivate();
        assert_eq!(viewer.follow_request(), None);
        viewer.apply_update(update_for(&viewer, "late\n", 5, false));
        assert!(viewer.lines().is_empty());
    }

    #[test]
    fn test_follow_request_carries_path_and_offset() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        assert_eq!(
            viewer.follow_request(),
            Some(LogFollowRequest {
                path: "/logs/run.log".to_string(),
                offset: None,
            })
        );
        viewer.apply_update(update_for(&viewer, "a\n", 2, false));
        assert_eq!(viewer.follow_request().unwrap().offset, Some(2));
    }

    // ----------------------------------------------------------------
    // search
    // ----------------------------------------------------------------

    /// Types a pattern into the search prompt and confirms it.
    fn search(viewer: &mut LogViewer, pattern: &str) {
        let mut action = Action::None;
        viewer.input(&mut action, key(KeyCode::Char('/')));
        for c in pattern.chars() {
            viewer.input(&mut action, key(KeyCode::Char(c)));
        }
        viewer.input(&mut action, key(KeyCode::Enter));
    }

    #[test]
    fn test_search_finds_matches_and_n_cycles() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        viewer.view_height = 3;
        viewer.apply_update(update_for(
            &viewer,
            "alpha\nERROR: one\nbeta\nerror: two\ngamma\nError three\n",
            99,
            false,
        ));
        let mut action = Action::None;

        // case-insensitive substring search; Enter jumps to the first
        // match at/after the top of the view (line 3 here: the viewer
        // followed to the bottom, so lines 3..6 are visible)
        assert_eq!(viewer.scroll, 3);
        search(&mut viewer, "error");
        assert_eq!(viewer.current_match, Some(3));
        assert!(!viewer.follow);

        // n cycles forward (wrapping), N cycles backwards
        viewer.input(&mut action, key(KeyCode::Char('n')));
        assert_eq!(viewer.current_match, Some(5));
        viewer.input(&mut action, key(KeyCode::Char('n')));
        assert_eq!(viewer.current_match, Some(1)); // wrapped
        viewer.input(&mut action, key(KeyCode::Char('n')));
        assert_eq!(viewer.current_match, Some(3));
        viewer.input(&mut action, key(KeyCode::Char('N')));
        assert_eq!(viewer.current_match, Some(1));

        // the view scrolled so the current match is visible
        assert!(viewer.scroll <= 1 && 1 < viewer.scroll + viewer.view_height);
    }

    #[test]
    fn test_search_without_match_and_prompt_editing() {
        let mut viewer = viewer_with_lines(5);
        let mut action = Action::None;

        // no match: no current match, view position untouched
        search(&mut viewer, "no such text");
        assert_eq!(viewer.current_match, None);

        // Backspace edits the prompt, Esc cancels it without searching
        viewer.input(&mut action, key(KeyCode::Char('/')));
        viewer.input(&mut action, key(KeyCode::Char('x')));
        viewer.input(&mut action, key(KeyCode::Backspace));
        assert_eq!(viewer.search_input, "");
        viewer.input(&mut action, key(KeyCode::Esc));
        assert!(!viewer.search_mode);
        // Esc in the prompt does not close the viewer
        assert!(viewer.is_open());
    }

    // ----------------------------------------------------------------
    // opening / closing
    // ----------------------------------------------------------------

    #[test]
    fn test_close_keys_and_help_action() {
        for close_key in [KeyCode::Esc, KeyCode::Char('q'), KeyCode::Char('L')] {
            let mut viewer = viewer_with_lines(3);
            let mut action = Action::None;
            assert!(viewer.input(&mut action, key(close_key)));
            assert!(!viewer.is_open());
        }

        // ? opens the help menu with the log view context
        let mut viewer = viewer_with_lines(3);
        let mut action = Action::None;
        viewer.input(&mut action, key(KeyCode::Char('?')));
        assert!(matches!(
            action,
            Action::OpenMenu(OpenMenu::Help(HelpContext::LogView))
        ));
    }

    #[test]
    fn test_reactivation_resets_the_state() {
        let mut viewer = viewer_with_lines(20);
        search(&mut viewer, "line");
        viewer.deactivate();

        viewer.activate("/logs/next.log");
        assert!(viewer.is_open());
        assert_eq!(viewer.path, "/logs/next.log");
        assert!(viewer.lines().is_empty());
        assert!(viewer.follow);
        assert_eq!(viewer.offset, None);
        assert!(viewer.pattern.is_empty());
    }

    // ----------------------------------------------------------------
    // rendering
    // ----------------------------------------------------------------

    /// Renders the viewer into a test terminal and returns the buffer
    /// content as one string.
    fn render_to_string(viewer: &mut LogViewer, width: u16, height: u16) -> String {
        render_terminal(viewer, width, height)
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn render_terminal(viewer: &mut LogViewer, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                viewer.render(f, &area);
            })
            .unwrap();
        terminal
    }

    #[test]
    fn test_render_fullscreen_shows_title_and_content() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        viewer.apply_update(update_for(&viewer, "first line\nsecond line\n", 23, false));

        let content = render_to_string(&mut viewer, 60, 10);
        // the block is titled with the log path
        assert!(content.contains("/logs/run.log"));
        // the log content is rendered
        assert!(content.contains("first line"));
        assert!(content.contains("second line"));
        // follow mode is indicated
        assert!(content.contains("FOLLOW"));
    }

    #[test]
    fn test_render_follow_keeps_bottom_visible_on_small_terminal() {
        let mut viewer = LogViewer::new();
        viewer.activate("/logs/run.log");
        let content: String = (0..50).map(|i| format!("line {:02}\n", i)).collect();
        viewer.apply_update(update_for(&viewer, &content, 99, false));

        // 8 rows: borders leave 6 content lines; follow shows the tail
        let content = render_to_string(&mut viewer, 40, 8);
        assert!(content.contains("line 49"));
        assert!(!content.contains("line 00"));
    }

    #[test]
    fn test_snapshot_log_view() {
        let mut viewer = LogViewer::new();
        viewer.activate("/work/logs/train_model-424242.out");
        viewer.apply_update(update_for(
            &viewer,
            "epoch 1/5: loss 0.91\nepoch 2/5: loss 0.55\nepoch 3/5: loss 0.34\n",
            62,
            false,
        ));
        let terminal = render_terminal(&mut viewer, 100, 12);
        insta::assert_snapshot!("log_view_fullscreen", terminal.backend());
    }
}
