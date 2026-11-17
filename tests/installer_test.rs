use soundthis::installer::systemd::SystemdManager;
use soundthis::installer::Installer;

#[test]
fn test_installer_install_and_uninstall_lifecycle() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_install_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let shell_dir = temp_dir.join(".config/omarchy");
    let _ = std::fs::create_dir_all(&shell_dir);
    let initial_shell_json = serde_json::json!({
        "bar": {
            "layout": {
                "right": ["clock", "battery"]
            }
        }
    });
    std::fs::write(
        shell_dir.join("shell.json"),
        serde_json::to_string(&initial_shell_json).unwrap(),
    )
    .unwrap();

    let fake_bin = temp_dir.join("bin/soundthis");

    // Test install_to
    assert!(Installer::install_to(&temp_dir, &fake_bin, false).is_ok());

    let service_file = temp_dir.join(".config/systemd/user/soundthis.service");
    assert!(service_file.exists());
    let service_content = std::fs::read_to_string(&service_file).unwrap();
    assert!(service_content.contains(&fake_bin.to_string_lossy().to_string()));

    let manifest_file = temp_dir.join(".config/omarchy/plugins/soundthis/manifest.json");
    assert!(manifest_file.exists());

    let panel_file = temp_dir.join(".config/omarchy/plugins/soundthis/Panel.qml");
    assert!(panel_file.exists());

    let desktop_file = temp_dir.join(".local/share/applications/soundthis.desktop");
    assert!(desktop_file.exists());
    let desktop_content = std::fs::read_to_string(&desktop_file).unwrap();
    assert!(desktop_content.contains(&fake_bin.to_string_lossy().to_string()));

    let icon_file = temp_dir.join(".local/share/icons/hicolor/scalable/apps/soundthis.svg");
    assert!(icon_file.exists());

    let hook_file = temp_dir.join(".config/omarchy/hooks/theme-set.d/soundthis");
    assert!(hook_file.exists());

    let updated_shell: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(shell_dir.join("shell.json")).unwrap())
            .unwrap();
    let right_list = updated_shell["bar"]["layout"]["right"].as_array().unwrap();
    assert!(right_list.contains(&serde_json::json!({ "id": "soundthis" })));

    // Running install_to again should be idempotent
    assert!(Installer::install_to(&temp_dir, &fake_bin, false).is_ok());

    // Test uninstall_from
    assert!(Installer::uninstall_from(&temp_dir, false).is_ok());
    assert!(!service_file.exists());
    assert!(!manifest_file.exists());
    assert!(!desktop_file.exists());
    assert!(!icon_file.exists());
    assert!(!hook_file.exists());

    let uninstalled_shell: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(shell_dir.join("shell.json")).unwrap())
            .unwrap();
    let right_list = uninstalled_shell["bar"]["layout"]["right"]
        .as_array()
        .unwrap();
    assert!(!right_list.iter().any(|item| {
        item.as_str() == Some("soundthis")
            || item.get("id").and_then(|v| v.as_str()) == Some("soundthis")
    }));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_helpers_and_systemd() {
    std::env::set_var("SOUNDTHIS_TEST", "1");
    let home = Installer::home_dir();
    assert!(!home.as_os_str().is_empty());

    let bin = Installer::resolve_bin_path();
    assert!(!bin.as_os_str().is_empty());

    // Systemd daemon-reload & stop on linux system
    let reload = SystemdManager::daemon_reload();
    assert!(reload.is_ok());

    let stop = SystemdManager::stop_and_disable("soundthis-dummy-test.service");
    assert!(stop.is_ok());

    let start_err = SystemdManager::enable_and_start("soundthis-dummy-test.service");
    assert!(start_err.is_err());
}

#[test]
fn test_installer_validate_templates() {
    assert!(Installer::validate_templates().is_ok());
}

#[test]
fn test_installer_edge_cases_and_uninstall_empty() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_install_edge_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    // Uninstall on clean directory without files succeeds
    assert!(Installer::uninstall_from(&temp_dir, false).is_ok());

    // Shell config with malformed JSON
    let shell_dir = temp_dir.join(".config/omarchy");
    let _ = std::fs::create_dir_all(&shell_dir);
    std::fs::write(shell_dir.join("shell.json"), "invalid json content").unwrap();

    let fake_bin = temp_dir.join("bin/soundthis");
    assert!(Installer::install_to(&temp_dir, &fake_bin, false).is_ok());
    assert!(Installer::uninstall_from(&temp_dir, false).is_ok());

    // Shell config with string element "soundthis"
    let string_shell_json = serde_json::json!({
        "bar": {
            "layout": {
                "right": ["soundthis", "clock"]
            }
        }
    });
    std::fs::write(
        shell_dir.join("shell.json"),
        serde_json::to_string(&string_shell_json).unwrap(),
    )
    .unwrap();

    assert!(Installer::install_to(&temp_dir, &fake_bin, false).is_ok());
    assert!(Installer::uninstall_from(&temp_dir, false).is_ok());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_resolve_bin_path_variants() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_bin_resolve_{}", std::process::id()));
    let _ = std::fs::create_dir_all(temp_dir.join(".cargo/bin"));
    let cargo_bin = temp_dir.join(".cargo/bin/soundthis");
    std::fs::write(&cargo_bin, b"binary").unwrap();

    let orig_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", &temp_dir);

    assert_eq!(Installer::resolve_bin_path(), cargo_bin);

    let _ = std::fs::remove_file(&cargo_bin);
    let _ = std::fs::create_dir_all(temp_dir.join(".local/bin"));
    let local_bin = temp_dir.join(".local/bin/soundthis");
    std::fs::write(&local_bin, b"binary").unwrap();

    assert_eq!(Installer::resolve_bin_path(), local_bin);

    let _ = std::fs::remove_file(&local_bin);
    let fallback = Installer::resolve_bin_path();
    assert!(!fallback.as_os_str().is_empty());

    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_install_and_uninstall_public_api() {
    std::env::set_var("SOUNDTHIS_TEST", "1");
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_install_api_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let orig_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", &temp_dir);

    assert!(Installer::install().is_ok());
    assert!(Installer::uninstall().is_ok());

    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}
