//! The `Space` leader menu: a which-key style popup that lists what the next
//! key does, so actions can be discovered instead of memorized.
//!
//! Pressing `Space` in Normal mode (or in the explorer) opens the menu. Keys
//! either run an action or open a group (`f` → file, `u` → UI, `q` → quit),
//! whose own keys then replace the listing. `Esc` closes the menu and
//! `Backspace` goes back up one level.

use crate::theme;
use iocraft::prelude::*;

/// Something the leader menu can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderAction {
    /// Fuzzy-find a file in the workspace (same as `Ctrl-p`).
    FindFile,
    /// Show, focus or hide the file explorer.
    Explorer,
    /// Start a forward search in the document (same as `/`).
    Search,
    /// Write the buffer (`:w`).
    Save,
    /// Quit, refusing if there are unsaved changes (`:q`).
    Quit,
    /// Write and quit (`:wq`).
    SaveQuit,
    /// Quit and discard changes (`:q!`).
    ForceQuit,
    /// Cycle to the next color theme (same as `Ctrl-t`).
    NextTheme,
    /// Toggle Unicode / image math rendering (`:math`).
    ToggleMath,
    /// Serve the workspace and open it in the browser (`:serve`).
    Serve,
    /// Enlarge images, diagrams and formulas (same as `]`).
    GrowGraphics,
    /// Shrink images, diagrams and formulas (same as `[`).
    ShrinkGraphics,
    /// Show the full keyboard reference.
    Keymaps,
}

/// One entry in a leader menu level.
pub struct LeaderEntry {
    /// Key that selects the entry.
    pub key: char,
    /// Short description shown next to the key.
    pub label: &'static str,
    /// What the key does.
    pub kind: EntryKind,
}

/// Whether a leader entry runs an action or opens a nested group.
pub enum EntryKind {
    /// Run the action and close the menu.
    Action(LeaderAction),
    /// Descend into a group of entries.
    Group(&'static [LeaderEntry]),
}

use EntryKind::{Action, Group};
use LeaderAction as A;

const FILE: &[LeaderEntry] = &[
    LeaderEntry { key: 'f', label: "Find file", kind: Action(A::FindFile) },
    LeaderEntry { key: 'e', label: "Explorer", kind: Action(A::Explorer) },
    LeaderEntry { key: 'w', label: "Save", kind: Action(A::Save) },
    LeaderEntry { key: 'b', label: "Open in browser", kind: Action(A::Serve) },
];

const UI: &[LeaderEntry] = &[
    LeaderEntry { key: 't', label: "Next theme", kind: Action(A::NextTheme) },
    LeaderEntry { key: 'm', label: "Toggle math mode", kind: Action(A::ToggleMath) },
    LeaderEntry { key: '+', label: "Larger graphics", kind: Action(A::GrowGraphics) },
    LeaderEntry { key: '-', label: "Smaller graphics", kind: Action(A::ShrinkGraphics) },
];

const QUIT: &[LeaderEntry] = &[
    LeaderEntry { key: 'q', label: "Quit", kind: Action(A::Quit) },
    LeaderEntry { key: 'w', label: "Save and quit", kind: Action(A::SaveQuit) },
    LeaderEntry { key: '!', label: "Quit without saving", kind: Action(A::ForceQuit) },
];

/// The top level of the leader menu.
pub const ROOT: &[LeaderEntry] = &[
    LeaderEntry { key: ' ', label: "Find file", kind: Action(A::FindFile) },
    LeaderEntry { key: 'e', label: "Explorer", kind: Action(A::Explorer) },
    LeaderEntry { key: '/', label: "Search document", kind: Action(A::Search) },
    LeaderEntry { key: 'w', label: "Save", kind: Action(A::Save) },
    LeaderEntry { key: 'f', label: "file", kind: Group(FILE) },
    LeaderEntry { key: 'u', label: "ui", kind: Group(UI) },
    LeaderEntry { key: 'q', label: "quit", kind: Group(QUIT) },
    LeaderEntry { key: '?', label: "All keymaps", kind: Action(A::Keymaps) },
];

/// Entries of the menu level reached by typing `path` after `Space`, or
/// `None` if `path` doesn't name a group.
pub fn level(path: &[char]) -> Option<&'static [LeaderEntry]> {
    let mut entries = ROOT;
    for key in path {
        match entries.iter().find(|e| e.key == *key)?.kind {
            Group(children) => entries = children,
            Action(_) => return None,
        }
    }
    Some(entries)
}

/// Outcome of pressing `key` at menu level `path`.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// The key opened a group; the menu stays open one level deeper.
    Descend,
    /// The key chose an action; the menu closes.
    Run(LeaderAction),
    /// The key isn't bound at this level; the menu closes.
    Unbound,
}

/// Resolves `key` at menu level `path`.
pub fn step(path: &[char], key: char) -> Step {
    let Some(entries) = level(path) else {
        return Step::Unbound;
    };
    match entries.iter().find(|e| e.key == key).map(|e| &e.kind) {
        Some(Group(_)) => Step::Descend,
        Some(Action(action)) => Step::Run(*action),
        None => Step::Unbound,
    }
}

/// How a key is printed in the menu.
fn key_label(key: char) -> String {
    match key {
        ' ' => "␣".to_string(),
        c => c.to_string(),
    }
}

