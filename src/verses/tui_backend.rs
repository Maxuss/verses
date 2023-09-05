use std::{borrow::Cow, io::Stdout, sync::Arc, time::Duration, vec};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use deunicode::{deunicode, deunicode_with_tofu_cow};
use handlebars::{handlebars_helper, Handlebars};
use ratatui::{
    prelude::*,
    style::Stylize,
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::config::VersesConfig;

use super::handler::{SyncTracker, VersesBackend};

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone)]
pub struct TerminalUiBackend<'a> {
    cached_info_vec: Vec<Line<'a>>,
    old_tracker_hash: u64,
}

#[async_trait::async_trait]
impl<'a> VersesBackend for TerminalUiBackend<'a> {
    async fn run_backend(
        &mut self,
        tracker: SyncTracker,
        config: Arc<VersesConfig>,
    ) -> anyhow::Result<()> {
        let mut terminal = setup_terminal()?;

        self.tui_loop(tracker, &mut terminal, config).await?;

        restore_terminal(&mut terminal)
    }
}

impl<'a> TerminalUiBackend<'a> {
    pub fn new() -> Self {
        Self {
            cached_info_vec: Vec::with_capacity(4),
            old_tracker_hash: 0,
        }
    }

    async fn tui_loop(
        &mut self,
        tracker: SyncTracker,
        terminal: &mut Term,
        cfg: Arc<VersesConfig>,
    ) -> anyhow::Result<()> {
        Ok(loop {
            terminal.draw(|frame| self.handle_ui(&tracker, frame, &cfg))?;
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if KeyCode::Char('q') == key.code {
                        break;
                    }
                }
            }
        })
    }

    fn handle_ui(
        &mut self,
        tracker: &SyncTracker,
        f: &mut Frame<CrosstermBackend<Stdout>>,
        cfg: &Arc<VersesConfig>,
    ) {
        let size = f.size();

        let tracker = tracker.lock().unwrap();

        let lyrics_top_text = maybe_romanize_str(
            &tracker.track_data.track_name,
            &tracker.lyrics.language,
            &cfg,
        );
        let lyrics_block = Block::default()
            .fg(cfg.theme.borders.lyrics_border_color.0)
            .title(Line::from(
                lyrics_top_text.fg(cfg.theme.borders.lyrics_border_text_color.0),
            ))
            .borders(Borders::ALL)
            .border_type(cfg.theme.borders.lyrics_border_style.0)
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
        let text = if tracker.lyrics.lines.is_empty() {
            vec![Line::from(
                "This song does not have synchronized lyrics :(".fg(cfg
                    .theme
                    .lyrics
                    .inactive_text_color
                    .0),
            )]
        } else {
            let lines = if tracker.lyrics.language != "en"
                && cfg.general.romanize_unicode
                && !cfg
                    .general
                    .romanize_exclude
                    .contains(&tracker.lyrics.language)
            {
                // attempting to romanize non-english lines
                tracker
                    .lyrics
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(idx, each)| {
                        let romanized = deunicode_with_tofu_cow(&each.words, "[?]");
                        let fg_color = if idx == current_line {
                            cfg.theme.lyrics.active_text_color.0
                        } else {
                            cfg.theme.lyrics.inactive_text_color.0
                        };
                        if romanized == each.words {
                            // in some cases, romanization is not needed
                            vec![Line::from(each.words.fg(fg_color))]
                        } else {
                            vec![
                                Line::from(each.words.fg(fg_color)),
                                Line::from(Span {
                                    content: romanized,
                                    style: Style::default().fg(fg_color),
                                }),
                            ]
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>()
            } else {
                tracker
                    .lyrics
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(idx, each)| {
                        if idx == current_line {
                            Line::from(each.words.fg(cfg.theme.lyrics.active_text_color.0))
                        } else {
                            Line::from(each.words.fg(cfg.theme.lyrics.inactive_text_color.0))
                        }
                    })
                    .collect::<Vec<_>>()
            };
            lines
        };

        let lyrics_part = Paragraph::new(text)
            .style(Style::default())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .scroll((
                (current_line as i16 - cfg.general.scroll_offset as i16).max(0) as u16,
                0,
            ))
            .block(lyrics_block);
        f.render_widget(lyrics_part, horizontal_layout[0]);

        // Track info
        let info_block = Block::new()
            .fg(cfg.theme.borders.info_border_color.0)
            .title("About this track".fg(cfg.theme.borders.info_border_text_color.0))
            .borders(Borders::ALL)
            .border_type(cfg.theme.borders.info_border_style.0)
            .title_alignment(Alignment::Left);

        let new_hash = tracker.identity_hash();
        let info_vec = if self.old_tracker_hash == new_hash {
            // use previously prepared info
            self.cached_info_vec.clone()
        } else {
            // rebuild info
            let mut reg = Handlebars::new();
            handlebars_helper!(join_helper: |input: Vec<String>| { input.join(", ") });
            reg.register_helper("join", Box::new(join_helper));
            let mut info_vec = Vec::with_capacity(4);
            if cfg.general.display.show_name {
                info_vec.push(Line::from(
                    reg.render_template(
                        &cfg.general.display.name_format,
                        &serde_json::json!({ "name": tracker.track_data.track_name }),
                    )
                    .unwrap(),
                ));
            };
            if cfg.general.display.show_artists {
                info_vec.push(Line::from(
                    reg.render_template(
                        &cfg.general.display.artists_format,
                        &serde_json::json!({ "artists": tracker.track_data.track_artists }),
                    )
                    .unwrap(),
                ));
            };
            if cfg.general.display.show_album {
                info_vec.push(Line::from(
                    reg.render_template(
                        &cfg.general.display.album_format,
                        &serde_json::json!({ "album": tracker.track_data.track_album }),
                    )
                    .unwrap(),
                ));
            };
            if cfg.general.display.show_genres {
                info_vec.push(Line::from(
                    reg.render_template(
                        &cfg.general.display.genres_format,
                        &serde_json::json!({ "genres": tracker.track_data.artist_genres }),
                    )
                    .unwrap(),
                ));
            };
            if cfg.general.display.show_popularity {
                info_vec.push(Line::from(
                    reg.render_template(
                        &cfg.general.display.popularity_format,
                        &serde_json::json!({ "popularity": tracker.track_data.popularity }),
                    )
                    .unwrap(),
                ));
            };
            self.old_tracker_hash = tracker.identity_hash();
            self.cached_info_vec = info_vec.clone();
            info_vec
        };
        let info_part = Paragraph::new(info_vec)
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false })
            .block(info_block);
        f.render_widget(info_part, horizontal_layout[1]);

        // Track progress
        let progress_percent = f32::ceil(
            (tracker.current_progress_ms as f32
                / tracker.track_data.track_duration.as_millis() as f32)
                * 100f32,
        ) as u16;
        let label = if cfg.theme.progress_bar.is_percentage {
            format!("{progress_percent}%")
        } else {
            format!(
                "{} / {}",
                fmt_duration(tracker.current_progress_ms),
                fmt_duration(tracker.track_data.track_duration.as_millis() as u32)
            )
        };
        let track_progress = Gauge::default()
            .gauge_style(Style::default().fg(cfg.theme.progress_bar.color.0))
            .percent(progress_percent)
            .label(label);
        f.render_widget(track_progress, vertical_layout[1])
    }
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

fn maybe_romanize_str(name: &str, language: &str, cfg: &Arc<VersesConfig>) -> String {
    if cfg.general.romanize_unicode
        && language != "en"
        && !cfg.general.romanize_exclude.contains(&language.to_owned())
    {
        let romanized = deunicode(name);
        if romanized == name {
            return romanized;
        } else {
            return format!("{name} ({romanized})");
        }
    } else {
        return name.to_owned();
    }
}
