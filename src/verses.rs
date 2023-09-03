use reqwest::{Client, StatusCode};
use rspotify::{model::CurrentlyPlayingContext, prelude::*, AuthCodePkceSpotify};
use serde::Deserialize;

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
        let track = self.currently_playing().await?.unwrap();
        let id = track.item.unwrap();
        let id = id.id().unwrap();
        let id = id.id();
        let lyrics = self.fetch_lyrics(id).await;
        println!("{lyrics:#?}");
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

#[derive(Debug, Clone, Deserialize)]
struct LyricsObject {
    lyrics: Lyrics,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LyricSyncType {
    Unsynced,
    LineSynced,
}
