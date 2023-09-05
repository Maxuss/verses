use std::{io::Stdout, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    style::Stylize,
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

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
            terminal.draw(|frame| handle_ui(&tracker, frame))?;
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

fn handle_ui(tracker: &SyncTracker, f: &mut Frame<CrosstermBackend<Stdout>>) {
    let size = f.size();

    let tracker = tracker.lock().unwrap();

    let lyrics_block = Block::default()
        .dark_gray()
        .title(Line::from(&tracker.track_data.track_name as &str))
        .borders(Borders::ALL)
        .title_alignment(Alignment::Left);

    // Layouts
    let vertical_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(95), Constraint::Percentage(5)].as_ref())
        .split(size);

    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(26)].as_ref())
        .split(vertical_layout[0]);

    // Lyrics
    let current_line = tracker.current_line as usize;
    let text = tracker
        .lyrics
        .lines
        .iter()
        .enumerate()
        .map(|(idx, each)| {
            if idx == current_line {
                Line::from(each.words.fg(Color::LightGreen))
            } else {
                Line::from(each.words.fg(Color::Gray))
            }
        })
        .collect::<Vec<_>>();

    let lyrics_part = Paragraph::new(text)
        .style(Style::default())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .scroll(((current_line as i16 - 4).max(0) as u16, 0))
        .block(lyrics_block);
    f.render_widget(lyrics_part, horizontal_layout[0]);

    // Track info
    let info_block = Block::new()
        .dark_gray()
        .title("About this track")
        .borders(Borders::ALL)
        .title_alignment(Alignment::Right);

    let info_part = Paragraph::new(vec![
        Line::from(format!("Author: {}", tracker.track_data.track_author)),
        Line::from(format!("Album: {}", tracker.track_data.track_album)),
    ])
    .style(Style::default().gray())
    .alignment(Alignment::Left)
    .wrap(Wrap { trim: false })
    .block(info_block);
    f.render_widget(info_part, horizontal_layout[1]);

    // Track progress
    let progress_percent = f32::ceil(
        (tracker.current_progress_ms as f32 / tracker.track_data.track_duration.as_millis() as f32)
            * 100f32,
    ) as u16;
    let label = format!(
        "{} / {}",
        fmt_duration(tracker.current_progress_ms),
        fmt_duration(tracker.track_data.track_duration.as_millis() as u32)
    );
    let track_progress = Gauge::default()
        .gauge_style(Style::default().fg(Color::LightBlue))
        .percent(progress_percent)
        .label(label);
    f.render_widget(track_progress, vertical_layout[1])
}

fn fmt_duration(duration_ms: u32) -> String {
    let mut seconds = duration_ms / 1000;
    let minutes = seconds / 60;
    seconds %= 60;
    format!("{minutes:0>2}:{seconds:0>2}")
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
