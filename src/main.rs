use std::{
    error::Error,
    io::{self, stdout},
    process::Command,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, Terminal};

use composer_tui::{ui, App, Config, FocusArea, InputMode};

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(16);

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
    let config = Config::load();
    let mut app = App::from_state_with_config(
        composer_tui::AppState::load(),
        discover_worktree_manager(),
        config,
    );

    loop {
        let size = terminal.size()?;
        let sidebar_width = app.config().sidebar_width();
        let (cols, rows) = ui::main_panel_terminal_size(
            size.width,
            size.height,
            app.is_fullscreen(),
            sidebar_width,
        );
        app.tick(cols, rows);

        // Render the UI
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Handle input events
        if event::poll(EVENT_POLL_INTERVAL)? {
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        let action = handle_key_event(&mut app, key);
                        if let Some(EditorAction::OpenSettings) = action {
                            open_settings_editor(terminal)?;
                            app.reload_config();
                        }
                    }
                    Event::Mouse(mouse) => {
                        let sidebar_width = app.config().sidebar_width();
                        handle_mouse_event(&mut app, mouse, size.width, size.height, sidebar_width);
                    }
                    _ => {}
                }

                if !event::poll(Duration::from_millis(0))? {
                    break;
                }
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

fn discover_worktree_manager() -> Option<composer_tui::WorktreeManager> {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| composer_tui::WorktreeManager::new(cwd).ok())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        Show,
        DisableMouseCapture
    )?;
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
    execute!(stdout(), LeaveAlternateScreen, Show, DisableMouseCapture)
}

/// Actions that require the event loop to do something outside normal key handling.
enum EditorAction {
    /// The user pressed `S` — open the config file in $EDITOR.
    OpenSettings,
}

fn handle_key_event(app: &mut App, key: KeyEvent) -> Option<EditorAction> {
    if matches!(key.kind, KeyEventKind::Release) {
        return None;
    }

    if app.is_modal_active() {
        handle_modal_key_event(app, key);
        return None;
    }

    if handle_global_tab_switch_key_event(app, key) {
        return None;
    }

    if app.focus() == FocusArea::Main {
        handle_main_focus_key_event(app, key);
        return None;
    }

    handle_navigation_key_event(app, key)
}

fn handle_navigation_key_event(app: &mut App, key: KeyEvent) -> Option<EditorAction> {
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        match key.code {
            KeyCode::Char('h') => {
                app.focus_left();
                return None;
            }
            KeyCode::Char('l') => {
                app.focus_right();
                return None;
            }
            KeyCode::Char('k') => {
                app.focus_up();
                return None;
            }
            KeyCode::Char('j') => {
                app.focus_down();
                return None;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('J') if app.focus() == FocusArea::Sidebar => {
            app.move_selected_workspace_down();
        }
        KeyCode::Char('K') if app.focus() == FocusArea::Sidebar => {
            app.move_selected_workspace_up();
        }
        KeyCode::Char('t')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.focus() == FocusArea::Sidebar =>
        {
            app.add_tab_to_selected_workspace();
        }
        KeyCode::Char('w')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && app.focus() == FocusArea::Sidebar =>
        {
            app.start_close_selected_workspace_tab();
        }
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
        KeyCode::Char('k') | KeyCode::Up => app.select_previous(),
        KeyCode::Enter => app.focus_right(),
        KeyCode::Char('n') => app.start_create_workspace(),
        KeyCode::Char('d') => app.start_delete_workspace(),
        KeyCode::Char('z') => app.toggle_fullscreen(),
        KeyCode::Char('S') => return Some(EditorAction::OpenSettings),
        KeyCode::Char('R') => app.reload_config(),
        _ => {}
    }

    None
}

fn handle_global_tab_switch_key_event(app: &mut App, key: KeyEvent) -> bool {
    if !key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }

    let KeyCode::Char(ch) = key.code else {
        return false;
    };
    let Some(number) = ch.to_digit(10) else {
        return false;
    };
    if number == 0 || number > 9 {
        return false;
    }

    let index = (number as usize) - 1;
    app.select_selected_workspace_tab(index);
    true
}

