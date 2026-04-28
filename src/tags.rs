use std::collections::HashSet;

use arboard::Clipboard;
use log::error;

use crate::TagItem;

pub fn apply_collapse(tags: &[TagItem], collapsed: &HashSet<i32>) -> Vec<TagItem> {
    let mut result = Vec::new();
    let mut hide_above_depth: Option<i32> = None;
    for tag in tags {
        if let Some(hd) = hide_above_depth {
            if tag.depth > hd {
                continue;
            }
            hide_above_depth = None;
        }
        let mut out = tag.clone();
        if out.is_sequence && collapsed.contains(&out.source_index) {
            out.is_collapsed = true;
            hide_above_depth = Some(out.depth);
        }
        result.push(out);
    }
    result
}

pub fn rebuild_filtered(all: &[TagItem], query: &str, advanced: bool, collapsed: &HashSet<i32>) -> Vec<TagItem> {
    let searched: Vec<TagItem> = if query.is_empty() {
        all.to_vec()
    } else if advanced {
        filter_advanced(all, query)
    } else {
        all.iter()
            .filter(|t| {
                t.tag.to_lowercase().contains(query)
                    || t.name.to_lowercase().contains(query)
                    || t.value.to_lowercase().contains(query)
            })
            .cloned()
            .collect()
    };
    if query.is_empty() {
        apply_collapse(&searched, collapsed)
    } else {
        searched
    }
}

/// Returns ancestors + matching tags for advanced (contextual) search.
pub fn filter_advanced(all: &[TagItem], q: &str) -> Vec<TagItem> {
    let mut result: Vec<TagItem> = Vec::new();
    let mut included: Vec<bool> = vec![false; all.len()];

    let mut matched: Vec<usize> = Vec::new();
    for (i, t) in all.iter().enumerate() {
        if t.tag.to_lowercase().contains(q) || t.name.to_lowercase().contains(q) || t.value.to_lowercase().contains(q) {
            matched.push(i);
        }
    }

    for idx in matched {
        let target_depth = all[idx].depth;
        let mut ancestors: Vec<usize> = Vec::new();
        if target_depth > 0 {
            let mut needed_depth = target_depth - 1;
            let mut j = idx as i64 - 1;
            while j >= 0 {
                let t = &all[j as usize];
                if (t.is_item_header || t.is_sequence) && t.depth == needed_depth {
                    ancestors.push(j as usize);
                    if needed_depth == 0 {
                        break;
                    }
                    needed_depth -= 1;
                }
                j -= 1;
            }
        }
        ancestors.reverse();
        for a in ancestors {
            if !included[a] {
                included[a] = true;
                result.push(all[a].clone());
            }
        }
        if !included[idx] {
            included[idx] = true;
            result.push(all[idx].clone());
        }
    }
    result
}

pub fn format_tag_line(tag: &TagItem) -> String {
    let indent = "  ".repeat(tag.depth.max(0) as usize);
    if tag.is_item_header {
        return format!("{indent}({})", tag.name);
    }
    if tag.tag.is_empty() {
        return format!("{indent}{}: {}", tag.name, tag.value);
    }
    format!("{indent}{} {}: {}", tag.tag, tag.name, tag.value)
}

pub fn format_branch(all: &[TagItem], source_index: usize) -> String {
    if all.is_empty() || source_index >= all.len() {
        return String::new();
    }

    let mut stack: Vec<usize> = Vec::new();
    for i in 0..=source_index {
        while stack.last().is_some_and(|&last| all[last].depth >= all[i].depth) {
            stack.pop();
        }
        stack.push(i);
    }

    let mut lines: Vec<String> = stack.iter().map(|&i| format_tag_line(&all[i])).collect();
    let base_depth = all[source_index].depth;
    let mut i = source_index + 1;
    while i < all.len() && all[i].depth > base_depth {
        lines.push(format_tag_line(&all[i]));
        i += 1;
    }

    lines.join("\n")
}

pub fn copy_to_clipboard(cb_opt: &mut Option<Clipboard>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(c) = cb_opt
        && let Err(e) = c.set_text(text)
    {
        error!("Clipboard error: {e}");
    }
}
