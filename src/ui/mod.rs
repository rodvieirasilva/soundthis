use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GuiConfig {
    #[serde(default)]
    pub keep_streaming: bool,
}

pub fn get_gui_config_path() -> PathBuf {
    crate::config::ServerConfig::default_config_path()
}

pub fn read_gui_config() -> GuiConfig {
    GuiConfig {
        keep_streaming: crate::config::ServerConfig::read_keep_streaming(),
    }
}

pub fn read_gui_config_from(path: &std::path::Path) -> GuiConfig {
    if let Ok(content) = std::fs::read_to_string(path) {
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            if let Ok(cfg) = toml::from_str::<crate::config::ServerConfig>(&content) {
                return GuiConfig {
                    keep_streaming: cfg.desktop.keep_streaming,
                };
            }
        }
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        GuiConfig::default()
    }
}

pub fn save_gui_config(cfg: &GuiConfig) -> Result<(), std::io::Error> {
    crate::config::ServerConfig::set_keep_streaming(cfg.keep_streaming)
        .map_err(std::io::Error::other)
}

pub fn save_gui_config_to(path: &std::path::Path, cfg: &GuiConfig) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let updated = crate::config::update_keep_streaming_in_toml(&existing, cfg.keep_streaming);
        std::fs::write(path, updated)
    } else {
        let json = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
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

pub fn get_gui_pid_file() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("soundthis_gui.pid")
    } else {
        std::env::temp_dir().join("soundthis_gui.pid")
    }
}

pub fn is_gui_running() -> bool {
    is_gui_running_from(&get_gui_pid_file())
}

pub fn is_gui_running_from(pid_file: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(pid_file) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid: u32 = match content.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(pid_file);
            return false;
        }
    };

    let cmdline_path = format!("/proc/{pid}/cmdline");
    match std::fs::read_to_string(&cmdline_path) {
        Ok(cmdline) => {
            let matches = cmdline.contains("qml")
                || cmdline.contains("soundthis")
                || cmdline.contains("cargo")
                || cmdline.contains("test");
            if matches {
                true
            } else {
                let _ = std::fs::remove_file(pid_file);
                false
            }
        }
        Err(_) => {
            let _ = std::fs::remove_file(pid_file);
            false
        }
    }
}

pub fn focus_existing_window() -> bool {
    // 1. Try omarchy-hyprland-focus-app (native Omarchy helper)
    if let Ok(status) = Command::new("omarchy-hyprland-focus-app")
        .arg("SoundThis")
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 2. Try hyprctl dispatch hl.dsp.focus (Hyprland 0.56+ Lua API)
    if let Ok(status) = Command::new("hyprctl")
        .args(["dispatch", "hl.dsp.focus({ window = \"title:SoundThis\" })"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 3. Try hyprctl dispatch focuswindow (Legacy Hyprland dispatcher)
    if let Ok(status) = Command::new("hyprctl")
        .args(["dispatch", "focuswindow", "title:SoundThis"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 4. Try wmctrl (X11 / XWayland fallback)
    if let Ok(status) = Command::new("wmctrl")
        .args(["-F", "-a", "SoundThis"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 5. Try xdotool fallback
    if let Ok(status) = Command::new("xdotool")
        .args(["search", "--name", "^SoundThis$", "windowactivate"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    false
}

#[cfg(feature = "gui")]
pub mod gtk_app;

#[cfg(feature = "gui")]
pub fn run_desktop_ui(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    gtk_app::run_gtk_app(port)
}

#[cfg(not(feature = "gui"))]
pub fn run_desktop_ui(_port: u16) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Notice: SoundThis compiled without native GUI support.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(p.ends_with("config.toml"));
        let initial = read_gui_config();
        let _ = save_gui_config(&initial);
        let reloaded = read_gui_config();
        assert_eq!(initial, reloaded);

        let pid_path = get_gui_pid_file();
        assert!(pid_path.ends_with("soundthis_gui.pid"));
        let _ = is_gui_running();
    }

    #[test]
    fn test_read_missing_config_returns_default() {
        let missing = PathBuf::from("/non/existent/path/gui.json");
        let cfg = read_gui_config_from(&missing);
        assert_eq!(cfg, GuiConfig::default());
    }

    #[test]
    fn test_copy_to_clipboard_does_not_panic() {
        // Even if wl-copy / xclip fails or succeeds, it shouldn't panic
        let _ = copy_to_clipboard("test_stream_url");
    }

    #[test]
    fn test_is_gui_running_missing_file() {
        let tmp = std::env::temp_dir().join(format!("test_missing_pid_{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        assert!(!is_gui_running_from(&tmp));
    }

    #[test]
    fn test_is_gui_running_invalid_pid() {
        let tmp = std::env::temp_dir().join(format!("test_invalid_pid_{}.pid", std::process::id()));
        std::fs::write(&tmp, "not_a_number").unwrap();
        assert!(!is_gui_running_from(&tmp));
        assert!(!tmp.exists());
    }

    #[test]
    fn test_is_gui_running_dead_pid() {
        let tmp = std::env::temp_dir().join(format!("test_dead_pid_{}.pid", std::process::id()));
        std::fs::write(&tmp, "9999999").unwrap();
        assert!(!is_gui_running_from(&tmp));
        assert!(!tmp.exists());
    }

    #[test]
    fn test_is_gui_running_active_test_process() {
        let tmp = std::env::temp_dir().join(format!("test_active_pid_{}.pid", std::process::id()));
        std::fs::write(&tmp, std::process::id().to_string()).unwrap();
        assert!(is_gui_running_from(&tmp));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_focus_existing_window_does_not_panic() {
        let _ = focus_existing_window();
    }
}
