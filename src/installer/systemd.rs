use crate::error::SoundThisError;
use std::process::Command;
use tracing::info;

pub struct SystemdManager;

impl SystemdManager {
    pub fn daemon_reload() -> Result<(), SoundThisError> {
        let status = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()
            .map_err(|e| SoundThisError::Installer(format!("Failed to run systemctl daemon-reload: {e}")))?;

        if status.success() {
            Ok(())
        } else {
            Err(SoundThisError::Installer("systemctl daemon-reload failed".into()))
        }
    }

    pub fn enable_and_start(service_name: &str) -> Result<(), SoundThisError> {
        info!("Enabling and starting systemd service: {service_name}");
        let _ = Command::new("systemctl")
            .args(["--user", "enable", service_name])
            .status();

        let status = Command::new("systemctl")
            .args(["--user", "restart", service_name])
            .status()
            .map_err(|e| SoundThisError::Installer(format!("Failed to start/restart service {service_name}: {e}")))?;

        if status.success() {
            Ok(())
        } else {
            Err(SoundThisError::Installer(format!("Failed to start/restart {service_name}")))
        }
    }

    pub fn stop_and_disable(service_name: &str) -> Result<(), SoundThisError> {
        info!("Stopping and disabling systemd service: {service_name}");
        let _ = Command::new("systemctl")
            .args(["--user", "stop", service_name])
            .status();

        let _ = Command::new("systemctl")
            .args(["--user", "disable", service_name])
            .status();

        Ok(())
    }
}
