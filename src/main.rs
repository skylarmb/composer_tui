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

use composer_tui::{ui, App, Config, InputMode};

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
        KeyCode::Char('n') => app.start_create_workspace(),
        KeyCode::Char('d') => app.start_delete_workspace(),
        _ => {}
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
