use std::{
    cell::RefCell,
    collections::HashSet,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use image::{DynamicImage, ImageBuffer, Rgba};
use log::error;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::{
    ContentKind, FileItem, FrameItem, MainWindow, TagItem,
    loader::{self, LazyPixelDecoder, collect_file_paths, dynamic_image_to_slint, dynamic_image_to_thumbnail, load, scan_single_file},
};

/// Small dark-gray placeholder rendered for lazy frames before decoding.
fn placeholder_image() -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_pixel(64, 80, Rgba([35u8, 37, 50, 255])))
}

pub fn load_and_apply(
    path: &Path,
    window: &MainWindow,
    all_tags_store: &Rc<RefCell<Vec<TagItem>>>,
    all_entries_store: &Rc<RefCell<Vec<loader::TagEntry>>>,
    all_frames_store: &Rc<RefCell<Vec<DynamicImage>>>,
    lazy_store: &Rc<RefCell<Option<LazyPixelDecoder>>>,
    collapsed: &Rc<RefCell<HashSet<i32>>>,
) {
    let data = load(path).unwrap_or_else(|e| {
        error!("Error loading file: {e}");
        loader::FileData {
            frames: vec![],
            lazy_decoder: None,
            tags: vec![],
            sop_class: String::new(),
            image_info: String::new(),
        }
    });

    // Store lazy decoder (or clear if not applicable).
    *lazy_store.borrow_mut() = data.lazy_decoder;

    {
        let mut store = all_frames_store.borrow_mut();
        *store = data.frames;
    }

    // Build Slint frame list.
    // For lazy mode: generate placeholder thumbnails; for preloaded: use real images.
    let is_lazy = lazy_store.borrow().is_some();
    let slint_frames: Vec<FrameItem> = if is_lazy {
        let frame_count = lazy_store.borrow().as_ref().map_or(0, |d| d.frame_count);
        let ph = placeholder_image();
        let ph_slint = dynamic_image_to_slint(&ph);
        (0..frame_count)
            .map(|i| FrameItem {
                thumbnail: ph_slint.clone(),
                label: format!("{}", i + 1).into(),
            })
            .collect()
    } else {
        let frames = all_frames_store.borrow();
        frames
            .iter()
            .enumerate()
            .map(|(i, img)| FrameItem {
                thumbnail: dynamic_image_to_thumbnail(img),
                label: format!("{}", i + 1).into(),
            })
            .collect()
    };

    // For lazy mode: no image until user selects a frame.
    // For preloaded: show first frame immediately.
    if is_lazy {
        window.set_current_image(Default::default());
    } else {
        let frames = all_frames_store.borrow();
        if let Some(first) = frames.first() {
            window.set_current_image(dynamic_image_to_slint(first));
        } else {
            window.set_current_image(Default::default());
        }
    }
    window.set_current_frame_index(0);
    window.set_frame_count(slint_frames.len() as i32);
    window.set_frames(ModelRc::from(Rc::new(VecModel::from(slint_frames))));

    {
        let mut store = all_entries_store.borrow_mut();
        *store = data.tags;
    }
    let all_tags: Vec<TagItem> = {
        let entries = all_entries_store.borrow();
        entries
            .iter()
            .enumerate()
            .map(|(i, e)| TagItem {
                tag: e.tag.as_str().into(),
                name: e.name.as_str().into(),
                vr: e.vr.as_str().into(),
                value: e.value.as_str().into(),
                is_private: e.is_private,
                depth: e.depth as i32,
                is_item_header: e.is_item_header,
                is_sequence: e.is_sequence,
                is_collapsed: false,
                source_index: i as i32,
            })
            .collect()
    };

    collapsed.borrow_mut().clear();
    *all_tags_store.borrow_mut() = all_tags.clone();
    window.set_filtered_tags(ModelRc::from(Rc::new(VecModel::from(all_tags))));

    let file_name: SharedString = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .into();
    window.set_current_file_name(file_name);
    window.set_current_file_path(path.to_string_lossy().as_ref().into());
    window.set_current_sop_class(data.sop_class.as_str().into());
    window.set_image_size_info(data.image_info.as_str().into());
}

pub fn dir_entry_to_file_item(e: &loader::DirFileEntry) -> FileItem {
    FileItem {
        path: e.path.as_str().into(),
        sop_class: e.sop_class.as_str().into(),
        content_kind: match e.content_kind {
            loader::ContentKind::PixelData => ContentKind::PixelData,
            loader::ContentKind::EncapsulatedPdf => ContentKind::EncapsulatedPdf,
            loader::ContentKind::Other => ContentKind::Other,
        },
        is_header: false,
    }
}

