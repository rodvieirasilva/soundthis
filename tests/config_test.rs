use soundthis::config::{Protocol, ServerConfig};

#[test]
fn test_config_defaults_and_parsing() {
    let default_cfg = ServerConfig::default();
    assert_eq!(default_cfg.port, 9697);
    assert_eq!(default_cfg.host, "0.0.0.0");
    assert_eq!(default_cfg.protocol, Protocol::Http);
    assert!(!default_cfg.protocol.is_https());
    assert_eq!(default_cfg.protocol.scheme(), "http");
    assert_eq!(default_cfg.protocol.ws_scheme(), "ws");
    assert!(default_cfg.tls.auto_generate);
    assert_eq!(default_cfg.cors.allow_origins, vec!["*"]);
    assert_eq!(default_cfg.audio.sample_rate, 48000);
    assert_eq!(default_cfg.audio.channels, 2);

    // Custom TOML parsing
    let custom_toml = r#"
        port = 8888
        host = "127.0.0.1"
        protocol = "https"

        [tls]
        auto_generate = false

        [cors]
        allow_origins = ["https://myapp.com"]
        allow_methods = ["GET", "POST"]
        allow_headers = ["Authorization", "Content-Type"]

        [audio]
        sample_rate = 44100
        channels = 1
        chunk_size = 1024
        latency_ms = 10
    "#;

    let parsed: ServerConfig = toml::from_str(custom_toml).unwrap();
    assert_eq!(parsed.port, 8888);
    assert_eq!(parsed.host, "127.0.0.1");
    assert_eq!(parsed.protocol, Protocol::Https);
    assert!(parsed.protocol.is_https());
    assert_eq!(parsed.protocol.scheme(), "https");
    assert_eq!(parsed.protocol.ws_scheme(), "wss");
    assert!(!parsed.tls.auto_generate);
    assert_eq!(parsed.cors.allow_origins, vec!["https://myapp.com"]);
    assert_eq!(parsed.audio.sample_rate, 44100);
    assert_eq!(parsed.audio.channels, 1);

    // Default template validity
    let template = ServerConfig::default_toml_template();
    let from_template: Result<ServerConfig, _> = toml::from_str(template);
    assert!(from_template.is_ok());
}

#[test]
fn test_tls_self_signed_cert_generation() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_test_certs_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let config = ServerConfig {
        protocol: Protocol::Https,
        tls: soundthis::config::TlsConfig {
            auto_generate: true,
            cert_path: Some(temp_dir.join("test_cert.pem")),
            key_path: Some(temp_dir.join("test_key.pem")),
        },
        ..Default::default()
    };

    // Should generate valid certs
    let cert_result = config.resolve_tls_pem("192.168.1.42");
    assert!(cert_result.is_ok());
    let (cert_pem, key_pem) = cert_result.unwrap();

    let cert_str = String::from_utf8(cert_pem).unwrap();
    let key_str = String::from_utf8(key_pem).unwrap();

    assert!(cert_str.contains("BEGIN CERTIFICATE"));
    assert!(key_str.contains("BEGIN PRIVATE KEY"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_config_load_or_create_file() {
    let temp_dir = std::env::temp_dir().join(format!("soundthis_cfg_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let cfg_path = temp_dir.join("config.toml");

    // File doesn't exist -> returns default
    let cfg = ServerConfig::load_or_create(Some(&cfg_path)).unwrap();
    assert_eq!(cfg.port, 9697);

    // Default path when explicit_path is None
    let default_cfg = ServerConfig::load_or_create(None);
    assert!(default_cfg.is_ok());

    // Write corrupted TOML
    std::fs::write(&cfg_path, "invalid [[ toml syntax").unwrap();
    let err = ServerConfig::load_or_create(Some(&cfg_path));
    assert!(err.is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_config_tls_existing_files_and_missing_no_auto() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_tls_branch_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let cert_file = temp_dir.join("cert.pem");
    let key_file = temp_dir.join("key.pem");
    std::fs::write(&cert_file, b"FAKE CERTIFICATE PEM DATA").unwrap();
    std::fs::write(&key_file, b"FAKE PRIVATE KEY PEM DATA").unwrap();

    let mut cfg = ServerConfig::default();
    cfg.tls.cert_path = Some(cert_file.clone());
    cfg.tls.key_path = Some(key_file.clone());
    cfg.tls.auto_generate = false;

    // Should read pre-existing cert and key
    let (cert, key) = cfg.resolve_tls_pem("127.0.0.1").unwrap();
    assert_eq!(cert, b"FAKE CERTIFICATE PEM DATA");
    assert_eq!(key, b"FAKE PRIVATE KEY PEM DATA");

    // When files do not exist and auto_generate is false
    let missing_cert = temp_dir.join("nonexistent_cert.pem");
    let missing_key = temp_dir.join("nonexistent_key.pem");
    cfg.tls.cert_path = Some(missing_cert);
    cfg.tls.key_path = Some(missing_key);

    let err = cfg.resolve_tls_pem("127.0.0.1");
    assert!(err.is_err());

    // Config directories
    assert!(ServerConfig::config_dir()
        .to_string_lossy()
        .contains(".config/soundthis"));
    assert!(ServerConfig::default_config_path()
        .to_string_lossy()
        .contains("config.toml"));
    assert!(ServerConfig::certs_dir()
        .to_string_lossy()
        .contains("certs"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_template_and_asset_validations() {
    // Verify external config template validation
    assert!(ServerConfig::validate_template().is_ok());

    // Verify external installer templates validation
    assert!(soundthis::installer::Installer::validate_templates().is_ok());

    // Verify external asset validation
    assert!(soundthis::server::http::validate_assets().is_ok());
}
