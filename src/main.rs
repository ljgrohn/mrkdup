mod app;
mod checkbox;
mod clipboard;
mod config;
mod editor;
mod files;
mod fsutil;
mod fuzzy;
mod highlight;
mod layout_cache;
mod render;
mod search;
mod tab;
mod theme;
mod tree;
mod ui;
mod wrap;

use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;

fn main() -> io::Result<()> {
    let root = match env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };
    let (config, mut warnings) = config::load();
    let (theme, theme_warnings) = theme::load(&config.theme_name);
    warnings.extend(theme_warnings);
    let mut app = app::App::new_with_theme(root, config, theme, config::config_dir())?;
    if !warnings.is_empty() {
        app.status = Some(format!(
            "config/theme: {} warning(s) — {}",
            warnings.len(),
            warnings[0]
        ));
    }
    let mut terminal = ratatui::init();
    // Without the kitty keyboard protocol, terminals never report the
    // Cmd/Super modifier (and often not Shift on letters); enable it
    // where supported so the modifier motions work.
    let enhanced = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        let _ = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    // The app owns the mouse: clicks land the cursor and drags select
    // inside the editor pane only, instead of the terminal striping a
    // selection across both panes. (Shift+drag still reaches the
    // terminal's own selection in most emulators.)
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = run(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => app.handle_key(k),
                Event::Mouse(m) => app.handle_mouse(m),
                _ => {}
            }
            if let Some(text) = app.clipboard.take() {
                // best effort: a terminal without OSC 52 just ignores it
                let _ = clipboard::copy(&mut io::stdout(), &text);
            }
        } else {
            app.tick();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
