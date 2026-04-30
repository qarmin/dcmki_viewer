use std::{cell::RefCell, collections::HashSet, env, path::Path, rc::Rc};

use arboard::Clipboard;
use image::DynamicImage;
use handsome_logger::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};
use log::{Record, error};
use slint::{ModelRc, SharedString, Timer, VecModel};

mod error;
mod file_ops;
mod loader;
mod tags;
mod visual_field;

use error::AppError;
use file_ops::{apply_directory_async, dir_entry_to_file_item, filter_dir_entries, load_and_apply, with_group_headers};
use loader::dynamic_image_to_slint;
use tags::{copy_to_clipboard, format_branch, rebuild_filtered};

slint::include_modules!();

fn filter_log(record: &Record) -> bool {
    match record.module_path() {
        Some(path) if path.starts_with("dcmki_viewer") => true,
        Some(path) if path.starts_with("dicom_pixeldata") => record.level() <= log::Level::Error,
        Some(_) => record.level() <= log::Level::Warn,
        None => true,
    }
}

fn main() -> Result<(), AppError> {
    let config = ConfigBuilder::default()
        .set_message_filtering(Some(filter_log))
        .build();
    let _ = TermLogger::init(config, TerminalMode::Mixed, ColorChoice::Auto);
    let args: Vec<String> = env::args().collect();

    let window = MainWindow::new()?;
    window.window().set_size(slint::WindowSize::Logical(slint::LogicalSize {
        width: 1700.0,
        height: 1000.0,
    }));

    let all_tags_store: Rc<RefCell<Vec<TagItem>>> = Rc::new(RefCell::new(Vec::new()));
    let all_entries_store: Rc<RefCell<Vec<loader::TagEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let all_frames_store: Rc<RefCell<Vec<DynamicImage>>> = Rc::new(RefCell::new(Vec::new()));
    let all_dir_entries_store: Rc<RefCell<Vec<loader::DirFileEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let collapsed_store: Rc<RefCell<HashSet<i32>>> = Rc::new(RefCell::new(HashSet::new()));
    let search_state: Rc<RefCell<(String, i32)>> = Rc::new(RefCell::new((String::new(), 0)));
    // Lazy pixel decoder for large DICOM files (keeps raw DICOM in memory, decodes on demand).
    let lazy_decoder_store: Rc<RefCell<Option<loader::LazyPixelDecoder>>> = Rc::new(RefCell::new(None));
    // Keeps the active directory scan timer alive; replaced on each new scan.
    let scan_timer: Rc<RefCell<Option<Rc<Timer>>>> = Rc::new(RefCell::new(None));
    // Keep Clipboard alive so X11 background thread keeps serving requests.
    let clipboard: Rc<RefCell<Option<Clipboard>>> = Rc::new(RefCell::new(Clipboard::new().ok()));

    // Frame navigation — for preloaded frames use the store; for lazy DICOM decode on demand.
    {
        let window_weak = window.as_weak();
        let frames_rc = all_frames_store.clone();
        let lazy_rc = lazy_decoder_store.clone();
        window.on_frame_selected(move |idx: i32| {
            let Some(w) = window_weak.upgrade() else { return };
            if idx < 0 { return; }
            // Try lazy decoder first.
            if let Some(decoder) = lazy_rc.borrow().as_ref() {
                if let Some(img) = decoder.decode_frame(idx as u32) {
                    w.set_current_image(dynamic_image_to_slint(&img));
                    // img is dropped here — only one decoded frame lives at a time.
                }
                return;
            }
            // Fallback: preloaded frames.
            let frames = frames_rc.borrow();
            if let Some(img) = frames.get(idx as usize) {
                w.set_current_image(dynamic_image_to_slint(img));
            }
        });
    }

    // File selected from directory list
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let entries_store = all_entries_store.clone();
        let frames_store = all_frames_store.clone();
        let lazy_store = lazy_decoder_store.clone();
        let collapsed = collapsed_store.clone();
        let search_state = search_state.clone();
        window.on_file_selected(move |path: SharedString| {
            if let Some(w) = window_weak.upgrade() {
                load_and_apply(
                    Path::new(path.as_str()),
                    &w,
                    &tags_store,
                    &entries_store,
                    &frames_store,
                    &lazy_store,
                    &collapsed,
                );
                // Clear search so the box and the Rust state stay in sync
                w.set_tag_search_text(SharedString::from(""));
                *search_state.borrow_mut() = (String::new(), 0);
            }
        });
    }

    // Open single file via dialog
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let entries_store = all_entries_store.clone();
        let frames_store = all_frames_store.clone();
        let lazy_store = lazy_decoder_store.clone();
        let collapsed = collapsed_store.clone();
        let search_state = search_state.clone();
        window.on_open_file_dialog(move || {
            if let Some(w) = window_weak.upgrade()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Medical files", &["dcm", "pdf"])
                    .add_filter("All files", &["*"])
                    .pick_file()
            {
                w.set_dir_mode(false);
                load_and_apply(&path, &w, &tags_store, &entries_store, &frames_store, &lazy_store, &collapsed);
                w.set_tag_search_text(SharedString::from(""));
                *search_state.borrow_mut() = (String::new(), 0);
            }
        });
    }

    // Open folder via dialog
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let entries_store = all_entries_store.clone();
        let frames_store = all_frames_store.clone();
        let lazy_store = lazy_decoder_store.clone();
        let dir_entries_store = all_dir_entries_store.clone();
        let collapsed = collapsed_store.clone();
        let scan_timer = scan_timer.clone();
        window.on_open_folder_dialog(move || {
            if let Some(w) = window_weak.upgrade()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                let only_visible = w.get_only_visible();
                let timer = apply_directory_async(
                    &dir,
                    &w,
                    dir_entries_store.clone(),
                    tags_store.clone(),
                    entries_store.clone(),
                    frames_store.clone(),
                    lazy_store.clone(),
                    collapsed.clone(),
                    only_visible,
                );
                *scan_timer.borrow_mut() = Some(timer);
            }
        });
    }

    // Filter / sort directory file list
    {
        let window_weak = window.as_weak();
        let dir_entries_store = all_dir_entries_store.clone();
        window.on_filter_dir_files(move |query: SharedString, sort_by_sop: bool, only_visible: bool| {
            if let Some(w) = window_weak.upgrade() {
                let entries = dir_entries_store.borrow();
                let display = filter_dir_entries(&entries, query.as_str(), sort_by_sop, only_visible);
                let file_items = with_group_headers(display.iter().map(dir_entry_to_file_item).collect());
                w.set_dir_files(ModelRc::from(Rc::new(VecModel::from(file_items))));
            }
        });
    }

    // Load initial content from command-line argument
    if args.len() >= 2 {
        #[expect(clippy::indexing_slicing)] // Validated by args.len() check
        let input = Path::new(&args[1]);
        if input.is_dir() {
            let only_visible = window.get_only_visible();
            let timer = apply_directory_async(
                input,
                &window,
                all_dir_entries_store,
                all_tags_store.clone(),
                all_entries_store.clone(),
                all_frames_store.clone(),
                lazy_decoder_store.clone(),
                collapsed_store.clone(),
                only_visible,
            );
            *scan_timer.borrow_mut() = Some(timer);
        } else {
            load_and_apply(
                input,
                &window,
                &all_tags_store,
                &all_entries_store,
                &all_frames_store,
                &lazy_decoder_store,
                &collapsed_store,
            );
        }
    }

    // Search
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let collapsed = collapsed_store.clone();
        let search_state = search_state.clone();
        window.on_search_changed(move |query: SharedString, mode: i32| {
            if let Some(w) = window_weak.upgrade() {
                let q = query.to_lowercase();
                *search_state.borrow_mut() = (q.clone(), mode);
                let all = tags_store.borrow();
                let filtered = rebuild_filtered(&all, &q, mode, &collapsed.borrow());
                w.set_filtered_tags(ModelRc::from(Rc::new(VecModel::from(filtered))));
            }
        });
    }

    // Toggle sequence collapse
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let collapsed = collapsed_store.clone();
        let search_state = search_state.clone();
        window.on_toggle_sequence(move |source_index: i32| {
            if let Some(w) = window_weak.upgrade() {
                {
                    let mut c = collapsed.borrow_mut();
                    if c.contains(&source_index) {
                        c.remove(&source_index);
                    } else {
                        c.insert(source_index);
                    }
                }
                let all = tags_store.borrow();
                let (q, mode) = search_state.borrow().clone();
                let filtered = rebuild_filtered(&all, &q, mode, &collapsed.borrow());
                w.set_filtered_tags(ModelRc::from(Rc::new(VecModel::from(filtered))));
            }
        });
    }

    // Collapse all sequences
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let collapsed = collapsed_store.clone();
        let search_state = search_state.clone();
        window.on_collapse_all(move || {
            if let Some(w) = window_weak.upgrade() {
                {
                    let all = tags_store.borrow();
                    let mut c = collapsed.borrow_mut();
                    for tag in all.iter() {
                        if tag.is_sequence {
                            c.insert(tag.source_index);
                        }
                    }
                }
                let all = tags_store.borrow();
                let (q, mode) = search_state.borrow().clone();
                let filtered = rebuild_filtered(&all, &q, mode, &collapsed.borrow());
                w.set_filtered_tags(ModelRc::from(Rc::new(VecModel::from(filtered))));
            }
        });
    }

    // Expand all sequences
    {
        let window_weak = window.as_weak();
        let tags_store = all_tags_store.clone();
        let collapsed = collapsed_store;
        window.on_expand_all(move || {
            if let Some(w) = window_weak.upgrade() {
                collapsed.borrow_mut().clear();
                let all = tags_store.borrow();
                let (q, mode) = search_state.borrow().clone();
                let filtered = rebuild_filtered(&all, &q, mode, &collapsed.borrow());
                w.set_filtered_tags(ModelRc::from(Rc::new(VecModel::from(filtered))));
            }
        });
    }

    // Export tags to text file
    {
        let entries_store = all_entries_store;
        window.on_export_tags(move || {
            let entries = entries_store.borrow();
            if entries.is_empty() {
                return;
            }
            let mut lines: Vec<String> = Vec::new();
            for e in entries.iter() {
                let indent = "  ".repeat(e.depth as usize);
                if e.is_item_header {
                    lines.push(format!("{indent}-- {} --", e.name));
                } else {
                    let vr_part = if e.vr.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", e.vr)
                    };
                    lines.push(format!("{indent}{}{} {}: {}", e.tag, vr_part, e.name, e.value_full));
                }
            }
            let text = lines.join("\n");
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Text file", &["txt"])
                .set_file_name("dicom_tags.txt")
                .save_file()
                && let Err(e) = std::fs::write(&path, text)
            {
                error!("Export error: {e}");
            }
        });
    }

    // Save current frame as PNG
    {
        let window_weak = window.as_weak();
        let frames_store = all_frames_store.clone();
        let lazy_store = lazy_decoder_store.clone();
        window.on_save_current_frame(move || {
            if let Some(w) = window_weak.upgrade() {
                let idx = w.get_current_frame_index();
                if idx < 0 { return; }
                // Try lazy decoder first
                let img = if let Some(decoder) = lazy_store.borrow().as_ref() {
                    decoder.decode_frame(idx as u32)
                } else {
                    frames_store.borrow().get(idx as usize).cloned()
                };
                if let Some(img) = img
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG image", &["png"])
                        .set_file_name("frame.png")
                        .save_file()
                    && let Err(e) = img.save(&path)
                {
                    error!("Save frame error: {e}");
                }
            }
        });
    }

    // Save all frames as PNG files
    {
        let frames_store = all_frames_store;
        let lazy_store = lazy_decoder_store;
        window.on_save_all_frames(move || {
            let frames = frames_store.borrow();
            let lazy = lazy_store.borrow();
            let is_lazy = lazy.is_some();
            let count = if is_lazy {
                lazy.as_ref().map_or(0, |d| d.frame_count as usize)
            } else {
                frames.len()
            };
            if count == 0 { return; }

            if count == 1 {
                let img = if is_lazy {
                    lazy.as_ref().and_then(|d| d.decode_frame(0))
                } else {
                    frames.first().cloned()
                };
                if let Some(img) = img
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG image", &["png"])
                        .set_file_name("frame.png")
                        .save_file()
                    && let Err(e) = img.save(&path)
                {
                    error!("Save error: {e}");
                }
            } else if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                for i in 0..count {
                    let img = if is_lazy {
                        lazy.as_ref().and_then(|d| d.decode_frame(i as u32))
                    } else {
                        frames.get(i).cloned()
                    };
                    if let Some(img) = img {
                        let path = dir.join(format!("frame_{:04}.png", i + 1));
                        if let Err(e) = img.save(&path) {
                            error!("Save frame {i} error: {e}");
                        }
                    }
                }
            }
        });
    }

    // Context menu copy actions
    {
        let tags_store_copy = all_tags_store;
        let clipboard_rc = clipboard;
        window.on_tag_copy_request(move |action: CopyAction, source_index: i32| {
            if source_index < 0 {
                return;
            }
            let all = tags_store_copy.borrow();
            let idx = source_index as usize;
            let Some(tag) = all.get(idx) else {
                return;
            };
            let mut cb = clipboard_rc.borrow_mut();
            match action {
                CopyAction::Value => copy_to_clipboard(&mut cb, tag.value.as_str()),
                CopyAction::Tag => copy_to_clipboard(&mut cb, tag.tag.as_str()),
                CopyAction::Name => copy_to_clipboard(&mut cb, tag.name.as_str()),
                CopyAction::Branch => {
                    let text = format_branch(&all, idx);
                    copy_to_clipboard(&mut cb, &text);
                }
            }
        });
    }

    window.run()?;
    Ok(())
}
