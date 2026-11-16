use adw::prelude::*;
use gtk4::glib;
use soundthis::server::theme::ThemePalette;
use soundthis::ui::gtk_app::*;
use soundthis::ui::*;
use std::path::PathBuf;

#[test]
fn test_build_theme_css() {
    let palette = ThemePalette::default();
    let css = build_theme_css(&palette);
    assert!(css.contains("@define-color window_bg_color"));
    assert!(css.contains(&palette.background));
    assert!(css.contains(&palette.bright_foreground));
}

#[test]
fn test_build_theme_css_light_mode() {
    let palette = ThemePalette {
        mode: "light".to_string(),
        ..Default::default()
    };
    let css = build_theme_css(&palette);
    assert!(css.contains("@define-color accent_fg_color #ffffff;"));
}

#[test]
fn test_parse_status_json() {
    let json = serde_json::json!({
        "enabled": true,
        "clients": [
            {
                "name": "My Phone",
                "ip": "192.168.1.100"
            }
        ]
    });

    let status = parse_status_json(&json);
    assert!(status.enabled);
    assert_eq!(status.clients.len(), 1);
    assert_eq!(status.clients[0].name, "My Phone");
    assert_eq!(status.clients[0].ip, "192.168.1.100");
}

#[test]
fn test_parse_status_json_empty() {
    let json = serde_json::json!({});
    let status = parse_status_json(&json);
    assert!(!status.enabled);
    assert!(status.clients.is_empty());
}

#[test]
fn test_fetch_status_offline() {
    let res = fetch_status(59998);
    assert!(res.is_err());
}

