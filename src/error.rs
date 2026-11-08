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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let err1 = SoundThisError::AudioCapture("mic failed".into());
        assert_eq!(format!("{err1}"), "Audio capture error: mic failed");

        let err2 = SoundThisError::Network("timeout".into());
        assert_eq!(format!("{err2}"), "Network error: timeout");

        let err3 = SoundThisError::Installer("write failed".into());
        assert_eq!(format!("{err3}"), "Installer error: write failed");

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err4 = SoundThisError::from(io_err);
        assert!(format!("{err4}").contains("file not found"));
    }
}
