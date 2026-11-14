use adw::prelude::*;
use gtk4::gdk_pixbuf;
use gtk4::gio;
use gtk4::glib;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::info;

pub fn run_gtk_app(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting SoundThis native GTK4/Adwaita application on port {port}");

    let app = adw::Application::builder()
        .application_id("com.omarchy.soundthis")
        .build();

    let port_rc = Rc::new(port);

    app.connect_activate(move |app| {
        if let Some(existing_window) = app.active_window() {
            existing_window.present();
            return;
        }

        build_ui(app, *port_rc);
    });

    let empty_args: [&str; 0] = [];
    let _ = app.run_with_args(&empty_args);

    Ok(())
}

fn build_ui(app: &adw::Application, port: u16) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("SoundThis")
        .default_width(440)
        .default_height(640)
        .build();

    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("SoundThis", "Desktop Audio Streaming");
    header.set_title_widget(Some(&title));

    let page = adw::PreferencesPage::new();

    // Group 1: Broadcasting control
    let broadcast_group = adw::PreferencesGroup::builder()
        .title("Audio Stream")
        .build();

    let stream_switch = gtk4::Switch::builder()
        .active(true)
        .valign(gtk4::Align::Center)
        .build();

    let broadcast_row = adw::ActionRow::builder()
        .title("Broadcasting")
        .subtitle("Stream audio from this computer")
        .build();
    broadcast_row.add_suffix(&stream_switch);
    broadcast_group.add(&broadcast_row);
    page.add(&broadcast_group);

    // Group 2: Connection details
    let lan_ip = crate::network::ip::get_lan_ip();
    let stream_url = format!("http://{lan_ip}:{port}");

    let connect_group = adw::PreferencesGroup::builder()
        .title("Listen on other devices")
        .description("Open the URL or scan the QR code from your phone or browser")
        .build();

    let url_row = adw::ActionRow::builder()
        .title("Stream URL")
        .subtitle(&stream_url)
        .build();

    let copy_btn = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text("Copy stream URL")
        .build();

    let url_for_copy = stream_url.clone();
    copy_btn.connect_clicked(move |_| {
        let _ = crate::ui::copy_to_clipboard(&url_for_copy);
    });

    let open_btn = gtk4::Button::builder()
        .icon_name("external-link-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text("Open in browser")
        .build();

    let url_for_open = stream_url.clone();
    open_btn.connect_clicked(move |_| {
        let _ = std::process::Command::new("xdg-open")
            .arg(&url_for_open)
            .status();
    });

    url_row.add_suffix(&copy_btn);
    url_row.add_suffix(&open_btn);
    connect_group.add(&url_row);

    // QR code display
    let palette = crate::server::theme::ThemePalette::load_current();
    if let Ok(svg_data) = palette.generate_qr_svg(&stream_url) {
        let qr_row = adw::ActionRow::builder().title("Scan QR Code").build();
        let bytes = glib::Bytes::from(svg_data.as_bytes());
        let stream = gio::MemoryInputStream::from_bytes(&bytes);
        if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_stream_at_scale(
            &stream,
            200,
            200,
            true,
            None::<&gio::Cancellable>,
        ) {
            let picture = gtk4::Picture::for_pixbuf(&pixbuf);
            picture.set_can_shrink(false);
            picture.set_halign(gtk4::Align::Center);
            picture.set_margin_top(8);
            picture.set_margin_bottom(8);
            connect_group.add(&picture);
        } else {
            connect_group.add(&qr_row);
        }
    }

    page.add(&connect_group);

    // Group 3: Connected Listeners
    let listeners_group = adw::PreferencesGroup::builder()
        .title("Connected Listeners")
        .description("No devices currently connected")
        .build();
    let listeners_row = adw::ActionRow::builder()
        .title("Active Listeners")
        .subtitle("0 devices listening")
        .build();
    listeners_group.add(&listeners_row);
    page.add(&listeners_group);

    // Group 4: Preferences
    let pref_group = adw::PreferencesGroup::builder()
        .title("Preferences")
        .build();

    let keep_cfg = crate::ui::read_gui_config();
    let keep_row = adw::ActionRow::builder()
        .title("Keep streaming on close")
        .subtitle("Continue streaming in Omarchy bar when this window closes")
        .build();

    let keep_switch = gtk4::Switch::builder()
        .active(keep_cfg.keep_streaming)
        .valign(gtk4::Align::Center)
        .build();

    keep_switch.connect_active_notify(|sw| {
        let mut cfg = crate::ui::read_gui_config();
        cfg.keep_streaming = sw.is_active();
        let _ = crate::ui::save_gui_config(&cfg);
    });

    keep_row.add_suffix(&keep_switch);
    pref_group.add(&keep_row);
    page.add(&pref_group);

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content_box.append(&header);
    content_box.append(&page);

    window.set_content(Some(&content_box));

    // Handle toggle switch events
    let is_updating = Arc::new(AtomicBool::new(false));
    let is_updating_clone = is_updating.clone();
    stream_switch.connect_active_notify(move |sw| {
        if is_updating_clone.load(Ordering::SeqCst) {
            return;
        }
        let active = sw.is_active();
        let endpoint = if active { "enable" } else { "disable" };
        let url = format!("http://127.0.0.1:{port}/api/{endpoint}");
        std::thread::spawn(move || {
            let _ = ureq_post(&url);
        });
    });

    // Periodic telemetry update (listeners, streaming state)
    let switch_weak = stream_switch.downgrade();
    let listeners_row_weak = listeners_row.downgrade();
    let listeners_group_weak = listeners_group.downgrade();
    let is_updating_timer = is_updating.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(1500), move || {
        let Some(sw) = switch_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        if let Ok(status) = fetch_status(port) {
            is_updating_timer.store(true, Ordering::SeqCst);
            if sw.is_active() != status.enabled {
                sw.set_active(status.enabled);
            }
            is_updating_timer.store(false, Ordering::SeqCst);

            if let (Some(l_row), Some(l_grp)) =
                (listeners_row_weak.upgrade(), listeners_group_weak.upgrade())
            {
                let count = status.clients.len();
                if count == 0 {
                    l_grp.set_description(Some("No devices currently connected"));
                    l_row.set_subtitle("0 devices listening");
                } else {
                    let desc = format!(
                        "{count} device{} connected",
                        if count == 1 { "" } else { "s" }
                    );
                    l_grp.set_description(Some(&desc));
                    let names: Vec<String> = status
                        .clients
                        .iter()
                        .map(|c| format!("{} ({})", c.name, c.ip))
                        .collect();
                    l_row.set_subtitle(&names.join(", "));
                }
            }
        }

        glib::ControlFlow::Continue
    });

    // Handle window close
    window.connect_close_request(move |_| {
        let keep = crate::ui::read_gui_config().keep_streaming;
        if !keep {
            let url = format!("http://127.0.0.1:{port}/api/disable");
            let _ = ureq_post(&url);
        }
        glib::Propagation::Proceed
    });

    // Apply Omarchy theme styling
    apply_css_theme();

    window.present();
}

