use std::{
    error::Error,
    io::{self, stdout},
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};

use composer_tui::{ui, App, Config, FocusArea, InputMode};

fn main() -> Result<(), Box<dyn Error>> {
    install_panic_hook();

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal);
    cleanup_terminal(&mut terminal)?;

    if let Err(err) = result {
        eprintln!("{err}");
    }

    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    Config::load();
    let mut app = App::new();

    loop {
        let size = terminal.size()?;
        let (cols, rows) = ui::main_panel_terminal_size(size.width, size.height);
        app.tick_terminals(cols, rows);

        // Render the UI
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Handle input events
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(&mut app, key);
            }
        }

        if app.should_quit() {
            if let Err(err) = app.save_state() {
                eprintln!("Warning: failed to save state: {err}");
            }
            break;
        }
    }

    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = cleanup_terminal_on_panic();
        default_hook(info);
    }));
}

fn cleanup_terminal_on_panic() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, Show)
}

fn handle_key_event(app: &mut App, key: KeyEvent) {
    if app.is_modal_active() {
        handle_modal_key_event(app, key);
        return;
    }

    if app.focus() == FocusArea::Main {
        handle_main_focus_key_event(app, key);
        return;
    }

    handle_navigation_key_event(app, key);
}

fn handle_navigation_key_event(app: &mut App, key: KeyEvent) {
    let focus_modifier = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    if focus_modifier {
        match key.code {
            KeyCode::Char('h') => app.focus_left(),
            KeyCode::Char('l') => app.focus_right(),
            KeyCode::Char('k') => app.focus_up(),
            KeyCode::Char('j') => app.focus_down(),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
        KeyCode::Char('k') | KeyCode::Up => app.select_previous(),
        KeyCode::Enter => app.focus_right(),
        KeyCode::Char('n') => app.start_create_workspace(),
        KeyCode::Char('d') => app.start_delete_workspace(),
        _ => {}
    }
}

fn handle_main_focus_key_event(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'o') => {
                app.focus_left();
                return;
            }
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'c') => {
                app.send_selected_terminal_input(&[0x03]);
                return;
            }
            _ => {}
        }
    }

    if let Some(bytes) = key_to_terminal_bytes(key) {
        app.send_selected_terminal_input(&bytes);
    }
}

fn key_to_terminal_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let mut out = Vec::new();
    if alt {
        out.push(0x1b);
    }

    match key.code {
        KeyCode::Enter => out.push(b'\r'),
        KeyCode::Backspace => out.push(0x7f),
        KeyCode::Tab => out.push(b'\t'),
        KeyCode::BackTab => out.extend_from_slice(b"\x1b[Z"),
        KeyCode::Left => out.extend_from_slice(b"\x1b[D"),
        KeyCode::Right => out.extend_from_slice(b"\x1b[C"),
        KeyCode::Up => out.extend_from_slice(b"\x1b[A"),
        KeyCode::Down => out.extend_from_slice(b"\x1b[B"),
        KeyCode::Home => out.extend_from_slice(b"\x1b[H"),
        KeyCode::End => out.extend_from_slice(b"\x1b[F"),
        KeyCode::Delete => out.extend_from_slice(b"\x1b[3~"),
        KeyCode::Insert => out.extend_from_slice(b"\x1b[2~"),
        KeyCode::PageUp => out.extend_from_slice(b"\x1b[5~"),
        KeyCode::PageDown => out.extend_from_slice(b"\x1b[6~"),
        KeyCode::Esc => out.push(0x1b),
        KeyCode::Char(ch) => {
            if ctrl {
                if let Some(ctrl_byte) = ctrl_char_to_byte(ch) {
                    out.push(ctrl_byte);
                } else {
                    return None;
                }
            } else {
                let mut utf8 = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut utf8).as_bytes());
            }
        }
        _ => return None,
    }

    Some(out)
}

fn ctrl_char_to_byte(ch: char) -> Option<u8> {
    let lower = ch.to_ascii_lowercase();
    match lower {
        'a'..='z' => Some((lower as u8) - b'a' + 1),
        '@' | ' ' => Some(0),
        '[' => Some(27),
        '\\' => Some(28),
        ']' => Some(29),
        '^' => Some(30),
        '_' => Some(31),
        '?' => Some(127),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn enter_focuses_main_panel_from_sidebar() {
        let mut app = App::from_state_with_manager(composer_tui::AppState::default(), None);
        assert_eq!(app.focus(), FocusArea::Sidebar);

        handle_key_event(&mut app, key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.focus(), FocusArea::Main);
    }

    #[test]
    fn ctrl_o_escapes_main_focus_to_sidebar() {
        let mut app = App::from_state_with_manager(composer_tui::AppState::default(), None);
        app.focus_right();
        assert_eq!(app.focus(), FocusArea::Main);

        handle_key_event(&mut app, key(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert_eq!(app.focus(), FocusArea::Sidebar);
    }

    #[test]
    fn key_to_terminal_bytes_maps_common_keys() {
        assert_eq!(
            key_to_terminal_bytes(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(vec![b'a'])
        );
        assert_eq!(
            key_to_terminal_bytes(key(KeyCode::Left, KeyModifiers::NONE)),
            Some(b"\x1b[D".to_vec())
        );
        assert_eq!(
            key_to_terminal_bytes(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(vec![0x03])
        );
    }
}

fn handle_modal_key_event(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_input(),
        KeyCode::Enter => app.confirm_input(),
        KeyCode::Backspace => app.pop_input_char(),
        KeyCode::Char(ch) => {
            if matches!(app.input_mode(), InputMode::CreateWorkspace { .. }) {
                app.push_input_char(ch);
            }
        }
        _ => {}
    }
}
