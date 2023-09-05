use std::sync::{Arc, Mutex};

use crate::event::{StatusEvent, TrackMetadata};

use super::Lyrics;

pub type SyncTracker = Arc<Mutex<LyricsTracker>>;

#[derive(Debug, Clone, Default)]
pub struct LyricsTracker {
    pub lyrics: Lyrics,
    pub current_line: isize,
    pub current_progress_ms: u32,
    pub track_data: TrackMetadata,
}

pub struct VersesHandler<T: VersesBackend> {
    tracker: SyncTracker,
    backend: T,
}

#[async_trait::async_trait]
pub trait VersesBackend {
    async fn run_backend(&self, tracker: SyncTracker) -> anyhow::Result<()>;
}

impl<T: VersesBackend + Send + Sync + 'static> VersesHandler<T> {
    pub fn new(backend: T) -> Self {
        Self {
            tracker: Arc::new(Mutex::new(Default::default())),
            backend,
        }
    }

    pub async fn run(self, event_rx: flume::Receiver<StatusEvent>) -> anyhow::Result<()> {
        let tracker_w = self.tracker.clone();
        let tracker_r = self.tracker;
        let event_handler =
            tokio::task::spawn(async move { Self::run_event_handler(tracker_w, event_rx).await });
        let backend_handler =
            tokio::task::spawn(async move { self.backend.run_backend(tracker_r).await });
        let (events, backend) = tokio::join!(event_handler, backend_handler);
        events??;
        backend??;
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
}
