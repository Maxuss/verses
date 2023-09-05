use std::{
    io::Stdout,
    sync::{Arc, Mutex},
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::CrosstermBackend, widgets::Paragraph, Terminal};

use crate::event::{StatusEvent, TrackMetadata};

use super::Lyrics;

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Default)]
struct LyricsTracker {
    lyrics: Lyrics,
    current_line: isize,
    current_progress_ms: u32,
    track_data: TrackMetadata,
}

pub struct VersesTui {
    tracker: Arc<Mutex<LyricsTracker>>,
}

impl VersesTui {
    pub fn new() -> Self {
        Self {
            tracker: Arc::new(Mutex::new(Default::default())),
        }
    }

    pub async fn run(self, event_rx: flume::Receiver<StatusEvent>) -> anyhow::Result<()> {
        let tracker_w = self.tracker.clone();
        let event_handler =
            tokio::task::spawn(async move { Self::run_event_handler(tracker_w, event_rx).await });
        let tui_handler = tokio::task::spawn(async move { self.run_tui().await });
        let (events, tui) = tokio::join!(event_handler, tui_handler);
        events??;
        tui??;
        Ok(())
    }

    async fn run_event_handler(
        tracker: Arc<Mutex<LyricsTracker>>,
        event_rx: flume::Receiver<StatusEvent>,
    ) -> anyhow::Result<()> {
        while let Ok(event) = event_rx.recv_async().await {
            match event {
                StatusEvent::NewTrack {
                    metadata,
                    new_lyrics,
                } => {
                    let mut tracker = tracker.lock().unwrap();
                    tracker.current_line = -1;
                    tracker.lyrics = new_lyrics;
                    tracker.track_data = metadata;
                }
                StatusEvent::SwitchLyricLine { new_line } => {
                    let mut tracker = tracker.lock().unwrap();
                    if new_line == -1 {
                        continue;
                    }
                    tracker.current_line = new_line as isize;
                }
                StatusEvent::NewTrackNoLyrics { metadata } => {
                    let mut tracker = tracker.lock().unwrap();
                    tracker.current_line = -1;
                    tracker.lyrics.lines.clear();
                    tracker.track_data = metadata;
                }
                StatusEvent::TrackProgress { new_progress_ms } => {
                    let mut tracker = tracker.lock().unwrap();
                    tracker.current_progress_ms = new_progress_ms;
                }
            }
        }
        Ok(())
    }

    async fn run_tui(&self) -> anyhow::Result<()> {
        let mut terminal = setup_terminal()?;

        self.tui_loop(&mut terminal).await?;

        restore_terminal(&mut terminal)
    }

    async fn tui_loop(&self, terminal: &mut Term) -> anyhow::Result<()> {
        let tracker = self.tracker.clone();
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
