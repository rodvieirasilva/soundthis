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

    // Event-driven Omarchy theme monitoring via gio::FileMonitor (inotify)
    let theme_monitors =
        setup_theme_monitor(&css_provider, &current_palette, picture_weak, &stream_url);

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
        let _ = &theme_monitors;
        let keep = crate::ui::read_gui_config().keep_streaming;
        if !keep {
            let url = format!("http://127.0.0.1:{port}/api/disable");
            let _ = ureq_post(&url);
        }
        glib::Propagation::Proceed
    });

    window
}

pub fn setup_theme_monitor(
    css_provider: &gtk4::CssProvider,
    current_palette: &Rc<std::cell::RefCell<crate::server::theme::ThemePalette>>,
    picture_weak: Option<gtk4::glib::WeakRef<gtk4::Picture>>,
    stream_url: &str,
) -> Vec<gio::FileMonitor> {
    let mut monitors = Vec::new();
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));

    let theme_dir = home.join(".local/state/omarchy/current/theme");
    let current_dir = home.join(".local/state/omarchy/current");

    let css_provider = css_provider.clone();
    let current_palette = current_palette.clone();
    let stream_url = stream_url.to_string();

    let reload_theme = Rc::new(move || {
        let new_palette = crate::server::theme::ThemePalette::load_current();
        if new_palette != *current_palette.borrow() {
            *current_palette.borrow_mut() = new_palette.clone();
            apply_theme(&css_provider, &new_palette);

            if let Some(ref p_weak) = picture_weak {
                if let Some(pic) = p_weak.upgrade() {
                    if let Ok(svg_data) = new_palette.generate_qr_svg(&stream_url) {
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
    });

    for path in [&current_dir, &theme_dir] {
        if path.exists() {
            let file = gio::File::for_path(path);
            if let Ok(monitor) =
                file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            {
                let cb = reload_theme.clone();
                let debounce_id = Rc::new(std::cell::RefCell::new(None::<glib::SourceId>));
                let debounce_id_clone = debounce_id.clone();

                monitor.connect_changed(move |_, _, _, _| {
                    if let Some(id) = debounce_id_clone.borrow_mut().take() {
                        id.remove();
                    }
                    let cb_inner = cb.clone();
                    let id = glib::timeout_add_local_once(
                        std::time::Duration::from_millis(80),
                        move || {
                            cb_inner();
                        },
                    );
                    *debounce_id_clone.borrow_mut() = Some(id);
                });
                monitors.push(monitor);
            }
        }
    }

    monitors
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

pub fn fetch_status(port: u16) -> Result<SimpleStatus, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()?;
    let res: serde_json::Value = client
        .get(format!("http://127.0.0.1:{port}/api/status"))
        .send()?
        .json()?;
    Ok(parse_status_json(&res))
}

pub fn ureq_post(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()?;
    client.post(url).send()?;
    Ok(())
}