fn handle_main_focus_key_event(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::PageUp => {
                app.scroll_selected_terminal_up();
                return;
            }
            KeyCode::PageDown => {
                app.scroll_selected_terminal_down();
                return;
            }
            _ => {}
        }
    }

    if app.selected_terminal_is_scrolled() {
        app.scroll_selected_terminal_to_bottom();
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'o') => {
                app.exit_fullscreen();
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

/// Open the config file in the user's $EDITOR.
///
/// Temporarily leaves raw mode and alternate screen so the editor can run
/// normally. Re-enters after the editor exits and reloads config.
fn open_settings_editor(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let config_path = match composer_tui::config::config_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Warning: could not locate config path: {err}");
            return Ok(());
        }
    };

    // Ensure the config file exists (create with commented template if missing).
    Config::write_default_template()?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // Leave TUI mode.
    cleanup_terminal(terminal)?;

    // Spawn the editor and wait for it to finish.
    let status = Command::new(&editor).arg(&config_path).status();

    // Re-enter TUI mode regardless of editor result.
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        Hide,
        EnableMouseCapture
    )?;
    // Force a full redraw after returning from editor.
    terminal.clear()?;

    if let Err(err) = status {
        eprintln!("Warning: failed to launch editor '{editor}': {err}");
    }

    Ok(())
}