#[test]
fn test_ureq_post_offline() {
    let res = ureq_post("http://127.0.0.1:59998/api/v1/disable");
    assert!(res.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_ui_widget_tree_and_simulation() {
    if gtk4::init().is_err() || adw::init().is_err() {
        return;
    }

    use axum::routing::{get, post};
    let app = axum::Router::new()
        .route(
            "/api/v1/status",
            get(|| async {
                axum::Json(serde_json::json!({
                    "enabled": true,
                    "clients": [{"name": "Phone", "ip": "192.168.1.200", "listening": true}]
                }))
            }),
        )
        .route(
            "/api/v1/disable",
            post(|| async { axum::Json(serde_json::json!({"enabled": false})) }),
        )
        .route(
            "/api/v1/enable",
            post(|| async { axum::Json(serde_json::json!({"enabled": true})) }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let srv_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Test fetch_status and ureq_post online
    let status_online = fetch_status(port);
    assert!(status_online.is_ok());
    assert!(status_online.unwrap().enabled);
    assert!(ureq_post(&format!("http://127.0.0.1:{port}/api/v1/enable")).is_ok());

    // Verify CSS theme loading on the initialized GTK display (both dark and light)
    let provider = gtk4::CssProvider::new();
    apply_theme(&provider, &ThemePalette::default());
    let light_palette = ThemePalette {
        mode: "light".to_string(),
        ..Default::default()
    };
    apply_theme(&provider, &light_palette);

    let window = build_ui_window(None, port);
    let root = window.upcast_ref::<gtk4::Widget>();

    // 1. Find widgets by test-id (widget_name)
    let stream_sw: gtk4::Switch =
        find_widget_by_name(root, "stream-switch").expect("stream-switch widget must exist");
    let keep_sw: gtk4::Switch =
        find_widget_by_name(root, "keep-switch").expect("keep-switch widget must exist");
    let copy_btn: gtk4::Button =
        find_widget_by_name(root, "copy-btn").expect("copy-btn widget must exist");
    let open_btn: gtk4::Button =
        find_widget_by_name(root, "open-btn").expect("open-btn widget must exist");
    let listeners_row: adw::ActionRow =
        find_widget_by_name(root, "listeners-row").expect("listeners-row widget must exist");

    assert!(stream_sw.is_active());
    assert_eq!(listeners_row.title(), "Active Listeners");
    assert_eq!(open_btn.tooltip_text().as_deref(), Some("Open in browser"));
    assert_eq!(copy_btn.tooltip_text().as_deref(), Some("Copy stream URL"));

    // 2. Simulate user click on copy button and open button
    copy_btn.emit_clicked();
    open_btn.emit_clicked();

    // 3. Simulate user toggling keep-switch
    let orig_keep = keep_sw.is_active();
    keep_sw.set_active(!orig_keep);

    // Process pending GTK/GLib main context events
    let ctx = glib::MainContext::default();
    while ctx.iteration(false) {}

    assert_eq!(keep_sw.is_active(), !orig_keep);

    // Verify config updated
    let cfg = read_gui_config();
    assert_eq!(cfg.keep_streaming, !orig_keep);

    // Restore original setting
    keep_sw.set_active(orig_keep);
    while ctx.iteration(false) {}
    let cfg_restored = read_gui_config();
    assert_eq!(cfg_restored.keep_streaming, orig_keep);

    // 4. Simulate toggling stream switch
    stream_sw.set_active(false);
    while ctx.iteration(false) {}
    assert!(!stream_sw.is_active());

    // 5. Test window close
    window.close();
    while ctx.iteration(false) {}

    srv_handle.abort();
}

#[test]
fn test_gui_config_serialization() {
    let tmp = std::env::temp_dir().join(format!("soundthis_gui_test_{}.json", std::process::id()));
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
fn test_stop_gui_process_missing_and_invalid() {
    let tmp = std::env::temp_dir().join(format!("test_stop_pid_{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    assert!(!stop_gui_process_from(&tmp));

    std::fs::write(&tmp, "invalid").unwrap();
    assert!(!stop_gui_process_from(&tmp));
    assert!(!tmp.exists());

    // When PID matches current process PID, stop_gui_process_from should return false
    std::fs::write(&tmp, std::process::id().to_string()).unwrap();
    assert!(!stop_gui_process_from(&tmp));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_is_gui_running_unmatched_process() {
    let tmp = std::env::temp_dir().join(format!("test_unmatched_pid_{}.pid", std::process::id()));
    // PID 1 is init/systemd whose cmdline does not contain soundthis/cargo/qml
    std::fs::write(&tmp, "1").unwrap();
    assert!(!is_gui_running_from(&tmp));
    assert!(!tmp.exists());
}

#[test]
fn test_is_gui_running_dead_pid() {
    let tmp = std::env::temp_dir().join(format!("test_dead_pid_{}.pid", std::process::id()));
    std::fs::write(&tmp, "99999999").unwrap();
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

#[test]
fn test_gui_config_json_serialization() {
    let tmp = std::env::temp_dir().join(format!("soundthis_gui_json_{}.json", std::process::id()));
    let cfg = GuiConfig {
        keep_streaming: true,
    };
    save_gui_config_to(&tmp, &cfg).unwrap();
    let loaded = read_gui_config_from(&tmp);
    assert_eq!(loaded, cfg);
    let _ = std::fs::remove_file(tmp);
}

#[test]
fn test_gui_config_toml_serialization() {
    let tmp = std::env::temp_dir().join(format!("soundthis_gui_test_{}.toml", std::process::id()));
    let initial_toml = r#"
[server]
port = 9697

[desktop]
keep_streaming = false
"#;
    std::fs::write(&tmp, initial_toml).unwrap();
    let mut cfg = read_gui_config_from(&tmp);
    assert!(!cfg.keep_streaming);

    cfg.keep_streaming = true;
    save_gui_config_to(&tmp, &cfg).unwrap();

    let reloaded = read_gui_config_from(&tmp);
    assert!(reloaded.keep_streaming);
    let _ = std::fs::remove_file(tmp);
}

#[test]
fn test_stop_gui_process_active_process() {
    let mut child = std::process::Command::new("sleep")
        .arg("10")
        .spawn()
        .expect("sleep should spawn");
    let pid = child.id();
    let tmp = std::env::temp_dir().join(format!("test_kill_pid_{}.pid", std::process::id()));
    std::fs::write(&tmp, pid.to_string()).unwrap();

    assert!(stop_gui_process_from(&tmp));
    assert!(!tmp.exists());
    let _ = child.kill();
    let _ = child.wait();
}

#[tokio::test]
async fn test_run_desktop_workflow_when_gui_running() {
    let pid_file = get_gui_pid_file();
    let _ = std::fs::write(&pid_file, std::process::id().to_string());
    let cfg = soundthis::config::ServerConfig::default();
    let res = run_desktop_workflow(&cfg).await;
    assert!(res.is_ok());
    let _ = std::fs::remove_file(pid_file);
}

#[test]
fn test_gui_helpers_and_error_paths() {
    // Non-existent PID stop
    let p = get_gui_pid_file();
    let _ = std::fs::remove_file(&p);
    assert!(!stop_gui_process());

    // Save GUI config to invalid path
    let invalid_path = std::path::Path::new("/proc/soundthis_nonexistent/config.json");
    let cfg = GuiConfig {
        keep_streaming: true,
    };
    assert!(save_gui_config_to(invalid_path, &cfg).is_err());

    // Test get_gui_config_path and read_gui_config
    let path = get_gui_config_path();
    assert!(!path.as_os_str().is_empty());
    let _ = read_gui_config();
    let _ = save_gui_config(&cfg);
}
