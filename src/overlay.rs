#[cfg(feature = "gui")]
use gtk4::{self, gdk, glib};
#[cfg(feature = "gui")]
use gtk4_layer_shell::LayerShell;
#[cfg(feature = "gui")]
use libadwaita as adw;
#[cfg(feature = "gui")]
use libadwaita::prelude::*;

#[cfg(feature = "gui")]
use std::sync::Arc;

#[cfg(feature = "gui")]
use crate::cli::Position;
#[cfg(feature = "gui")]
use crate::provider::AnyProvider;
#[cfg(feature = "gui")]
use crate::screenshot;

#[cfg(feature = "gui")]
const PID_FILE: &str = "/tmp/satori-overlay.pid";

/// Kill any previous satori overlay process and write our PID.
#[cfg(feature = "gui")]
fn manage_pid_file() {
    // Kill previous instance if PID file exists
    if let Ok(content) = std::fs::read_to_string(PID_FILE)
        && let Ok(pid) = content.trim().parse::<u32>()
    {
        // SAFETY: sending SIGTERM to a process by PID (best effort)
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Write our PID
    let _ = std::fs::write(PID_FILE, std::process::id().to_string());
}

/// Remove PID file on exit.
#[cfg(feature = "gui")]
fn cleanup_pid_file() {
    let _ = std::fs::remove_file(PID_FILE);
}

/// Configuration for the overlay window, assembled from CLI args + config.
#[cfg(feature = "gui")]
pub struct OverlayConfig {
    pub target_lang: String,
    pub translation_style: String,
    pub ui_opacity: f64,
    pub position: Position,
    pub font: Option<String>,
    pub color: Option<String>,
    pub background_color: Option<String>,
    pub background_opacity: Option<f64>,
    /// Pre-loaded image data (from file). If None, takes a screenshot.
    pub image_data: Option<Vec<u8>>,
    /// Persisted margins from previous drag: [top, bottom, left, right]
    pub last_margins: Option<[i32; 4]>,
}

#[cfg(feature = "gui")]
struct DisplayEntry {
    source_text: String,
    translated_text: String,
}

/// Run the GTK4 overlay application.
#[cfg(feature = "gui")]
pub fn run(provider: AnyProvider, config: OverlayConfig) {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    manage_pid_file();

    let app = adw::Application::builder()
        .application_id("com.github.satori")
        .build();

    let save_pos = config.position;
    let provider = Arc::new(provider);
    // Extract image_data before Arc to avoid keeping screenshot bytes in memory permanently
    let initial_image: Option<Vec<u8>> = config.image_data;
    let config = Arc::new(OverlayConfig {
        target_lang: config.target_lang,
        translation_style: config.translation_style,
        ui_opacity: config.ui_opacity,
        position: config.position,
        font: config.font,
        color: config.color,
        background_color: config.background_color,
        background_opacity: config.background_opacity,
        image_data: None,
        last_margins: config.last_margins,
    });
    // Shared slot to capture final margins when the window closes
    let final_margins: Arc<std::sync::Mutex<Option<[i32; 4]>>> =
        Arc::new(std::sync::Mutex::new(None));
    let final_margins_outer = Arc::clone(&final_margins);

    app.connect_activate(move |app| {
        let image_data = if let Some(ref data) = initial_image {
            data.clone()
        } else {
            match screenshot::take_screenshot() {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Screenshot failed: {e}");
                    std::process::exit(1);
                }
            }
        };

        let window = build_window(app, &config);

        // Load only recent history (lazy — older entries are on disk)
        let history = crate::history::list_entries(Some(20)).unwrap_or_default();
        let initial: Vec<DisplayEntry> = history
            .into_iter()
            .map(|h| DisplayEntry {
                source_text: h.source_text,
                translated_text: h.translated_text,
            })
            .collect();
        let has_history = !initial.is_empty();
        let entries: Rc<RefCell<Vec<DisplayEntry>>> = Rc::new(RefCell::new(initial));
        let current_idx: Rc<Cell<usize>> = Rc::new(Cell::new(entries.borrow().len()));
        let is_translating: Rc<Cell<bool>> = Rc::new(Cell::new(true));

        // Header bar with navigation and action buttons
        let header = adw::HeaderBar::new();
        header.set_decoration_layout(Some(":close"));

        let prev_btn = gtk4::Button::from_icon_name("go-previous-symbolic");
        prev_btn.set_tooltip_text(Some("Previous translation"));
        prev_btn.set_sensitive(has_history);

        let next_btn = gtk4::Button::from_icon_name("go-next-symbolic");
        next_btn.set_tooltip_text(Some("Next translation"));
        next_btn.set_sensitive(false);

        let screenshot_btn = gtk4::Button::from_icon_name("camera-photo-symbolic");
        screenshot_btn.set_tooltip_text(Some("New screenshot"));
        screenshot_btn.set_sensitive(false);

        let copy_btn = gtk4::Button::from_icon_name("edit-copy-symbolic");
        copy_btn.set_tooltip_text(Some("Copy translation (Ctrl+C)"));
        copy_btn.set_visible(false);

        header.pack_start(&prev_btn);
        header.pack_start(&next_btn);
        header.pack_start(&copy_btn);
        header.pack_end(&screenshot_btn);

        let content_area = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        body.set_margin_top(12);
        body.set_margin_bottom(24);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.set_halign(gtk4::Align::Center);
        body.set_valign(gtk4::Align::Start);
        body.set_vexpand(true);

        show_spinner_in(&body);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scrolled.set_vexpand(true);
        scrolled.set_propagate_natural_height(true);
        // Cap at ~80% of monitor height so content grows naturally but still scrolls
        let max_h = gdk::Display::default()
            .and_then(|d| d.monitors().item(0))
            .and_then(|m| m.downcast::<gdk::Monitor>().ok())
            .map(|m| (m.geometry().height() as f64 * 0.75) as i32)
            .unwrap_or(800);
        scrolled.set_max_content_height(max_h);
        scrolled.set_child(Some(&body));

        content_area.append(&header);
        content_area.append(&scrolled);
        window.set_content(Some(&content_area));
        window.present();

        setup_key_handler(&window);
        setup_drag_handler(&window, save_pos);

        // Capture final margins on window close for persistence
        {
            let fm = Arc::clone(&final_margins);
            window.connect_close_request(move |w| {
                use gtk4_layer_shell::Edge;
                use gtk4_layer_shell::LayerShell;
                let margins = [
                    w.margin(Edge::Top),
                    w.margin(Edge::Bottom),
                    w.margin(Edge::Left),
                    w.margin(Edge::Right),
                ];
                if let Ok(mut slot) = fm.lock() {
                    *slot = Some(margins);
                }
                glib::Propagation::Proceed
            });
        }

        // Wire copy button
        {
            let w = window.downgrade();
            copy_btn.connect_clicked(move |_| {
                if let Some(win) = w.upgrade() {
                    copy_translation_to_clipboard(&win);
                }
            });
        }

        // Wire prev button
        {
            let ent = Rc::clone(&entries);
            let idx = Rc::clone(&current_idx);
            let b = body.clone();
            let p = prev_btn.clone();
            let n = next_btn.clone();
            let c = copy_btn.clone();
            let w = window.downgrade();
            prev_btn.connect_clicked(move |_| {
                let cur = idx.get();
                if cur > 0 {
                    idx.set(cur - 1);
                    if let Some(win) = w.upgrade() {
                        let e = ent.borrow();
                        update_display(&b, &e, cur - 1, &p, &n, &c, &win);
                    }
                }
            });
        }

        // Wire next button
        {
            let ent = Rc::clone(&entries);
            let idx = Rc::clone(&current_idx);
            let b = body.clone();
            let p = prev_btn.clone();
            let n = next_btn.clone();
            let c = copy_btn.clone();
            let w = window.downgrade();
            next_btn.connect_clicked(move |_| {
                let cur = idx.get();
                let len = ent.borrow().len();
                if cur + 1 < len {
                    idx.set(cur + 1);
                    if let Some(win) = w.upgrade() {
                        let e = ent.borrow();
                        update_display(&b, &e, cur + 1, &p, &n, &c, &win);
                    }
                }
            });
        }

        // Wire screenshot button
        {
            let ent = Rc::clone(&entries);
            let idx = Rc::clone(&current_idx);
            let busy = Rc::clone(&is_translating);
            let b = body.clone();
            let p = prev_btn.clone();
            let n = next_btn.clone();
            let c = copy_btn.clone();
            let s = screenshot_btn.clone();
            let w = window.downgrade();
            let prov = Arc::clone(&provider);
            let lang = config.target_lang.clone();
            let style = config.translation_style.clone();

            screenshot_btn.connect_clicked(move |_| {
                if busy.get() {
                    return;
                }
                let Some(win) = w.upgrade() else { return };
                busy.set(true);
                s.set_sensitive(false);
                win.set_visible(false);

                let prov = Arc::clone(&prov);
                let lang = lang.clone();
                let style = style.clone();
                let ent = Rc::clone(&ent);
                let idx = Rc::clone(&idx);
                let busy = Rc::clone(&busy);
                let b = b.clone();
                let p = p.clone();
                let n = n.clone();
                let c = c.clone();
                let s = s.clone();
                let ww = win.downgrade();

                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    let shot = screenshot::take_screenshot();
                    let Some(win) = ww.upgrade() else { return };
                    win.set_visible(true);

                    match shot {
                        Err(e) => {
                            clear_body(&b);
                            show_error(&b, &format!("Screenshot failed: {e}"));
                            busy.set(false);
                            s.set_sensitive(true);
                        }
                        Ok(img) => {
                            clear_body(&b);
                            show_spinner_in(&b);

                            let lang_t = lang.clone();
                            let style_t = style.clone();
                            let (tx, rx) = std::sync::mpsc::channel();
                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                                let result = rt.block_on(prov.translate(&img, &lang_t, &style_t));
                                let _ = tx.send(result);
                            });

                            let w2 = win.downgrade();
                            glib::timeout_add_local(
                                std::time::Duration::from_millis(100),
                                move || match rx.try_recv() {
                                    Ok(result) => {
                                        clear_body(&b);
                                        match result {
                                            Ok(tr) => {
                                                let _ = crate::history::record(
                                                    &tr.source_text,
                                                    &tr.translated_text,
                                                    &lang,
                                                    &style,
                                                );
                                                ent.borrow_mut().push(DisplayEntry {
                                                    source_text: tr.source_text,
                                                    translated_text: tr.translated_text,
                                                });
                                                let new_idx = ent.borrow().len() - 1;
                                                idx.set(new_idx);
                                                if let Some(win) = w2.upgrade() {
                                                    let e = ent.borrow();
                                                    update_display(
                                                        &b, &e, new_idx, &p, &n, &c, &win,
                                                    );
                                                }
                                            }
                                            Err(e) => show_error(&b, &e.to_string()),
                                        }
                                        busy.set(false);
                                        s.set_sensitive(true);
                                        glib::ControlFlow::Break
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                                        glib::ControlFlow::Continue
                                    }
                                    Err(_) => {
                                        clear_body(&b);
                                        show_error(&b, "Translation thread disconnected");
                                        busy.set(false);
                                        s.set_sensitive(true);
                                        glib::ControlFlow::Break
                                    }
                                },
                            );
                        }
                    }
                });
            });
        }

        // Start initial translation
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let prov = Arc::clone(&provider);
            let lang = config.target_lang.clone();
            let style = config.translation_style.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                let result = rt.block_on(prov.translate(&image_data, &lang, &style));
                let _ = tx.send(result);
            });
        }

        {
            let ent = Rc::clone(&entries);
            let idx = Rc::clone(&current_idx);
            let busy = Rc::clone(&is_translating);
            let b = body.clone();
            let p = prev_btn.clone();
            let n = next_btn.clone();
            let c = copy_btn.clone();
            let s = screenshot_btn.clone();
            let w = window.downgrade();
            let lang = config.target_lang.clone();
            let style = config.translation_style.clone();

            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(result) => {
                        clear_body(&b);
                        match result {
                            Ok(tr) => {
                                let _ = crate::history::record(
                                    &tr.source_text,
                                    &tr.translated_text,
                                    &lang,
                                    &style,
                                );
                                ent.borrow_mut().push(DisplayEntry {
                                    source_text: tr.source_text,
                                    translated_text: tr.translated_text,
                                });
                                let new_idx = ent.borrow().len() - 1;
                                idx.set(new_idx);
                                if let Some(win) = w.upgrade() {
                                    let e = ent.borrow();
                                    update_display(&b, &e, new_idx, &p, &n, &c, &win);
                                }
                            }
                            Err(e) => show_error(&b, &e.to_string()),
                        }
                        busy.set(false);
                        s.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        clear_body(&b);
                        show_error(&b, "Translation thread disconnected");
                        busy.set(false);
                        s.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    });

    app.run_with_args::<String>(&[]);
    cleanup_pid_file();
    let saved = final_margins_outer.lock().ok().and_then(|m| *m);
    let _ = crate::config::save_last_pos(save_pos, saved);
}

