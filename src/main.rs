mod app;
mod checkbox;
mod config;
mod editor;
mod files;
mod fsutil;
mod fuzzy;
mod highlight;
mod layout_cache;
mod render;
mod search;
mod theme;
mod tree;
mod ui;
mod wrap;

use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
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
    let mut app = app::App::new_with_theme(root, config, theme)?;
    if !warnings.is_empty() {
        app.status = Some(format!(
            "config: ignored {} line(s) — {}",
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
    let result = run(&mut terminal, &mut app);
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
                _ => {}
            }
        } else {
            app.tick();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
