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

    let pid_file = crate::ui::get_gui_pid_file();
    let _ = std::fs::write(&pid_file, std::process::id().to_string());
    struct PidGuard(std::path::PathBuf);
    impl Drop for PidGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _guard = PidGuard(pid_file);

    let app = adw::Application::builder()
        .application_id("com.omarchy.soundthis")
        .build();

    let port_rc = Rc::new(port);

    app.connect_activate(move |app| {
        if let Some(existing_window) = app.active_window() {
            existing_window.present();
            return;
        }

        let window = build_ui_window(Some(app), *port_rc);
        window.present();
    });

    let empty_args: [&str; 0] = [];
    let _ = app.run_with_args(&empty_args);

    Ok(())
}

pub fn build_ui_window(app: Option<&adw::Application>, port: u16) -> adw::ApplicationWindow {
    let mut builder = adw::ApplicationWindow::builder()
        .title("SoundThis")
        .default_width(440)
        .default_height(640);
    if let Some(a) = app {
        builder = builder.application(a);
    }
    let window = builder.build();
    window.set_widget_name("main-window");

    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("SoundThis", "Desktop Audio Streaming");
    header.set_title_widget(Some(&title));

    let page = adw::PreferencesPage::new();

    // Group 1: Broadcasting control
    let broadcast_group = adw::PreferencesGroup::builder()
        .title("Audio Stream")
        .build();

    let stream_switch = gtk4::Switch::builder()
        .name("stream-switch")
        .active(true)
        .valign(gtk4::Align::Center)
        .build();

    let broadcast_row = adw::ActionRow::builder()
        .name("broadcast-row")
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
        .name("url-row")
        .title("Stream URL")
        .subtitle(&stream_url)
        .build();

    let copy_btn = gtk4::Button::builder()
        .name("copy-btn")
        .icon_name("edit-copy-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text("Copy stream URL")
        .build();

    let url_for_copy = stream_url.clone();
    copy_btn.connect_clicked(move |_| {
        let _ = crate::ui::copy_to_clipboard(&url_for_copy);
    });

    let open_btn = gtk4::Button::builder()
        .name("open-btn")
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
    let mut picture_weak = None;
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
            picture.set_widget_name("qr-picture");
            picture.set_can_shrink(false);
            picture.set_halign(gtk4::Align::Center);
            picture.set_margin_top(8);
            picture.set_margin_bottom(8);
            picture_weak = Some(picture.downgrade());
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
        .name("listeners-row")
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
        .name("keep-row")
        .title("Keep streaming on close")
        .subtitle("Continue streaming in Omarchy bar when this window closes")
        .build();

    let keep_switch = gtk4::Switch::builder()
        .name("keep-switch")
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

    // Omarchy theme setup
    let css_provider = gtk4::CssProvider::new();
    let current_palette = Rc::new(std::cell::RefCell::new(
        crate::server::theme::ThemePalette::load_current(),
    ));
    apply_theme(&css_provider, &current_palette.borrow());
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // Periodic telemetry update (listeners, streaming state, theme)
    let switch_weak = stream_switch.downgrade();
    let listeners_row_weak = listeners_row.downgrade();
    let listeners_group_weak = listeners_group.downgrade();
    let is_updating_timer = is_updating.clone();
    let css_provider_timer = css_provider.clone();
    let palette_timer = current_palette.clone();
    let stream_url_clone = stream_url.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(1500), move || {
        let Some(sw) = switch_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        // Live theme synchronization
        let new_palette = crate::server::theme::ThemePalette::load_current();
        if new_palette != *palette_timer.borrow() {
            *palette_timer.borrow_mut() = new_palette.clone();
            apply_theme(&css_provider_timer, &new_palette);

            if let Some(ref p_weak) = picture_weak {
                if let Some(pic) = p_weak.upgrade() {
                    if let Ok(svg_data) = new_palette.generate_qr_svg(&stream_url_clone) {
                        let bytes = glib::Bytes::from(svg_data.as_bytes());
                        let stream = gio::MemoryInputStream::from_bytes(&bytes);
                        if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_stream_at_scale(
                            &stream,
                            200,
                            200,
                            true,
                            None::<&gio::Cancellable>,
                        ) {
                            pic.set_pixbuf(Some(&pixbuf));
                        }
                    }
                }
            }
        }

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

    window
}

pub fn find_widget_by_name<W: glib::object::IsA<gtk4::Widget>>(
    root: &gtk4::Widget,
    name: &str,
) -> Option<W> {
    if root.widget_name() == name {
        return root.clone().downcast::<W>().ok();
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_widget_by_name::<W>(&c, name) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

pub fn build_theme_css(palette: &crate::server::theme::ThemePalette) -> String {
    let accent_fg = if palette.mode == "light" {
        "#ffffff"
    } else {
        &palette.darker_background
    };
    format!(
        r#"
@define-color window_bg_color {bg};
@define-color window_fg_color {fg};
@define-color view_bg_color {bg};
@define-color view_fg_color {fg};
@define-color headerbar_bg_color {dark_bg};
@define-color headerbar_fg_color {fg};
@define-color card_bg_color {card_bg};
@define-color card_fg_color {fg};
@define-color accent_color {accent};
@define-color accent_bg_color {accent};
@define-color accent_fg_color {accent_fg};

window, window.background, .background, preferencespage, scrolledwindow, viewport {{
    background-color: {bg};
    color: {fg};
}}

headerbar {{
    background-color: {dark_bg};
    color: {fg};
}}

.boxed-list, row {{
    background-color: {card_bg};
    color: {fg};
}}
"#,
        bg = palette.background,
        fg = palette.bright_foreground,
        dark_bg = palette.dark_background,
        card_bg = palette.lighter_background,
        accent = palette.accent,
        accent_fg = accent_fg,
    )
}

pub fn apply_theme(provider: &gtk4::CssProvider, palette: &crate::server::theme::ThemePalette) {
    let style_manager = adw::StyleManager::default();
    if palette.mode == "light" {
        style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
    } else {
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
    }
    let css = build_theme_css(palette);
    provider.load_from_data(&css);
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
        assert!(css.contains("@define-color window_bg_color"));
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
    fn test_ui_widget_tree_and_simulation() {
        if gtk4::init().is_err() || adw::init().is_err() {
            return;
        }

        // Verify CSS theme loading on the initialized GTK display
        let provider = gtk4::CssProvider::new();
        apply_theme(&provider, &crate::server::theme::ThemePalette::default());

        let window = build_ui_window(None, 50123);
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

        // 2. Simulate user click on copy button
        copy_btn.emit_clicked();

        // 3. Simulate user toggling keep-switch
        let orig_keep = keep_sw.is_active();
        keep_sw.set_active(!orig_keep);

        // Process pending GTK/GLib main context events
        let ctx = glib::MainContext::default();
        while ctx.iteration(false) {}

        assert_eq!(keep_sw.is_active(), !orig_keep);

        // Verify config updated
        let cfg = crate::ui::read_gui_config();
        assert_eq!(cfg.keep_streaming, !orig_keep);

        // Restore original setting
        keep_sw.set_active(orig_keep);
        while ctx.iteration(false) {}
        let cfg_restored = crate::ui::read_gui_config();
        assert_eq!(cfg_restored.keep_streaming, orig_keep);
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
