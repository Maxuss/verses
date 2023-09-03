pub mod tui;

use std::{sync::Arc, time::Duration};

use reqwest::{Client, StatusCode};
use rspotify::{
    model::{CurrentlyPlayingContext, FullTrack, TrackId},
    prelude::*,
    sync::Mutex,
    AuthCodePkceSpotify,
};
use serde::Deserialize;

use crate::{
    event::{StatusEvent, TrackMetadata},
    verses::tui::VersesTui,
};

#[derive(Debug, Clone)]
pub struct Verses {
    spotify: AuthCodePkceSpotify,
    client: reqwest::Client,
}

impl Verses {
    pub fn new(spotify: AuthCodePkceSpotify) -> Self {
        let client = Client::new();
        Self { spotify, client }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (events_tx, events_rx) = flume::bounded::<StatusEvent>(4);

        let tui = VersesTui::new();

        let tui = tokio::task::spawn(async move { tui.run(events_rx).await });
        let dispatcher = tokio::task::spawn(async move { self.run_dispatcher(events_tx).await });

        let (tui, dispatcher) = tokio::join!(tui, dispatcher);
        tui??;
        dispatcher??;

        Ok(())
    }

    async fn run_dispatcher(&self, events_tx: flume::Sender<StatusEvent>) -> anyhow::Result<()> {
        let mut cached_id: String = String::new();
        let mut cached_lyrics: Option<Lyrics> = None;
        let mut last_lyrics_line: usize = 0;

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
                        let progress_ms = progress.as_millis();
                        let next_lyrics_line = lyrics.lines.get(last_lyrics_line + 1);
                        if let Some(next_lyrics_line) = next_lyrics_line {
                            // we havent reached last line yet, so we can continue
                            if progress_ms as u32 > next_lyrics_line.start_time_ms {
                                // we have surpassed next lyric line begin time, send an update event
                                last_lyrics_line += 1;

                                // TODO: handle when player goes back
                                events_tx
                                    .send_async(StatusEvent::SwitchLyricLine {
                                        new_line: last_lyrics_line,
                                    })
                                    .await?;
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                } else {
                    cached_id = id.clone();
                    last_lyrics_line = 0; // resetting lyrics in case song was switched
                }

                let track = self.spotify.track(TrackId::from_id(&id)?, None).await?;
                let metadata = extract_track_meta(track);

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
            .get(format!(
                "https://api.lyricstify.vercel.app/v1/lyrics/{track_id}"
            ))
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

fn extract_track_meta(track: FullTrack) -> TrackMetadata {
    TrackMetadata {
        track_name: track.name,
        track_author: track
            .artists
            .into_iter()
            .map(|each| each.name)
            .collect::<Vec<String>>()
            .join(", "),
        track_album: track.album.name,
        track_duration: track.duration.to_std().unwrap(),
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LyricSyncType {
    #[default]
    Unsynced,
    LineSynced,
}
