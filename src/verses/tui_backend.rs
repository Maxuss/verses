use std::{borrow::Cow, io::Stdout, sync::Arc, time::Duration, vec};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use deunicode::{deunicode, deunicode_with_tofu};
use handlebars::{handlebars_helper, Handlebars};
use lazy_static::lazy_static;
use ratatui::{
    prelude::*,
    style::Stylize,
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};
use regex::Regex;

use crate::config::VersesConfig;

use super::{
    handler::{SyncTracker, VersesBackend},
    LyricSyncType,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

lazy_static! {
    static ref UNEPXECTED_CAMEL_CASE_REGEX: Regex = Regex::new("[a-z][A-Z]").unwrap();
}

#[derive(Debug, Clone)]
pub struct TerminalUiBackend<'a> {
    cached_info_vec: Vec<Line<'a>>,
    old_tracker_hash: u64,

    autoscroll_enabled: bool,
    scroll_amount: u16,
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

impl<'a> Default for TerminalUiBackend<'a> {
    fn default() -> Self {
        Self {
            cached_info_vec: Vec::with_capacity(4),
            old_tracker_hash: 0,
            autoscroll_enabled: true,
            scroll_amount: 0,
        }
    }
}

impl<'a> TerminalUiBackend<'a> {
    async fn tui_loop(
        &mut self,
        tracker: SyncTracker,
        terminal: &mut Term,
        cfg: Arc<VersesConfig>,
    ) -> anyhow::Result<()> {
        loop {
            terminal.draw(|frame| self.handle_ui(&tracker, frame, &cfg))?;
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break Ok(()),
                        KeyCode::Char('a') => {
                            self.autoscroll_enabled = !self.autoscroll_enabled;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            self.scroll_amount += 1;
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            self.scroll_amount = (self.scroll_amount as i16 - 1).max(0) as u16;
                        }
                        KeyCode::Char('r') => {
                            self.scroll_amount = 0;
                        }
                        _ => continue,
                    }
                }
            }
        }
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
            cfg,
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
                    .flat_map(|(idx, each)| {
                        let romanized = deunicode_with_tofu(&each.words, "[?]");
                        let other = romanized.clone();
                        let replace = Cow::Owned(
                            (*UNEPXECTED_CAMEL_CASE_REGEX.replace_all(
                                &other,
                                |captures: &regex::Captures| {
                                    captures
                                        .iter()
                                        .filter(Option::is_some)
                                        .map(|it| it.unwrap().as_str().to_lowercase())
                                        .collect::<String>()
                                },
                            ))
                            .to_owned(),
                        );

                        let fg_color = if idx == current_line
                            && tracker.lyrics.sync_type != LyricSyncType::Unsynced
                        {
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
                                    content: replace,
                                    style: Style::default().fg(fg_color),
                                }),
                            ]
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                tracker
                    .lyrics
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(idx, each)| {
                        if idx == current_line
                            && tracker.lyrics.sync_type != LyricSyncType::Unsynced
                        {
                            Line::from(each.words.fg(cfg.theme.lyrics.active_text_color.0))
                        } else {
                            Line::from(each.words.fg(cfg.theme.lyrics.inactive_text_color.0))
                        }
                    })
                    .collect::<Vec<_>>()
            };
            lines
        };

        let scroll_y =
            if self.autoscroll_enabled && tracker.lyrics.sync_type == LyricSyncType::LineSynced {
                let max_y_height = horizontal_layout[0].height as i16;
                let y_offset = cfg.general.scroll_offset as i16;
                (current_line as i16 - y_offset).clamp(0, (text.len() as i16 - max_y_height).max(0))
                    as u16
            } else {
                self.scroll_amount
            };

        let lyrics_part = Paragraph::new(text)
            .style(Style::default())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
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
            let text_color = cfg.theme.borders.info_text_color.0;
            info_vec
                .iter_mut()
                .for_each(|line| line.patch_style(Style::default().fg(text_color)));
            self.old_tracker_hash = tracker.identity_hash();
            self.cached_info_vec = info_vec.clone();
            info_vec
        };

        let right_side_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .split(horizontal_layout[1]);

        let info_part = Paragraph::new(info_vec)
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false })
            .block(info_block);
        f.render_widget(info_part, right_side_layout[0]);

        let controls_block = Block::new()
            .fg(cfg.theme.borders.info_border_color.0)
            .title("Controls".fg(cfg.theme.borders.info_border_text_color.0))
            .borders(Borders::ALL)
            .border_type(cfg.theme.borders.info_border_style.0)
            .title_alignment(Alignment::Left);
        let autoscroll = format!(
            "Autoscroll: {}",
            if self.autoscroll_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        let controls_part = Paragraph::new(vec![
            Line::from(autoscroll.fg(cfg.theme.borders.info_text_color.0)),
            Line::from(vec![
                "q".bg(cfg.theme.borders.info_text_color.0),
                "   - Quit".fg(cfg.theme.borders.info_text_color.0),
            ]),
            Line::from(vec![
                "j".bg(cfg.theme.borders.info_text_color.0),
                "/".fg(cfg.theme.borders.info_text_color.0),
                "k".bg(cfg.theme.borders.info_text_color.0),
                " - Scroll (down/up)".fg(cfg.theme.borders.info_text_color.0),
            ]),
            Line::from(vec![
                "a".bg(cfg.theme.borders.info_text_color.0),
                "   - Toggle autoscroll".fg(cfg.theme.borders.info_text_color.0),
            ]),
            Line::from(vec![
                "r".bg(cfg.theme.borders.info_text_color.0),
                "   - Reset scroll".fg(cfg.theme.borders.info_text_color.0),
            ]),
        ])
        .style(Style::default())
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(controls_block);
        f.render_widget(controls_part, right_side_layout[1]);

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

#[inline]
fn fmt_duration(duration_ms: u32) -> String {
    let mut seconds = duration_ms / 1000;
    let minutes = seconds / 60;
    seconds %= 60;
    format!("{minutes:0>2}:{seconds:0>2}")
}

#[inline]
fn setup_terminal() -> anyhow::Result<Term> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen,)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

#[inline]
fn restore_terminal(terminal: &mut Term) -> anyhow::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor().map_err(anyhow::Error::from)
}

#[inline]
fn maybe_romanize_str(name: &str, language: &str, cfg: &Arc<VersesConfig>) -> String {
    if cfg.general.romanize_unicode
        && cfg.general.romanize_track_names
        && language != "en"
        && !cfg.general.romanize_exclude.contains(&language.to_owned())
    {
        let romanized = deunicode(name);
        if romanized == name {
            romanized
        } else {
            format!("{name} ({romanized})")
        }
    } else {
        name.to_owned()
    }
}