/// Check whether a point (col, row) falls within a `Rect`.
fn rect_contains(rect: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
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

/// Handle mouse click events for focus and workspace selection.
fn handle_mouse_event(
    app: &mut App,
    mouse: MouseEvent,
    width: u16,
    height: u16,
    sidebar_width: u16,
) {
    // Only respond to left button presses (ignore drags, scrolls, releases).
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return;
    }

    // Ignore mouse clicks while a modal is active.
    if app.is_modal_active() {
        return;
    }

    let (_header_rect, sidebar_rect, main_rect, _status_rect) =
        ui::layout_rects(width, height, app.is_fullscreen(), sidebar_width);

    let col = mouse.column;
    let row = mouse.row;

    if let Some(tab_index) = ui::main_panel_tab_index_at(main_rect, app, col, row) {
        app.select_selected_workspace_tab(tab_index);
        app.focus_right();
        return;
    }

    if let Some(sidebar) = sidebar_rect {
        if rect_contains(sidebar, col, row) {
            // Workspace items start below the sidebar top border (sidebar.y + 1).
            let inner_row = row.saturating_sub(sidebar.y + 1) as usize;
            app.set_selected_index(inner_row);
            app.focus_left();
            return;
        }
    }

    if rect_contains(main_rect, col, row) {
        app.focus_right();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        std::env::set_var("COMPOSER_TUI_DISABLE_STATE_SAVE", "1");
        App::from_state_with_manager(composer_tui::AppState::default(), None)
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn enter_focuses_main_panel_from_sidebar() {
        let mut app = test_app();
        assert_eq!(app.focus(), FocusArea::Sidebar);

        handle_key_event(&mut app, key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.focus(), FocusArea::Main);
    }

    #[test]
    fn ctrl_o_escapes_main_focus_to_sidebar() {
        let mut app = test_app();
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

    #[test]
    fn z_toggles_fullscreen_from_sidebar() {
        let mut app = test_app();
        assert!(!app.is_fullscreen());

        handle_key_event(&mut app, key(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(app.is_fullscreen());

        handle_key_event(&mut app, key(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(!app.is_fullscreen());
    }

    #[test]
    fn ctrl_o_exits_fullscreen_and_focuses_sidebar() {
        let mut app = test_app();
        app.toggle_fullscreen();
        app.focus_right(); // focus main
        assert!(app.is_fullscreen());
        assert_eq!(app.focus(), FocusArea::Main);

        handle_key_event(&mut app, key(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert!(!app.is_fullscreen());
        assert_eq!(app.focus(), FocusArea::Sidebar);
    }

    #[test]
    fn shift_page_keys_are_consumed_for_scrollback() {
        let mut app = test_app();
        app.focus_right();

        handle_key_event(&mut app, key(KeyCode::PageUp, KeyModifiers::SHIFT));
        assert_eq!(app.focus(), FocusArea::Main);

        handle_key_event(&mut app, key(KeyCode::PageDown, KeyModifiers::SHIFT));
        assert_eq!(app.focus(), FocusArea::Main);
    }

    #[test]
    fn key_release_events_are_ignored() {
        let mut app = test_app();
        assert_eq!(app.focus(), FocusArea::Sidebar);

        handle_key_event(
            &mut app,
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
        );
        assert_eq!(app.focus(), FocusArea::Sidebar);
    }

    #[test]
    fn s_key_returns_editor_action() {
        let mut app = test_app();
        let action = handle_key_event(&mut app, key(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(matches!(action, Some(EditorAction::OpenSettings)));
    }

    #[test]
    fn r_key_reloads_config() {
        let mut app = test_app();
        // R should not panic and should return no editor action.
        let action = handle_key_event(&mut app, key(KeyCode::Char('R'), KeyModifiers::NONE));
        assert!(action.is_none());
    }

    #[test]
    fn alt_number_switches_tabs_from_main_focus() {
        let mut app = test_app();
        app.add_tab_to_selected_workspace();
        app.add_tab_to_selected_workspace();
        assert!(
            matches!(app.input_mode(), InputMode::Normal),
            "tab setup should keep app in normal mode: {:?}",
            app.input_mode()
        );
        app.focus_right();

        let action = handle_key_event(&mut app, key(KeyCode::Char('1'), KeyModifiers::ALT));
        assert!(action.is_none());
        assert_eq!(
            app.selected_workspace()
                .expect("workspace")
                .active_tab_index(),
            0
        );

        let action = handle_key_event(&mut app, key(KeyCode::Char('3'), KeyModifiers::ALT));
        assert!(action.is_none());
        assert_eq!(
            app.selected_workspace()
                .expect("workspace")
                .active_tab_index(),
            2
        );
    }

    #[test]
    fn ctrl_t_and_ctrl_w_manage_tabs_from_sidebar() {
        let mut app = test_app();
        assert_eq!(app.focus(), FocusArea::Sidebar);
        assert_eq!(app.selected_workspace().expect("workspace").tab_count(), 1);

        handle_key_event(&mut app, key(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.selected_workspace().expect("workspace").tab_count(), 2);
        assert!(
            matches!(app.input_mode(), InputMode::Normal),
            "tab creation should keep app in normal mode"
        );

        handle_key_event(&mut app, key(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(app.selected_workspace().expect("workspace").tab_count(), 1);
    }

    #[test]
    fn shift_j_and_shift_k_reorder_workspaces_from_sidebar() {
        let mut app = test_app();
        assert_eq!(app.selected_workspace().expect("workspace").name(), "W1");

        handle_key_event(&mut app, key(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(app.selected_index(), 1);
        assert_eq!(app.workspaces()[0].name(), "W2");
        assert_eq!(app.workspaces()[1].name(), "W1");

        handle_key_event(&mut app, key(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.workspaces()[0].name(), "W1");
        assert_eq!(app.workspaces()[1].name(), "W2");
    }

    #[test]
    fn shift_j_and_shift_k_do_not_reorder_outside_sidebar() {
        let mut app = test_app();
        app.focus_right();

        handle_key_event(&mut app, key(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.workspaces()[0].name(), "W1");
        assert_eq!(app.workspaces()[1].name(), "W2");
    }

    #[test]
    fn mouse_click_on_main_border_tab_switches_tab() {
        let mut app = test_app();
        app.add_tab_to_selected_workspace();
        app.add_tab_to_selected_workspace();
        assert!(
            matches!(app.input_mode(), InputMode::Normal),
            "tab setup should keep app in normal mode"
        );
        assert_eq!(
            app.selected_workspace()
                .expect("workspace")
                .active_tab_index(),
            2
        );

        let (_header, _, main, _) = ui::layout_rects(120, 24, false, 20);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: main.x + 2,
            row: main.y,
            modifiers: KeyModifiers::NONE,
        };

        handle_mouse_event(&mut app, click, 120, 24, 20);
        assert_eq!(
            app.selected_workspace()
                .expect("workspace")
                .active_tab_index(),
            0
        );
        assert_eq!(app.focus(), FocusArea::Main);
    }
}
