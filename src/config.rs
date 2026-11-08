use crate::error::SoundThisError;
use rcgen::generate_simple_self_signed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub const DEFAULT_PORT: u16 = 9697;
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: u16 = 2;
// 960 frames * 4 bytes per sample (16-bit stereo) = 3840 bytes (~20ms of audio)
pub const CHUNK_SIZE: usize = 3840;
pub const BROADCAST_CAPACITY: usize = 64;
pub const CONFIG_TEMPLATE: &str = include_str!("../config/config.toml.template");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Http,
    Https,
}

impl Protocol {
    pub fn is_https(&self) -> bool {
        matches!(self, Self::Https)
    }

    pub fn scheme(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    pub fn ws_scheme(&self) -> &'static str {
        match self {
            Self::Http => "ws",
            Self::Https => "wss",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub auto_generate: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            auto_generate: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default = "default_origins")]
    pub allow_origins: Vec<String>,
    #[serde(default = "default_methods")]
    pub allow_methods: Vec<String>,
    #[serde(default = "default_headers")]
    pub allow_headers: Vec<String>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origins: default_origins(),
            allow_methods: default_methods(),
            allow_headers: default_headers(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_channels")]
    pub channels: u16,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_latency_ms")]
    pub latency_ms: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: default_sample_rate(),
            channels: default_channels(),
            chunk_size: default_chunk_size(),
            latency_ms: default_latency_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default)]
    pub protocol: Protocol,

    #[serde(default)]
    pub tls: TlsConfig,

    #[serde(default)]
    pub cors: CorsConfig,

    #[serde(default)]
    pub audio: AudioConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            protocol: Protocol::default(),
            tls: TlsConfig::default(),
            cors: CorsConfig::default(),
            audio: AudioConfig::default(),
        }
    }
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}

fn default_true() -> bool {
    true
}

fn default_origins() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_methods() -> Vec<String> {
    vec!["GET".to_string(), "POST".to_string(), "OPTIONS".to_string()]
}

fn default_headers() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_sample_rate() -> u32 {
    SAMPLE_RATE
}

fn default_channels() -> u16 {
    CHANNELS
}

fn default_chunk_size() -> usize {
    CHUNK_SIZE
}

fn default_latency_ms() -> u32 {
    20
}

impl ServerConfig {
    pub fn config_dir() -> PathBuf {
        let home = dirs_home().unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".config/soundthis")
    }

    pub fn default_config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn certs_dir() -> PathBuf {
        Self::config_dir().join("certs")
    }

    pub fn load_or_create(explicit_path: Option<&Path>) -> Result<Self, SoundThisError> {
        let path = match explicit_path {
            Some(p) => p.to_path_buf(),
            None => Self::default_config_path(),
        };

        if !path.exists() {
            if explicit_path.is_none() {
                Self::validate_template()?;
                // Ensure directory exists and write default config with explanatory comments
                let _ = fs::create_dir_all(Self::config_dir());
                let default_content = Self::default_toml_template();
                if let Err(e) = fs::write(&path, default_content) {
                    warn!(
                        "Could not write default config to {}: {}",
                        path.display(),
                        e
                    );
                } else {
                    info!("Created default configuration file at {}", path.display());
                }
            }
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path).map_err(SoundThisError::Io)?;

        toml::from_str(&content).map_err(|e| {
            SoundThisError::Installer(format!(
                "Failed to parse TOML configuration from {}: {e}",
                path.display()
            ))
        })
    }

    pub fn validate_template() -> Result<(), SoundThisError> {
        let parsed: Self = toml::from_str(CONFIG_TEMPLATE).map_err(|e| {
            SoundThisError::Installer(format!("Default config template is invalid TOML: {e}"))
        })?;

        if parsed.port == 0 {
            return Err(SoundThisError::Installer(
                "Config template must specify a non-zero port".into(),
            ));
        }

        if !CONFIG_TEMPLATE.contains("[tls]")
            || !CONFIG_TEMPLATE.contains("[cors]")
            || !CONFIG_TEMPLATE.contains("[audio]")
        {
            return Err(SoundThisError::Installer(
                "Config template missing standard configuration sections".into(),
            ));
        }

        Ok(())
    }

    pub fn default_toml_template() -> &'static str {
        CONFIG_TEMPLATE
    }

    pub fn resolve_tls_pem(&self, lan_ip: &str) -> Result<(Vec<u8>, Vec<u8>), SoundThisError> {
        let (cert_file, key_file) = match (&self.tls.cert_path, &self.tls.key_path) {
            (Some(c), Some(k)) => (c.clone(), k.clone()),
            _ => {
                let dir = Self::certs_dir();
                (dir.join("cert.pem"), dir.join("key.pem"))
            }
        };

        if cert_file.exists() && key_file.exists() {
            let cert_bytes = fs::read(&cert_file).map_err(|e| {
                SoundThisError::Network(format!(
                    "Failed to read cert file {}: {e}",
                    cert_file.display()
                ))
            })?;
            let key_bytes = fs::read(&key_file).map_err(|e| {
                SoundThisError::Network(format!(
                    "Failed to read key file {}: {e}",
                    key_file.display()
                ))
            })?;
            return Ok((cert_bytes, key_bytes));
        }

        if self.tls.auto_generate {
            if let Some(parent) = cert_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Some(parent) = key_file.parent() {
                let _ = fs::create_dir_all(parent);
            }

            info!(
                "Auto-generating self-signed TLS certificates for localhost and {}",
                lan_ip
            );
            let subject_alt_names = vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                lan_ip.to_string(),
            ];

            let cert = generate_simple_self_signed(subject_alt_names).map_err(|e| {
                SoundThisError::Network(format!("Failed to generate self-signed cert: {e}"))
            })?;

            let cert_pem = cert.cert.pem();
            let key_pem = cert.signing_key.serialize_pem();

            fs::write(&cert_file, &cert_pem)?;
            fs::write(&key_file, &key_pem)?;
            info!(
                "Saved self-signed TLS certificates to {}",
                cert_file.display()
            );

            return Ok((cert_pem.into_bytes(), key_pem.into_bytes()));
        }

        Err(SoundThisError::Network(format!(
            "Certificate file {} or key file {} not found and auto_generate is false",
            cert_file.display(),
            key_file.display()
        )))
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
