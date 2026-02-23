use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use crate::provider::AnyProvider;
use crate::screenshot;

slint::include_modules!();

const PID_FILE: &str = "/tmp/satori-overlay.pid";

/// Kill any previous satori overlay process and write our PID.
fn manage_pid_file() {
    if let Ok(content) = std::fs::read_to_string(PID_FILE)
        && let Ok(pid) = content.trim().parse::<u32>()
    {
        // SAFETY: sending SIGTERM to a process by PID (best effort)
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = std::fs::write(PID_FILE, std::process::id().to_string());
}

fn cleanup_pid_file() {
    let _ = std::fs::remove_file(PID_FILE);
}

struct DisplayEntry {
    source_text: String,
    translated_text: String,
}

/// Run the Slint overlay application.
pub fn run(
    provider: AnyProvider,
    target_lang: String,
    translation_style: String,
    image_data: Option<Vec<u8>>,
) {
    manage_pid_file();

    let overlay = SatoriOverlay::new().unwrap();
    let provider = Arc::new(provider);

    // Shared state (Rc — only accessed on UI thread)
    let entries: Rc<RefCell<Vec<DisplayEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let current_idx: Rc<Cell<usize>> = Rc::new(Cell::new(0));
    let is_translating: Rc<Cell<bool>> = Rc::new(Cell::new(true));

    // Load history
    let history = crate::history::list_entries(Some(20)).unwrap_or_default();
    let has_history = !history.is_empty();
    for h in history {
        entries.borrow_mut().push(DisplayEntry {
            source_text: h.source_text,
            translated_text: h.translated_text,
        });
    }
    current_idx.set(entries.borrow().len());

    overlay.set_prev_enabled(has_history);
    overlay.set_screenshot_enabled(false);

    // Close handler
    overlay.on_close_requested({
        let weak = overlay.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.hide().unwrap();
            }
            slint::quit_event_loop().unwrap();
        }
    });

    // Copy handler — use wl-copy
    overlay.on_copy_clicked({
        let ent = Rc::clone(&entries);
        let idx = Rc::clone(&current_idx);
        move || {
            let entries = ent.borrow();
            if let Some(entry) = entries.get(idx.get())
                && !entry.translated_text.is_empty()
            {
                let _ = std::process::Command::new("wl-copy")
                    .arg(&entry.translated_text)
                    .spawn();
            }
        }
    });

    // Prev handler
    overlay.on_prev_clicked({
        let weak = overlay.as_weak();
        let ent = Rc::clone(&entries);
        let idx = Rc::clone(&current_idx);
        move || {
            let cur = idx.get();
            if cur > 0 {
                idx.set(cur - 1);
                if let Some(ui) = weak.upgrade() {
                    update_display(&ui, &ent.borrow(), cur - 1);
                }
            }
        }
    });

    // Next handler
    overlay.on_next_clicked({
        let weak = overlay.as_weak();
        let ent = Rc::clone(&entries);
        let idx = Rc::clone(&current_idx);
        move || {
            let cur = idx.get();
            let len = ent.borrow().len();
            if cur + 1 < len {
                idx.set(cur + 1);
                if let Some(ui) = weak.upgrade() {
                    update_display(&ui, &ent.borrow(), cur + 1);
                }
            }
        }
    });

    // Translation completed callback — runs on UI thread, safe to use Rc
    overlay.on_translation_completed({
        let weak = overlay.as_weak();
        let ent = Rc::clone(&entries);
        let idx = Rc::clone(&current_idx);
        let busy = Rc::clone(&is_translating);
        move |source: slint::SharedString, translated: slint::SharedString| {
            ent.borrow_mut().push(DisplayEntry {
                source_text: source.to_string(),
                translated_text: translated.to_string(),
            });
            let new_idx = ent.borrow().len() - 1;
            idx.set(new_idx);
            if let Some(ui) = weak.upgrade() {
                update_display(&ui, &ent.borrow(), new_idx);
                ui.set_screenshot_enabled(true);
            }
            busy.set(false);
        }
    });

    // Translation failed callback — runs on UI thread
    overlay.on_translation_failed({
        let weak = overlay.as_weak();
        let busy = Rc::clone(&is_translating);
        move |error: slint::SharedString| {
            if let Some(ui) = weak.upgrade() {
                ui.set_is_loading(false);
                ui.set_error_text(error);
                ui.set_screenshot_enabled(true);
            }
            busy.set(false);
        }
    });

    // Screenshot handler
    overlay.on_screenshot_clicked({
        let weak = overlay.as_weak();
        let busy = Rc::clone(&is_translating);
        let prov = Arc::clone(&provider);
        let lang = target_lang.clone();
        let style = translation_style.clone();

        move || {
            if busy.get() {
                return;
            }
            let Some(ui) = weak.upgrade() else { return };
            busy.set(true);
            ui.set_screenshot_enabled(false);
            ui.hide().unwrap();

            let prov = Arc::clone(&prov);
            let lang = lang.clone();
            let style = style.clone();
            let weak2 = weak.clone();

            slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
                let shot = screenshot::take_screenshot();
                let Some(ui) = weak2.upgrade() else { return };
                ui.show().unwrap();

                match shot {
                    Err(e) => {
                        ui.invoke_translation_failed(format!("Screenshot failed: {e}").into());
                    }
                    Ok(img) => {
                        ui.set_is_loading(true);
                        ui.set_error_text("".into());
                        fire_translation(prov, img, lang, style, weak2.clone());
                    }
                }
            });
        }
    });

    // Start initial translation
    let initial_image =
        image_data.unwrap_or_else(|| screenshot::take_screenshot().expect("Screenshot failed"));

    fire_translation(
        Arc::clone(&provider),
        initial_image,
        target_lang,
        translation_style,
        overlay.as_weak(),
    );

    overlay.run().unwrap();
    cleanup_pid_file();
}

