use crate::config::{CHANNELS, CHUNK_SIZE, SAMPLE_RATE};
use crate::error::SoundThisError;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Trait defining an abstract audio capture source (Open/Closed & Dependency Inversion).
#[async_trait]
pub trait AudioSource: Send + Sync {
    async fn start(&mut self) -> Result<mpsc::Receiver<Vec<u8>>, SoundThisError>;
    async fn stop(&mut self) -> Result<(), SoundThisError>;
}

/// Linux PulseAudio / PipeWire capture source using `parec`.
pub struct ParecAudioSource {
    child: Option<Child>,
    stop_tx: Option<mpsc::Sender<()>>,
}

impl Default for ParecAudioSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ParecAudioSource {
    pub fn new() -> Self {
        Self {
            child: None,
            stop_tx: None,
        }
    }

    async fn spawn_parec() -> Result<Child, SoundThisError> {
        let rate_arg = format!("--rate={SAMPLE_RATE}");
        let channels_arg = format!("--channels={CHANNELS}");

        let attempts: [&[&str]; 3] = [
            &[
                "--format=s16le",
                &rate_arg,
                &channels_arg,
                "--device=@DEFAULT_MONITOR@",
                "--raw",
                "--latency-msec=20",
            ],
            &[
                "--format=s16le",
                &rate_arg,
                &channels_arg,
                "--device=@DEFAULT_MONITOR@",
                "--raw",
            ],
            &["--format=s16le", &rate_arg, &channels_arg],
        ];

        for (i, args) in attempts.iter().enumerate() {
            info!("Attempting audio capture with parec (profile #{})", i + 1);
            match Command::new("parec")
                .args(*args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => return Ok(child),
                Err(err) => {
                    warn!("Failed to spawn parec with profile #{}: {}", i + 1, err);
                }
            }
        }

        Err(SoundThisError::AudioCapture(
            "Could not spawn `parec` for audio monitor capture. Ensure PulseAudio/PipeWire utils are installed.".into(),
        ))
    }
}

#[async_trait]
impl AudioSource for ParecAudioSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<Vec<u8>>, SoundThisError> {
        let _ = self.stop().await;
        let mut child = Self::spawn_parec().await?;
        let mut stdout = child.stdout.take().ok_or_else(|| {
            SoundThisError::AudioCapture("Failed to capture stdout of parec".into())
        })?;

        let (data_tx, data_rx) = mpsc::channel(64);
        let (stop_tx, mut stop_rx) = mpsc::channel(1);

        self.child = Some(child);
        self.stop_tx = Some(stop_tx);

        tokio::spawn(async move {
            let mut buf = vec![0u8; CHUNK_SIZE];
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        break;
                    }
                    res = stdout.read_exact(&mut buf) => {
                        match res {
                            Ok(_) => {
                                if data_tx.send(buf.clone()).await.is_err() {
                                    // Receiver dropped
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Error reading audio capture stream: {e}");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(data_rx)
    }

    async fn stop(&mut self) -> Result<(), SoundThisError> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(()).await;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }
}

impl Drop for ParecAudioSource {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

/// In-memory mock audio source for unit tests.
pub struct MockAudioSource {
    chunks: Vec<Vec<u8>>,
}

impl MockAudioSource {
    pub fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self { chunks }
    }
}

#[async_trait]
impl AudioSource for MockAudioSource {
    async fn start(&mut self) -> Result<mpsc::Receiver<Vec<u8>>, SoundThisError> {
        let (tx, rx) = mpsc::channel(16);
        for chunk in self.chunks.drain(..) {
            let _ = tx.send(chunk).await;
        }
        Ok(rx)
    }

    async fn stop(&mut self) -> Result<(), SoundThisError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_audio_source() {
        let chunk = vec![1, 2, 3, 4];
        let mut mock = MockAudioSource::new(vec![chunk.clone()]);
        let mut rx = mock.start().await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, chunk);
        assert!(mock.stop().await.is_ok());
    }

    #[test]
    fn test_parec_source_new() {
        let source = ParecAudioSource::default();
        assert!(source.child.is_none());
        assert!(source.stop_tx.is_none());
    }

    #[tokio::test]
    async fn test_parec_source_lifecycle() {
        let mut source = ParecAudioSource::new();
        // start may succeed if parec is installed and audio daemon running, or return AudioCapture error
        match source.start().await {
            Ok(mut rx) => {
                let _ =
                    tokio::time::timeout(tokio::time::Duration::from_millis(150), rx.recv()).await;
                assert!(source.stop().await.is_ok());
            }
            Err(e) => {
                assert!(matches!(e, SoundThisError::AudioCapture(_)));
            }
        }
    }

    #[tokio::test]
    async fn test_parec_source_drop() {
        let mut source = ParecAudioSource::new();
        if let Ok(_rx) = source.start().await {
            drop(source); // Exercise Drop implementation
        }
    }
}
