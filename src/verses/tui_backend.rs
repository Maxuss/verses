use std::{io::Stdout, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, widgets::Paragraph, Terminal};

use super::handler::{SyncTracker, VersesBackend};

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy)]
pub struct TerminalUiBackend;

#[async_trait::async_trait]
impl VersesBackend for TerminalUiBackend {
    async fn run_backend(&self, tracker: SyncTracker) -> anyhow::Result<()> {
        let mut terminal = setup_terminal()?;

        self.tui_loop(tracker, &mut terminal).await?;

        restore_terminal(&mut terminal)
    }
}

impl TerminalUiBackend {
    async fn tui_loop(&self, tracker: SyncTracker, terminal: &mut Term) -> anyhow::Result<()> {
        Ok(loop {
            terminal.draw(|frame| {
                let tracker = tracker.lock().unwrap();

                let line = tracker
                    .lyrics
                    .lines
                    .get(tracker.current_line as usize)
                    .map(|it| it.words.clone())
                    .unwrap_or("No lyrics :(".to_owned());
                let greeting = Paragraph::new(line);
                frame.render_widget(greeting, frame.size());
            })?;
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if KeyCode::Char('q') == key.code {
                        break;
                    }
                }
            }
        })
    }
}

fn setup_terminal() -> anyhow::Result<Term> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen,)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Term) -> anyhow::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor().map_err(anyhow::Error::from)
}
