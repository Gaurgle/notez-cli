pub mod theme;

use std::io;
use std::path::Path;
use std::process::Command;

use crossterm::{
    event::KeyCode,
    event::KeyEvent,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;

pub type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

/// Enter the TUI: raw mode + alternate screen. Returns the Terminal.
pub fn enter() -> io::Result<Terminal> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    ratatui::Terminal::new(backend)
}

/// Leave the TUI: restore terminal state.
pub fn leave() -> io::Result<()> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Open a file in $EDITOR, then restore the TUI.
/// Leaves alternate screen, runs editor, re-enters alternate screen.
pub fn open_in_editor(editor: &str, file: &Path) -> io::Result<()> {
    leave()?;
    Command::new(editor)
        .arg(file)
        .status()
        .expect("failed to launch editor");
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    Ok(())
}

/// Tracks vim-style command input (e.g., `:wq`, `:qa`).
pub struct VimCommandMode {
    pub active: bool,
    pub buffer: String,
}

impl VimCommandMode {
    pub fn new() -> Self {
        Self {
            active: false,
            buffer: String::new(),
        }
    }

    /// Process a key event. Returns Some(command) if a command was completed (Enter pressed).
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if !self.active {
            if key.code == KeyCode::Char(':') {
                self.active = true;
                self.buffer.clear();
                self.buffer.push(':');
                return None;
            }
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                let cmd = self.buffer.clone();
                self.active = false;
                self.buffer.clear();
                Some(cmd)
            }
            KeyCode::Esc => {
                self.active = false;
                self.buffer.clear();
                None
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    self.active = false;
                }
                None
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                None
            }
            _ => None,
        }
    }

    /// Check if a completed command means "quit".
    pub fn is_quit(cmd: &str) -> bool {
        matches!(cmd, ":wq" | ":qa" | ":q" | ":q!")
    }
}
