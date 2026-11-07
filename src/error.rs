use std::fmt;

#[derive(Debug)]
pub enum SoundThisError {
    AudioCapture(String),
    Network(String),
    Installer(String),
    Io(std::io::Error),
}

impl std::error::Error for SoundThisError {}

impl fmt::Display for SoundThisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AudioCapture(msg) => write!(f, "Audio capture error: {msg}"),
            Self::Network(msg) => write!(f, "Network error: {msg}"),
            Self::Installer(msg) => write!(f, "Installer error: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl From<std::io::Error> for SoundThisError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