/// Rows the menu occupies for the level at `path` (used to size the panel).
pub fn menu_height(path: &[char], width: u32) -> u32 {
    let n = level(path).map_or(0, |e| e.len()) as u32;
    let cols = columns_for(width);
    // Title row + entries + top/bottom border.
    1 + n.div_ceil(cols) + 2
}

/// Width of one key/label cell in the menu.
const CELL_W: u32 = 26;

fn columns_for(width: u32) -> u32 {
    (width.saturating_sub(4) / CELL_W).clamp(1, 4)
}

/// Properties for the [`LeaderMenu`] component.
#[derive(Default, Props)]
pub struct LeaderMenuProps {
    /// Keys typed after `Space` so far (empty at the top level).
    pub path: Vec<char>,
    /// Width of the area the menu spans.
    pub width: u32,
}

/// Which-key style panel docked above the status line.
#[component]
pub fn LeaderMenu(props: &LeaderMenuProps) -> impl Into<AnyElement<'static>> {
    let entries = level(&props.path).unwrap_or(ROOT);
    let cols = columns_for(props.width) as usize;
    let rows = entries.len().div_ceil(cols);
    let breadcrumb = std::iter::once("Space".to_string())
        .chain(props.path.iter().enumerate().map(|(i, k)| {
            // Name the group, not just its key: "Space › file".
            level(&props.path[..i])
                .and_then(|es| es.iter().find(|e| e.key == *k))
                .map_or_else(|| key_label(*k), |e| e.label.to_string())
        }))
        .collect::<Vec<_>>()
        .join(" › ");
    let back_hint = if props.path.is_empty() { "close" } else { "back" };
    let back_key = if props.path.is_empty() { "esc" } else { "⌫" };

    // Fill column-major so related entries read top to bottom.
    let columns: Vec<Vec<&LeaderEntry>> = (0..cols)
        .map(|c| entries.iter().skip(c * rows).take(rows).collect())
        .filter(|c: &Vec<&LeaderEntry>| !c.is_empty())
        .collect();

    element! {
        View(
            width: props.width,
            flex_direction: FlexDirection::Column,
            background_color: theme::status_bg(),
            border_style: BorderStyle::Round,
            border_color: theme::border(),
            padding_left: 1,
            padding_right: 1,
        ) {
            View(height: 1, flex_direction: FlexDirection::Row) {
                Text(content: breadcrumb, color: theme::blue(), weight: Weight::Bold, wrap: TextWrap::NoWrap)
                View(flex_grow: 1.0) {}
                Text(content: back_key, color: theme::fg(), weight: Weight::Bold)
                Text(content: format!(" {back_hint}"), color: theme::comment())
            }
            View(flex_direction: FlexDirection::Row) {
                #(columns.into_iter().map(|col| element! {
                    View(flex_direction: FlexDirection::Column, width: CELL_W) {
                        #(col.into_iter().map(|e| {
                            let is_group = matches!(e.kind, Group(_));
                            let label = if is_group { format!("+{}", e.label) } else { e.label.to_string() };
                            element! {
                                View(height: 1, flex_direction: FlexDirection::Row) {
                                    View(width: 3, flex_shrink: 0.0) {
                                        Text(content: key_label(e.key), color: theme::yellow(), weight: Weight::Bold)
                                    }
                                    Text(content: "→ ", color: theme::dark_grey())
                                    Text(
                                        content: label,
                                        color: if is_group { theme::magenta() } else { theme::fg() },
                                        weight: if is_group { Weight::Bold } else { Weight::Normal },
                                        wrap: TextWrap::NoWrap,
                                    )
                                }
                            }.into_any()
                        }))
                    }
                }.into_any()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_actions_and_groups() {
        assert_eq!(step(&[], ' '), Step::Run(LeaderAction::FindFile));
        assert_eq!(step(&[], 'e'), Step::Run(LeaderAction::Explorer));
        assert_eq!(step(&[], 'f'), Step::Descend);
        assert_eq!(step(&[], 'z'), Step::Unbound);
    }

    #[test]
    fn nested_groups_resolve() {
        assert_eq!(step(&['f'], 'f'), Step::Run(LeaderAction::FindFile));
        assert_eq!(step(&['u'], 't'), Step::Run(LeaderAction::NextTheme));
        assert_eq!(step(&['q'], '!'), Step::Run(LeaderAction::ForceQuit));
        assert_eq!(step(&['q'], 'x'), Step::Unbound);
        assert!(level(&['e']).is_none());
    }

    #[test]
    fn keys_are_unique_per_level() {
        fn check(entries: &[LeaderEntry]) {
            for (i, a) in entries.iter().enumerate() {
                assert!(entries[i + 1..].iter().all(|b| b.key != a.key), "duplicate key {:?}", a.key);
                if let Group(children) = a.kind {
                    check(children);
                }
            }
        }
        check(ROOT);
    }

    #[test]
    fn height_accounts_for_columns() {
        // 8 entries: 2 columns at 60 wide → 4 rows + title + border.
        assert_eq!(menu_height(&[], 60), 1 + 4 + 2);
        assert_eq!(menu_height(&[], 200), 1 + 2 + 2);
    }
}
