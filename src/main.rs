mod app;
mod editor;
mod fsutil;
mod tree;
mod ui;

use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};

fn main() -> io::Result<()> {
    let root = match env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };
    let mut app = app::App::new(root)?;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
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
