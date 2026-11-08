use crate::audio::source::AudioSource;
use crate::config::BROADCAST_CAPACITY;
use crate::error::SoundThisError;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

pub struct AudioBroadcaster {
    sender: broadcast::Sender<Arc<Vec<u8>>>,
}

impl AudioBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Vec<u8>>> {
        self.sender.subscribe()
    }

    pub fn broadcast_chunk(&self, chunk: Arc<Vec<u8>>) {
        let _ = self.sender.send(chunk);
    }

    pub fn start_broadcasting<S>(
        &self,
        mut source: S,
        state: AppState,
    ) -> Result<(), SoundThisError>
    where
        S: AudioSource + 'static,
    {
        let sender = self.sender.clone();

        tokio::spawn(async move {
            info!("Starting audio capture & broadcast loop...");
            match source.start().await {
                Ok(mut rx) => {
                    while let Some(chunk) = rx.recv().await {
                        // Only broadcast audio if server enabled
                        if state.is_enabled() {
                            let arc_chunk = Arc::new(chunk);
                            // It's normal if there are currently 0 active subscribers
                            let _ = sender.send(arc_chunk);
                        }
                    }
                    let _ = source.stop().await;
                }
                Err(e) => {
                    tracing::error!("Failed to start audio source: {e}");
                }
            }
        });

        Ok(())
    }
}
