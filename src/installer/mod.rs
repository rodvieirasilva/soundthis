pub mod systemd;

use crate::error::SoundThisError;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use systemd::SystemdManager;
use tracing::info;

pub const PLUGIN_MANIFEST: &str = include_str!("../../plugin/manifest.json");
pub const PLUGIN_PANEL_QML: &str = include_str!("../../plugin/Panel.qml");
pub const SERVICE_TEMPLATE: &str = include_str!("../../systemd/soundthis.service.template");

pub struct Installer;

impl Installer {
    pub fn validate_templates() -> Result<(), SoundThisError> {
        // Validate systemd service template
        if !SERVICE_TEMPLATE.contains("[Unit]")
            || !SERVICE_TEMPLATE.contains("[Service]")
            || !SERVICE_TEMPLATE.contains("[Install]")
        {
            return Err(SoundThisError::Installer(
                "Service template missing standard systemd sections".into(),
            ));
        }
        if !SERVICE_TEMPLATE.contains("%BIN_PATH%") {
            return Err(SoundThisError::Installer(
                "Service template missing %BIN_PATH% placeholder".into(),
            ));
        }

        // Validate plugin manifest
        let manifest: serde_json::Value = serde_json::from_str(PLUGIN_MANIFEST).map_err(|e| {
            SoundThisError::Installer(format!("Plugin manifest is not valid JSON: {e}"))
        })?;
        if manifest.get("id").and_then(|v| v.as_str()) != Some("soundthis") {
            return Err(SoundThisError::Installer(
                "Plugin manifest does not define id 'soundthis'".into(),
            ));
        }

        // Validate Panel.qml
        if !PLUGIN_PANEL_QML.contains("Panel") || !PLUGIN_PANEL_QML.contains("soundthis") {
            return Err(SoundThisError::Installer(
                "Plugin Panel.qml appears incomplete or corrupted".into(),
            ));
        }

        Ok(())
    }

    pub fn home_dir() -> PathBuf {
        dirs_home().unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
        })
    }

    pub fn resolve_bin_path() -> PathBuf {
        let home = Self::home_dir();
        let cargo_bin = home.join(".cargo/bin/soundthis");
        if cargo_bin.exists() {
            return cargo_bin;
        }

        let local_bin = home.join(".local/bin/soundthis");
        if local_bin.exists() {
            return local_bin;
        }

        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("soundthis"))
    }

    pub fn install() -> Result<(), SoundThisError> {
        let home = Self::home_dir();
        let bin_path = Self::resolve_bin_path();
        Self::install_to(&home, &bin_path, true)?;

        println!("\n✅ SoundThis successfully installed!");
        println!("   Server: Running on port 9697 via systemd user service");
        println!("   Plugin: Installed in Omarchy bar");
        println!("   Status: Run `soundthis status` to check streaming status\n");

        Ok(())
    }

    pub fn install_to(
        home: &Path,
        bin_path: &Path,
        manage_systemd: bool,
    ) -> Result<(), SoundThisError> {
        Self::validate_templates()?;
        info!("Installing SoundThis using binary: {}", bin_path.display());

        // 1. Install user systemd service
        let systemd_dir = home.join(".config/systemd/user");
        fs::create_dir_all(&systemd_dir)?;
        let service_file = systemd_dir.join("soundthis.service");
        let service_content = SERVICE_TEMPLATE.replace("%BIN_PATH%", &bin_path.to_string_lossy());
        fs::write(&service_file, service_content)?;
        info!("Wrote systemd unit to {}", service_file.display());

        if manage_systemd {
            SystemdManager::daemon_reload()?;
            SystemdManager::enable_and_start("soundthis.service")?;
        }

        // 2. Install Omarchy plugin
        let plugin_dir = home.join(".config/omarchy/plugins/soundthis");
        fs::create_dir_all(&plugin_dir)?;
        fs::write(plugin_dir.join("manifest.json"), PLUGIN_MANIFEST)?;
        fs::write(plugin_dir.join("Panel.qml"), PLUGIN_PANEL_QML)?;
        info!("Installed Omarchy plugin files to {}", plugin_dir.display());

        // 3. Register widget in Omarchy shell.json
        let shell_config_file = home.join(".config/omarchy/shell.json");
        if shell_config_file.exists() {
            if let Ok(content) = fs::read_to_string(&shell_config_file) {
                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(right_layout) = json
                        .get_mut("bar")
                        .and_then(|b| b.get_mut("layout"))
                        .and_then(|l| l.get_mut("right"))
                        .and_then(|r| r.as_array_mut())
                    {
                        let already_exists = right_layout.iter().any(|item| {
                            item.as_str() == Some("soundthis")
                                || item.get("id").and_then(|v| v.as_str()) == Some("soundthis")
                        });
                        if !already_exists {
                            let soundthis_val = serde_json::json!({ "id": "soundthis" });
                            right_layout.insert(0, soundthis_val);
                            if let Ok(updated) = serde_json::to_string_pretty(&json) {
                                let _ = fs::write(&shell_config_file, updated);
                                info!("Added soundthis widget to bar.layout.right in shell.json");
                            }
                        }
                    }
                }
            }
        }

        // 4. Reload Omarchy shell if requested
        if manage_systemd {
            info!("Reloading Omarchy shell...");
            let _ = Command::new("omarchy").args(["restart", "shell"]).status();
        }

        Ok(())
    }

    pub fn uninstall() -> Result<(), SoundThisError> {
        let home = Self::home_dir();
        Self::uninstall_from(&home, true)?;
        println!("\n🗑️ SoundThis successfully uninstalled!\n");
        Ok(())
    }

    pub fn uninstall_from(home: &Path, manage_systemd: bool) -> Result<(), SoundThisError> {
        info!("Uninstalling SoundThis...");

        // 1. Stop systemd daemon
        if manage_systemd {
            let _ = SystemdManager::stop_and_disable("soundthis.service");
        }
        let service_file = home.join(".config/systemd/user/soundthis.service");
        if service_file.exists() {
            let _ = fs::remove_file(&service_file);
            if manage_systemd {
                let _ = SystemdManager::daemon_reload();
            }
            info!("Removed {}", service_file.display());
        }

        // 2. Remove plugin files
        let plugin_dir = home.join(".config/omarchy/plugins/soundthis");
        if plugin_dir.exists() {
            let _ = fs::remove_dir_all(&plugin_dir);
            info!("Removed {}", plugin_dir.display());
        }

        // 3. Remove from Omarchy shell.json
        let shell_config_file = home.join(".config/omarchy/shell.json");
        if shell_config_file.exists() {
            if let Ok(content) = fs::read_to_string(&shell_config_file) {
                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(right_layout) = json
                        .get_mut("bar")
                        .and_then(|b| b.get_mut("layout"))
                        .and_then(|l| l.get_mut("right"))
                        .and_then(|r| r.as_array_mut())
                    {
                        right_layout.retain(|item| {
                            item.as_str() != Some("soundthis")
                                && item.get("id").and_then(|v| v.as_str()) != Some("soundthis")
                        });
                        if let Ok(updated) = serde_json::to_string_pretty(&json) {
                            let _ = fs::write(&shell_config_file, updated);
                            info!("Removed soundthis widget from shell.json");
                        }
                    }
                }
            }
        }

        // 4. Reload Omarchy shell
        if manage_systemd {
            let _ = Command::new("omarchy").args(["restart", "shell"]).status();
        }

        Ok(())
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
