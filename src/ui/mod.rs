use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use tracing::info;

pub const MAIN_QML: &str = include_str!("../../ui/Main.qml");

pub fn validate_template() {
    assert!(!MAIN_QML.is_empty(), "Embedded Main.qml must not be empty");
    assert!(
        MAIN_QML.contains("ApplicationWindow"),
        "Main.qml must define an ApplicationWindow"
    );
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GuiConfig {
    #[serde(default)]
    pub keep_streaming: bool,
}

pub fn get_gui_config_path() -> PathBuf {
    crate::config::ServerConfig::config_dir().join("gui.json")
}

pub fn read_gui_config() -> GuiConfig {
    read_gui_config_from(&get_gui_config_path())
}

pub fn read_gui_config_from(path: &std::path::Path) -> GuiConfig {
    if let Ok(content) = std::fs::read_to_string(path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        GuiConfig::default()
    }
}

pub fn save_gui_config(cfg: &GuiConfig) -> Result<(), std::io::Error> {
    save_gui_config_to(&get_gui_config_path(), cfg)
}

pub fn save_gui_config_to(path: &std::path::Path, cfg: &GuiConfig) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

pub fn copy_to_clipboard(text: &str) -> bool {
    // 1. Try wl-copy (Wayland / Omarchy default)
    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return true;
            }
        }
    }

    // 2. Try xclip (X11 fallback)
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return true;
            }
        }
    }

    false
}

pub fn get_runtime_qml_dir() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("soundthis")
    } else {
        std::env::temp_dir().join("soundthis")
    }
}

pub fn ensure_qml_file() -> Result<PathBuf, std::io::Error> {
    let dir = get_runtime_qml_dir();
    std::fs::create_dir_all(&dir)?;
    let qml_file = dir.join("Main.qml");

    // Write file if it doesn't exist or content differs
    let needs_write = match std::fs::read_to_string(&qml_file) {
        Ok(existing) => existing != MAIN_QML,
        Err(_) => true,
    };

    if needs_write {
        std::fs::write(&qml_file, MAIN_QML)?;
    }

    Ok(qml_file)
}

pub fn run_desktop_ui(_port: u16) -> Result<ExitStatus, std::io::Error> {
    validate_template();
    let qml_path = ensure_qml_file()?;
    info!("Launching SoundThis desktop UI: {}", qml_path.display());

    Command::new("qml6")
        .arg("-a")
        .arg("gui")
        .arg(&qml_path)
        .status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_template() {
        validate_template();
    }

    #[test]
    fn test_gui_config_serialization() {
        let tmp =
            std::env::temp_dir().join(format!("soundthis_gui_test_{}.json", std::process::id()));
        let mut cfg = GuiConfig::default();
        assert!(!cfg.keep_streaming);

        cfg.keep_streaming = true;
        save_gui_config_to(&tmp, &cfg).expect("save should succeed");

        let loaded = read_gui_config_from(&tmp);
        assert_eq!(loaded, cfg);
        assert!(loaded.keep_streaming);

        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn test_paths_and_default_reading() {
        let p = get_gui_config_path();
        assert!(p.ends_with("gui.json"));
        let _ = read_gui_config();

        let dir = get_runtime_qml_dir();
        assert!(dir.ends_with("soundthis"));
    }

    #[test]
    fn test_read_missing_config_returns_default() {
        let missing = PathBuf::from("/non/existent/path/gui.json");
        let cfg = read_gui_config_from(&missing);
        assert_eq!(cfg, GuiConfig::default());
    }

    #[test]
    fn test_ensure_qml_file() {
        let path = ensure_qml_file().expect("ensure_qml_file failed");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("read failed");
        assert!(content.contains("ApplicationWindow"));
    }

    #[test]
    fn test_copy_to_clipboard_does_not_panic() {
        // Even if wl-copy / xclip fails or succeeds, it shouldn't panic
        let _ = copy_to_clipboard("test_stream_url");
    }
}
