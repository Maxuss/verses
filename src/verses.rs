pub mod handler;
pub mod tui_backend;

use std::{sync::Arc, time::Duration};

use reqwest::{Client, StatusCode};
use rspotify::{
    model::{CurrentlyPlayingContext, FullArtist, FullTrack, TrackId},
    prelude::*,
    AuthCodePkceSpotify,
};
use serde::Deserialize;

use crate::{
    config::VersesConfig,
    event::{StatusEvent, TrackMetadata},
    verses::handler::VersesHandler,
};

use self::tui_backend::TerminalUiBackend;

#[derive(Debug, Clone)]
pub struct Verses {
    spotify: AuthCodePkceSpotify,
    client: reqwest::Client,
    config: Arc<VersesConfig>,
}

impl Verses {
    pub fn new(spotify: AuthCodePkceSpotify, config: Arc<VersesConfig>) -> Self {
        let client = Client::new();
        Self {
            spotify,
            client,
            config,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (events_tx, events_rx) = flume::bounded::<StatusEvent>(4);

        // TODO: other backend choices?
        let tui = VersesHandler::new(TerminalUiBackend::new());

        let cfg_clone_backend: Arc<VersesConfig> = self.config.clone();
        tokio::task::spawn(async move { self.run_dispatcher(events_tx).await });
        tui.run(events_rx, cfg_clone_backend).await?;

        Ok(())
    }

    async fn run_dispatcher(&self, events_tx: flume::Sender<StatusEvent>) -> anyhow::Result<()> {
        let mut cached_id: String = String::new();
        let mut cached_lyrics: Option<Lyrics> = None;
        let mut current_lyrics_line: isize = 0;

        // Fetching status every second
        while let Ok(status) = self.currently_playing().await {
            if let Some(status) = status {
                let item = if let Some(item) = status.item {
                    item
                } else {
                    continue;
                };
                let id = item.id().unwrap();
                let id = id.id().to_owned();

                if cached_id == id {
                    // we are playing the same song, no need to send an update event
                    if let Some(lyrics) = &cached_lyrics {
                        let progress = status
                            .progress
                            .map(|it| it.to_std().unwrap())
                            .unwrap_or(Duration::ZERO);
                        let progress_ms = progress.as_millis() as u32;

                        // sending new progress
                        events_tx
                            .send_async(StatusEvent::TrackProgress {
                                new_progress_ms: progress_ms,
                            })
                            .await?;

                        // finding floor line index
                        let lyrics_line_index: isize = lyrics.lines.len() as isize
                            - 1
                            - lyrics
                                .lines
                                .iter()
                                .rev()
                                .position(|it| it.start_time_ms <= progress_ms)
                                .unwrap_or(lyrics.lines.len())
                                as isize;
                        if current_lyrics_line != lyrics_line_index {
                            events_tx
                                .send_async(StatusEvent::SwitchLyricLine {
                                    new_line: lyrics_line_index as isize,
                                })
                                .await?;
                            current_lyrics_line = lyrics_line_index as isize;
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                } else {
                    cached_id = id.clone();
                    current_lyrics_line = -1; // resetting lyrics in case song was switched
                }

                // setting track progress to 0 since we are listening to a new track
                events_tx
                    .send_async(StatusEvent::TrackProgress { new_progress_ms: 0 })
                    .await?;

                let track = self.spotify.track(TrackId::from_id(&id)?, None).await?;
                let main_artist = track.artists.get(0).unwrap();
                let main_artist = self.spotify.artist(main_artist.id.clone().unwrap()).await?;
                let metadata = extract_track_meta(track, main_artist);

                let lyrics = self.fetch_lyrics(&id).await?;
                if let Some(new_lyrics) = lyrics {
                    cached_lyrics = Some(new_lyrics.clone());
                    events_tx
                        .send_async(StatusEvent::NewTrack {
                            metadata,
                            new_lyrics,
                        })
                        .await?;
                } else {
                    events_tx
                        .send_async(StatusEvent::NewTrackNoLyrics { metadata })
                        .await?;
                }
            } else {
                // Not playing anything, retry later
                continue;
            }
        }
        Ok(())
    }

    async fn fetch_lyrics(&self, track_id: &str) -> anyhow::Result<Option<Lyrics>> {
        let resp = self
            .client
            .get(format!("{}{track_id}", self.config.api.lyricstify_api_url))
            .send()
            .await?;
        if resp.status() == StatusCode::NOT_FOUND {
            // Song does not have lyrics
            Ok(None)
        } else {
            resp.json::<LyricsObject>()
                .await
                .map_err(anyhow::Error::from)
                .map(|it| Some(it.lyrics))
        }
    }

    async fn currently_playing(&self) -> anyhow::Result<Option<CurrentlyPlayingContext>> {
        self.spotify.auto_reauth().await?;
        self.spotify
            .current_playing(None, None::<Vec<_>>)
            .await
            .map_err(anyhow::Error::from)
    }
}

fn extract_track_meta(track: FullTrack, artist: FullArtist) -> TrackMetadata {
    TrackMetadata {
        track_name: track.name,
        track_artists: track.artists.into_iter().map(|each| each.name).collect(),
        track_album: track.album.name,
        track_duration: track.duration.to_std().unwrap(),
        artist_genres: artist.genres,
        popularity: track.popularity,
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LyricsObject {
    lyrics: Lyrics,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Lyrics {
    #[serde(rename = "syncType")]
    pub sync_type: LyricSyncType,
    pub lines: Vec<LyricLine>,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LyricLine {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: u32,
    pub words: String,
}

#[derive(Debug, Clone, Deserialize, Default, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LyricSyncType {
    #[default]
    Unsynced,
    LineSynced,
}
