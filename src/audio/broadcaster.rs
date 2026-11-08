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

impl Default for AudioBroadcaster {
    fn default() -> Self {
        Self::new()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::source::MockAudioSource;
    use crate::config::Protocol;

    #[tokio::test]
    async fn test_broadcaster_with_mock_source() {
        let broadcaster = AudioBroadcaster::default();
        let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);

        let mut sub = broadcaster.subscribe();
        let mock_chunks = vec![vec![10, 20, 30], vec![40, 50, 60]];
        let source = MockAudioSource::new(mock_chunks);

        assert!(broadcaster
            .start_broadcasting(source, state.clone())
            .is_ok());

        let chunk1 = tokio::time::timeout(tokio::time::Duration::from_millis(200), sub.recv())
            .await
            .expect("Timeout waiting for chunk 1")
            .expect("Recv error");
        assert_eq!(*chunk1, vec![10, 20, 30]);

        let chunk2 = tokio::time::timeout(tokio::time::Duration::from_millis(200), sub.recv())
            .await
            .expect("Timeout waiting for chunk 2")
            .expect("Recv error");
        assert_eq!(*chunk2, vec![40, 50, 60]);

        // Broadcast manual chunk
        broadcaster.broadcast_chunk(Arc::new(vec![99]));
        let chunk3 = sub.recv().await.unwrap();
        assert_eq!(*chunk3, vec![99]);
    }
}
