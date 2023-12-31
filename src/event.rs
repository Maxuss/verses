use std::time::Duration;

use crate::verses::Lyrics;

#[derive(Debug, Clone)]
pub enum StatusEvent {
    NewTrack {
        metadata: TrackMetadata,
        new_lyrics: Lyrics,
    },
    NewTrackNoLyrics {
        metadata: TrackMetadata,
    },
    SwitchLyricLine {
        new_line: isize,
    },
    TrackProgress {
        new_progress_ms: u32,
    },
}

#[derive(Default, Debug, Clone)]
pub struct TrackMetadata {
    pub track_name: String,
    pub track_artists: Vec<String>,
    pub track_album: String,
    pub track_duration: Duration,
    pub artist_genres: Vec<String>,
    pub popularity: u32,
}