#[cfg(feature = "gui")]
fn build_window(app: &adw::Application, config: &OverlayConfig) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Satori")
        .default_width(480)
        .build();

    window.set_opacity(config.ui_opacity);

    // Layer shell setup
    window.init_layer_shell();
    window.set_layer(gtk4_layer_shell::Layer::Overlay);
    window.set_namespace("satori-overlay");
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);

    // Position anchoring + restore saved margins
    apply_position(&window, config.position, config.last_margins);

    // Custom CSS for font, colors
    apply_custom_css(&window, config);

    window
}

#[cfg(feature = "gui")]
fn apply_position(window: &adw::ApplicationWindow, pos: Position, saved_margins: Option<[i32; 4]>) {
    use gtk4_layer_shell::Edge;
    let default_margin = 20;
    match pos {
        Position::Center => {
            // No anchors = centered
        }
        Position::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
            if let Some([t, _, l, _]) = saved_margins {
                window.set_margin(Edge::Top, t);
                window.set_margin(Edge::Left, l);
            } else {
                window.set_margin(Edge::Top, default_margin);
                window.set_margin(Edge::Left, default_margin);
            }
        }
        Position::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
            if let Some([t, _, _, r]) = saved_margins {
                window.set_margin(Edge::Top, t);
                window.set_margin(Edge::Right, r);
            } else {
                window.set_margin(Edge::Top, default_margin);
                window.set_margin(Edge::Right, default_margin);
            }
        }
        Position::BottomLeft => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            if let Some([_, b, l, _]) = saved_margins {
                window.set_margin(Edge::Bottom, b);
                window.set_margin(Edge::Left, l);
            } else {
                window.set_margin(Edge::Bottom, default_margin);
                window.set_margin(Edge::Left, default_margin);
            }
        }
        Position::BottomRight => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
            if let Some([_, b, _, r]) = saved_margins {
                window.set_margin(Edge::Bottom, b);
                window.set_margin(Edge::Right, r);
            } else {
                window.set_margin(Edge::Bottom, default_margin);
                window.set_margin(Edge::Right, default_margin);
            }
        }
    }
}