/// Inject group-header items before each SOP-class group.
/// Input must already be sorted by sop_class.
pub fn with_group_headers(items: Vec<FileItem>) -> Vec<FileItem> {
    let mut result: Vec<FileItem> = Vec::with_capacity(items.len() + 8);
    let mut current_sop: Option<slint::SharedString> = None;
    for item in items {
        if current_sop.as_ref() != Some(&item.sop_class) {
            result.push(FileItem {
                path: "".into(),
                sop_class: item.sop_class.clone(),
                content_kind: ContentKind::Other,
                is_header: true,
            });
            current_sop = Some(item.sop_class.clone());
        }
        result.push(item);
    }
    result
}

/// Filter + sort the full directory entry list.
pub fn filter_dir_entries(
    entries: &[loader::DirFileEntry],
    query: &str,
    sort_by_sop: bool,
    only_visible: bool,
) -> Vec<loader::DirFileEntry> {
    let q = query.to_lowercase();
    let mut filtered: Vec<&loader::DirFileEntry> = entries
        .iter()
        .filter(|e| {
            (q.is_empty()
                || e.sop_class.to_lowercase().contains(q.as_str())
                || e.path.to_lowercase().contains(q.as_str()))
                && (!only_visible || e.content_kind != loader::ContentKind::Other)
        })
        .collect();
    if sort_by_sop {
        filtered.sort_by(|a, b| a.sop_class.cmp(&b.sop_class).then(a.path.cmp(&b.path)));
    }
    filtered.into_iter().cloned().collect()
}

/// Scan `dir` recursively in a background thread, reporting progress via the
/// loading screen.  Returns a `Timer` that must be kept alive by the caller
/// until the scan finishes (dropping it cancels the scan).
pub fn apply_directory_async(
    dir: &Path,
    window: &MainWindow,
    all_dir_entries_store: Rc<RefCell<Vec<loader::DirFileEntry>>>,
    all_tags_store: Rc<RefCell<Vec<TagItem>>>,
    all_entries_store: Rc<RefCell<Vec<loader::TagEntry>>>,
    all_frames_store: Rc<RefCell<Vec<DynamicImage>>>,
    lazy_store: Rc<RefCell<Option<LazyPixelDecoder>>>,
    collapsed: Rc<RefCell<HashSet<i32>>>,
    only_visible: bool,
) -> Rc<Timer> {
    let paths = collect_file_paths(dir);
    let total = paths.len();

    if total == 0 {
        error!("No DICOM/PDF files found in: {}", dir.display());
        return Rc::new(Timer::default());
    }

    window.set_dir_mode(true);
    window.set_dir_path(dir.to_string_lossy().as_ref().into());
    window.set_loading(true);
    window.set_loading_current(0);
    window.set_loading_total(total as i32);

    // Background thread sends scanned entries through a channel.
    let (tx, rx) = std::sync::mpsc::channel::<loader::DirFileEntry>();
    std::thread::spawn(move || {
        for path in paths {
            let entry = scan_single_file(&path);
            if tx.send(entry).is_err() {
                break;
            }
        }
    });

    let rx = Arc::new(Mutex::new(rx));
    let results: Rc<RefCell<Vec<loader::DirFileEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let received = Rc::new(std::cell::Cell::new(0usize));

    let window_weak = window.as_weak();
    let results_cb = results;
    let received_cb = received;

    let timer = Rc::new(Timer::default());
    let timer_ref = timer.clone();
    timer.start(TimerMode::Repeated, std::time::Duration::from_millis(16), move || {
        // Drain whatever the background thread has produced so far.
        {
            let lock = rx.lock().unwrap();
            while let Ok(entry) = lock.try_recv() {
                results_cb.borrow_mut().push(entry);
                received_cb.set(received_cb.get() + 1);
            }
        }

        let count = received_cb.get();
        let Some(w) = window_weak.upgrade() else { return };
        w.set_loading_current(count as i32);

        if count < total {
            return;
        }

        // --- All files scanned — stop the timer.
        timer_ref.stop();

        let mut entries = results_cb.borrow().clone();
        // Default: sort by SOP class, then path.
        entries.sort_by(|a, b| a.sop_class.cmp(&b.sop_class).then(a.path.cmp(&b.path)));
        *all_dir_entries_store.borrow_mut() = entries.clone();

        let display = filter_dir_entries(&entries, "", true, only_visible);
        let file_items = with_group_headers(display.iter().map(dir_entry_to_file_item).collect());
        w.set_dir_files(ModelRc::from(Rc::new(VecModel::from(file_items))));

        if let Some(first) = display.first() {
            let first_path = std::path::PathBuf::from(&first.path);
            load_and_apply(
                &first_path,
                &w,
                &all_tags_store,
                &all_entries_store,
                &all_frames_store,
                &lazy_store,
                &collapsed,
            );
        }

        w.set_loading(false);
    });

    timer
}
