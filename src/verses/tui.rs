use std::sync::Arc;

use rspotify::sync::Mutex;

use crate::{
    event::{StatusEvent, TrackMetadata},
    verses::LyricLine,
};

use super::Lyrics;

#[derive(Debug, Clone, Default)]
struct LyricsTracker {
    lyrics: Lyrics,
    current_line: isize,
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
                    let mut tracker = tracker.lock().await.unwrap();
                    tracker.current_line = -1;
                    tracker.lyrics = new_lyrics;
                    tracker.track_data = metadata;
                }
                StatusEvent::SwitchLyricLine { new_line } => {
                    let mut tracker = tracker.lock().await.unwrap();
                    if new_line == -1 {
                        continue;
                    }
                    tracker.current_line = new_line as isize;

                    // TODO: debug, remove me
                    println!(
                        "{}",
                        tracker
                            .lyrics
                            .lines
                            .get(tracker.current_line as usize)
                            .unwrap_or(&LyricLine {
                                start_time_ms: 0,
                                words: String::new()
                            })
                            .words
                    )
                }
                StatusEvent::NewTrackNoLyrics { metadata } => {
                    let mut tracker = tracker.lock().await.unwrap();
                    tracker.current_line = -1;
                    tracker.lyrics.lines.clear();
                    tracker.track_data = metadata;

                    // TODO: debug, remove me
                    println!("NO LYRICS")
                }
            }
        }
        Ok(())
    }

    async fn run_tui(&self) -> anyhow::Result<()> {
        // TODO:
        Ok(())
    }
}