#[cfg(feature = "gui")]
fn apply_custom_css(window: &adw::ApplicationWindow, config: &OverlayConfig) {
    let mut css_parts = Vec::new();

    if let Some(ref font) = config.font {
        css_parts.push(format!("font-family: {font};"));
    }
    if let Some(ref color) = config.color {
        css_parts.push(format!("color: {color};"));
    }
    if let Some(ref bg) = config.background_color {
        let opacity = config.background_opacity.unwrap_or(0.9);
        css_parts.push(format!("background-color: {bg};"));
        css_parts.push(format!("opacity: {opacity};"));
    } else if let Some(opacity) = config.background_opacity {
        css_parts.push(format!("opacity: {opacity};"));
    }

    if !css_parts.is_empty() {
        let css = format!("window {{ {} }}", css_parts.join(" "));
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(&css);
        gtk4::style_context_add_provider_for_display(
            &gdk::Display::default().unwrap(),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    // Suppress unused warning when no CSS is needed
    let _ = window;
}

#[cfg(feature = "gui")]
fn setup_key_handler(window: &adw::ApplicationWindow) {
    let controller = gtk4::EventControllerKey::new();
    let window_weak = window.downgrade();
    controller.connect_key_pressed(move |_, key, _, modifier| {
        let Some(window) = window_weak.upgrade() else {
            return glib::Propagation::Proceed;
        };

        match key {
            gdk::Key::Escape => {
                window.close();
                glib::Propagation::Stop
            }
            gdk::Key::c if modifier.contains(gdk::ModifierType::CONTROL_MASK) => {
                copy_translation_to_clipboard(&window);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(controller);
}

#[cfg(feature = "gui")]
fn setup_drag_handler(window: &adw::ApplicationWindow, position: Position) {
    use gtk4_layer_shell::Edge;
    use std::cell::Cell;
    use std::rc::Rc;

    let drag = gtk4::GestureDrag::new();
    drag.set_propagation_phase(gtk4::PropagationPhase::Capture);

    // Track starting margins when drag begins
    let start_margins = Rc::new(Cell::new((0i32, 0i32, 0i32, 0i32))); // top, bottom, left, right

    let window_weak = window.downgrade();
    let margins = Rc::clone(&start_margins);
    drag.connect_drag_begin(move |_, _, _| {
        let Some(w) = window_weak.upgrade() else {
            return;
        };
        margins.set((
            w.margin(Edge::Top),
            w.margin(Edge::Bottom),
            w.margin(Edge::Left),
            w.margin(Edge::Right),
        ));
    });

    let window_weak = window.downgrade();
    let margins = Rc::clone(&start_margins);
    let pos = position;
    drag.connect_drag_update(move |_, dx, dy| {
        let Some(w) = window_weak.upgrade() else {
            return;
        };
        let (st, sb, sl, sr) = margins.get();
        let dx = dx as i32;
        let dy = dy as i32;

        // Adjust margins based on which edges are anchored
        match pos {
            Position::TopLeft => {
                w.set_margin(Edge::Top, (st + dy).max(0));
                w.set_margin(Edge::Left, (sl + dx).max(0));
            }
            Position::TopRight => {
                w.set_margin(Edge::Top, (st + dy).max(0));
                w.set_margin(Edge::Right, (sr - dx).max(0));
            }
            Position::BottomLeft => {
                w.set_margin(Edge::Bottom, (sb - dy).max(0));
                w.set_margin(Edge::Left, (sl + dx).max(0));
            }
            Position::BottomRight => {
                w.set_margin(Edge::Bottom, (sb - dy).max(0));
                w.set_margin(Edge::Right, (sr - dx).max(0));
            }
            Position::Center => {
                // Center has no anchors — no drag support
            }
        }
    });

    window.add_controller(drag);
}

#[cfg(feature = "gui")]
fn copy_translation_to_clipboard(window: &adw::ApplicationWindow) {
    // SAFETY: reading back a String we stored via set_data
    if let Some(clipboard_text) = unsafe { window.data::<String>("translation_text") } {
        let text = unsafe { clipboard_text.as_ref() };
        let display = gdk::Display::default().unwrap();
        let clipboard = display.clipboard();
        clipboard.set_text(text);
    }
}

#[cfg(feature = "gui")]
fn clear_body(body: &gtk4::Box) {
    while let Some(child) = body.first_child() {
        body.remove(&child);
    }
}

#[cfg(feature = "gui")]
fn show_spinner_in(body: &gtk4::Box) {
    let spinner = gtk4::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_width_request(48);
    spinner.set_height_request(48);

    let label = gtk4::Label::new(Some("Translating..."));
    label.add_css_class("dim-label");

    body.append(&spinner);
    body.append(&label);
}

#[cfg(feature = "gui")]
fn update_display(
    body: &gtk4::Box,
    entries: &[DisplayEntry],
    index: usize,
    prev_btn: &gtk4::Button,
    next_btn: &gtk4::Button,
    copy_btn: &gtk4::Button,
    window: &adw::ApplicationWindow,
) {
    clear_body(body);

    if let Some(entry) = entries.get(index) {
        if !entry.source_text.is_empty() {
            let label = gtk4::Label::new(None);
            label.set_wrap(true);
            label.set_selectable(true);
            label.add_css_class("dim-label");
            label.set_markup(&format!(
                "<i>{}</i>",
                glib::markup_escape_text(&entry.source_text)
            ));
            body.append(&label);
        }

        if !entry.translated_text.is_empty() {
            let label = gtk4::Label::new(None);
            label.set_wrap(true);
            label.set_selectable(true);
            label.set_markup(&format!(
                "<b>{}</b>",
                glib::markup_escape_text(&entry.translated_text)
            ));
            body.append(&label);

            // SAFETY: storing a String that lives as long as the window
            unsafe {
                window.set_data("translation_text", entry.translated_text.clone());
            }
            copy_btn.set_visible(true);
        } else {
            copy_btn.set_visible(false);
        }

        if entry.source_text.is_empty() && entry.translated_text.is_empty() {
            let label = gtk4::Label::new(Some("No text detected in image."));
            label.add_css_class("dim-label");
            body.append(&label);
        }

        // Position counter
        let counter = gtk4::Label::new(Some(&format!("{} / {}", index + 1, entries.len())));
        counter.add_css_class("dim-label");
        counter.add_css_class("caption");
        counter.set_margin_top(8);
        body.append(&counter);
    }

    prev_btn.set_sensitive(index > 0);
    next_btn.set_sensitive(index + 1 < entries.len());
}

#[cfg(feature = "gui")]
fn show_error(content_box: &gtk4::Box, error_msg: &str) {
    let error_label = gtk4::Label::new(None);
    error_label.set_markup("<span foreground=\"red\"><b>Translation failed</b></span>");
    content_box.append(&error_label);

    let detail_label = gtk4::Label::new(Some(error_msg));
    detail_label.set_wrap(true);
    detail_label.set_selectable(true);
    detail_label.add_css_class("dim-label");
    content_box.append(&detail_label);
}
