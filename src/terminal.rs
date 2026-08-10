use std::io::{stdout, Write};

use anyhow::Result;
use crossterm::{cursor, event, execute, terminal};

fn write_setup_sequence<W: Write>(writer: &mut W) -> Result<()> {
  // Mouse capture stays on for the whole interactive session so live `g`/`G`
  // gravity adjustments can receive pointer input without restarting.
  execute!(
    writer,
    terminal::EnterAlternateScreen,
    cursor::Hide,
    terminal::Clear(terminal::ClearType::All),
    event::EnableMouseCapture,
    event::EnableFocusChange
  )?;

  Ok(())
}

fn write_cleanup_sequence<W: Write>(writer: &mut W) -> Result<()> {
  execute!(
    writer,
    event::DisableFocusChange,
    event::DisableMouseCapture,
    cursor::Show,
    terminal::LeaveAlternateScreen
  )?;

  Ok(())
}

/// Setup terminal for rendering
pub fn setup() -> Result<()> {
  terminal::enable_raw_mode()?;

  let mut out = stdout();
  write_setup_sequence(&mut out)?;

  Ok(())
}

/// Restore terminal to normal state
pub fn cleanup() -> Result<()> {
  let mut out = stdout();
  write_cleanup_sequence(&mut out)?;
  terminal::disable_raw_mode()?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_setup_sequence_writes_alternate_screen_hide_clear_and_mouse() {
    let mut output = Vec::new();

    write_setup_sequence(&mut output).unwrap();

    let text = String::from_utf8(output).unwrap();

    assert_eq!(
      text,
      "\u{1b}[?1049h\u{1b}[?25l\u{1b}[2J\u{1b}[?1000h\u{1b}[?1002h\u{1b}[?1003h\u{1b}[?1015h\u{1b}[?1006h\u{1b}[?1004h"
    );
  }

  #[test]
  fn test_cleanup_sequence_disables_mouse_then_restores_screen() {
    let mut output = Vec::new();

    write_cleanup_sequence(&mut output).unwrap();

    let text = String::from_utf8(output).unwrap();

    assert_eq!(
      text,
      "\u{1b}[?1004l\u{1b}[?1006l\u{1b}[?1015l\u{1b}[?1003l\u{1b}[?1002l\u{1b}[?1000l\u{1b}[?25h\u{1b}[?1049l"
    );
  }
}