pub fn build_theme_css(palette: &crate::server::theme::ThemePalette) -> String {
    format!(
        "window {{ background-color: {}; color: {}; }}",
        palette.background, palette.bright_foreground
    )
}

fn apply_css_theme() {
    let palette = crate::server::theme::ThemePalette::load_current();
    let css = build_theme_css(&palette);
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&css);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClientSummary {
    pub name: String,
    pub ip: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SimpleStatus {
    pub enabled: bool,
    pub clients: Vec<ClientSummary>,
}

pub fn parse_status_json(val: &serde_json::Value) -> SimpleStatus {
    let enabled = val["enabled"].as_bool().unwrap_or(false);
    let mut clients = Vec::new();
    if let Some(list) = val["clients"].as_array() {
        for c in list {
            let name = c["name"].as_str().unwrap_or("Device").to_string();
            let ip = c["ip"].as_str().unwrap_or("unknown").to_string();
            clients.push(ClientSummary { name, ip });
        }
    }
    SimpleStatus { enabled, clients }
}

fn fetch_status(port: u16) -> Result<SimpleStatus, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()?;
    let res: serde_json::Value = client
        .get(format!("http://127.0.0.1:{port}/api/status"))
        .send()?
        .json()?;
    Ok(parse_status_json(&res))
}

fn ureq_post(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()?;
    client.post(url).send()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_theme_css() {
        let palette = crate::server::theme::ThemePalette::default();
        let css = build_theme_css(&palette);
        assert!(css.contains("window {"));
        assert!(css.contains(&palette.background));
        assert!(css.contains(&palette.bright_foreground));
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
    fn test_apply_css_theme_does_not_panic() {
        if gtk4::init().is_ok() {
            apply_css_theme();
        }
    }

    #[test]
    fn test_fetch_status_offline() {
        let res = fetch_status(59998);
        assert!(res.is_err());
    }

    #[test]
    fn test_ureq_post_offline() {
        let res = ureq_post("http://127.0.0.1:59998/api/disable");
        assert!(res.is_err());
    }
}
