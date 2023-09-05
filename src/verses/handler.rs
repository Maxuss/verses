use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use crate::{
    config::VersesConfig,
    event::{StatusEvent, TrackMetadata},
};

use super::Lyrics;

pub type SyncTracker = Arc<Mutex<LyricsTracker>>;

#[derive(Debug, Clone, Default)]
pub struct LyricsTracker {
    pub lyrics: Lyrics,
    pub current_line: isize,
    pub current_progress_ms: u32,
    pub track_data: TrackMetadata,
}

impl LyricsTracker {
    pub fn identity_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.lyrics.language.hash(&mut hasher);
        self.lyrics.lines.iter().for_each(|it| {
            it.words.hash(&mut hasher);
            it.start_time_ms.hash(&mut hasher)
        });
        (self.lyrics.sync_type as u8).hash(&mut hasher);
        self.track_data.popularity.hash(&mut hasher);
        self.track_data.track_artists.hash(&mut hasher);
        self.track_data.track_album.hash(&mut hasher);
        self.track_data.track_artists.hash(&mut hasher);
        self.track_data.track_name.hash(&mut hasher);

        hasher.finish()
    }
}

pub struct VersesHandler<T: VersesBackend> {
    tracker: SyncTracker,
    backend: T,
}

#[async_trait::async_trait]
pub trait VersesBackend {
    async fn run_backend(
        &mut self,
        tracker: SyncTracker,
        config: Arc<VersesConfig>,
    ) -> anyhow::Result<()>;
}

impl<T: VersesBackend + Send + Sync + 'static> VersesHandler<T> {
    pub fn new(backend: T) -> Self {
        Self {
            tracker: Arc::new(Mutex::new(Default::default())),
            backend,
        }
    }

    pub async fn run(
        mut self,
        event_rx: flume::Receiver<StatusEvent>,
        config: Arc<VersesConfig>,
    ) -> anyhow::Result<()> {
        let tracker_w = self.tracker.clone();
        let tracker_r = self.tracker;
        let event_handler =
            tokio::task::spawn(async move { Self::run_event_handler(tracker_w, event_rx).await });
        let backend_handler =
            tokio::task::spawn(async move { self.backend.run_backend(tracker_r, config).await });
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