/// Spawn a background translation thread. Results are delivered via
/// `translation-completed` / `translation-failed` Slint callbacks on the UI thread.
fn fire_translation(
    provider: Arc<AnyProvider>,
    image_data: Vec<u8>,
    lang: String,
    style: String,
    weak: slint::Weak<SatoriOverlay>,
) {
    let lang2 = lang.clone();
    let style2 = style.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(provider.translate(&image_data, &lang2, &style2));

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(tr) => {
                    let _ =
                        crate::history::record(&tr.source_text, &tr.translated_text, &lang, &style);
                    ui.invoke_translation_completed(
                        tr.source_text.into(),
                        tr.translated_text.into(),
                    );
                }
                Err(e) => {
                    ui.invoke_translation_failed(format!("{e}").into());
                }
            }
        });
    });
}

fn update_display(ui: &SatoriOverlay, entries: &[DisplayEntry], index: usize) {
    if let Some(entry) = entries.get(index) {
        // Adapt window width based on content
        let max_chars = entry
            .source_text
            .chars()
            .count()
            .max(entry.translated_text.chars().count());
        let width = match max_chars {
            0..=30 => 320,
            31..=80 => 400,
            81..=200 => 500,
            201..=500 => 600,
            _ => 700,
        };
        ui.window().set_size(slint::LogicalSize::new(
            width as f32,
            ui.window().size().height as f32,
        ));

        ui.set_source_text(entry.source_text.clone().into());
        ui.set_translated_text(entry.translated_text.clone().into());
        ui.set_counter_text(format!("{} / {}", index + 1, entries.len()).into());
        ui.set_is_loading(false);
        ui.set_error_text("".into());
        ui.set_copy_visible(!entry.translated_text.is_empty());
    }

    ui.set_prev_enabled(index > 0);
    ui.set_next_enabled(index + 1 < entries.len());
}
