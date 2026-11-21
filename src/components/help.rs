use crate::theme;
use iocraft::prelude::*;

/// One group of shortcuts in the help overlay.
struct Section {
    title: &'static str,
    keys: &'static [(&'static str, &'static str)],
}

// Descriptions stay ≤ 23 columns so two columns fit an 80×24 terminal.
const SECTIONS: &[Section] = &[
    Section {
        title: "Move",
        keys: &[
            ("j k", "line down / up"),
            ("w b e", "word fwd / back / end"),
            ("{ }", "prev / next paragraph"),
            ("Ctrl-d/u", "half page down / up"),
            ("gg G", "top / bottom"),
            ("/ ?  n N", "search, next / prev"),
        ],
    },
    Section {
        title: "Edit",
        keys: &[
            ("i a", "insert before / after"),
            ("o O", "new line below / above"),
            ("Esc", "back to Normal mode"),
            ("v", "visual selection"),
            ("dd yy p", "delete / yank / paste"),
            ("u Ctrl-r", "undo / redo"),
        ],
    },
    Section {
        title: "Files",
        keys: &[
            ("\\", "toggle file tree"),
            ("Ctrl-p", "find file"),
            (":w", "save"),
            (":q  :wq", "quit / save and quit"),
        ],
    },
    Section {
        title: "View",
        keys: &[
            ("Ctrl-t", "next color theme"),
            ("[ ]", "shrink / grow images"),
            (":math", "toggle math mode"),
            (":serve", "open in the browser"),
        ],
    },
];

/// Properties for the [`HelpOverlay`] component.
#[derive(Default, Props)]
pub struct HelpOverlayProps {
    /// Full width of the area the overlay is centered in.
    pub width: u32,
    /// Full height of the area the overlay is centered in.
    pub height: u32,
}

/// Keyboard reference shown over the document (F1 or `:help`).
///
/// Two columns of grouped shortcuts on a raised panel; collapses to a single
/// column on narrow terminals.
#[component]
pub fn HelpOverlay(props: &HelpOverlayProps) -> impl Into<AnyElement<'static>> {
    let two_col = props.width >= 78;
    let panel_w = if two_col { 76 } else { props.width.saturating_sub(4).min(42) };
    let key_w = 10u32;
    let section_w = if two_col { 34 } else { panel_w.saturating_sub(6) };
    let desc_w = section_w.saturating_sub(key_w);
    // Exact panel height (content is static): title + gap, then each column's
    // sections (title + rows + gap), plus top padding and the border.
    let rows_of = |secs: &[Section]| secs.iter().map(|s| s.keys.len() as u32 + 2).sum::<u32>();
    let body_rows = if two_col {
        rows_of(&SECTIONS[..2]).max(rows_of(&SECTIONS[2..]))
    } else {
        rows_of(SECTIONS)
    };
    let panel_h = (2 + body_rows + 1 + 2).min(props.height);

    let section = |sec: &Section| {
        element! {
            View(flex_direction: FlexDirection::Column, margin_bottom: 1, width: section_w) {
                Text(content: sec.title, color: theme::blue(), weight: Weight::Bold, wrap: TextWrap::NoWrap)
                #(sec.keys.iter().map(|(k, d)| element! {
                    // Fixed-size, non-wrapping cells: free-flowing text here
                    // gets measured at narrow widths during layout and
                    // inflates the panel height.
                    View(flex_direction: FlexDirection::Row, height: 1) {
                        View(width: key_w, flex_shrink: 0.0) {
                            Text(content: *k, color: theme::fg(), weight: Weight::Bold, wrap: TextWrap::NoWrap)
                        }
                        View(width: desc_w, flex_shrink: 0.0) {
                            Text(content: *d, color: theme::muted(), wrap: TextWrap::NoWrap)
                        }
                    }
                }.into_any()))
            }
        }
        .into_any()
    };

    let columns: Vec<AnyElement<'static>> = if two_col {
        vec![
            element! {
                View(flex_direction: FlexDirection::Column, margin_right: 2) {
                    #(SECTIONS[..2].iter().map(section))
                }
            }
            .into_any(),
            element! {
                View(flex_direction: FlexDirection::Column) {
                    #(SECTIONS[2..].iter().map(section))
                }
            }
            .into_any(),
        ]
    } else {
        vec![element! {
            View(flex_direction: FlexDirection::Column) {
                #(SECTIONS.iter().map(section))
            }
        }
        .into_any()]
    };

    element! {
        View(
            position: Position::Absolute,
            width: props.width,
            height: props.height,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        ) {
            View(
                width: panel_w,
                height: panel_h,
                overflow: Overflow::Hidden,
                flex_direction: FlexDirection::Column,
                background_color: theme::status_bg(),
                border_style: BorderStyle::Round,
                border_color: theme::blue(),
                padding_left: 2,
                padding_right: 2,
                padding_top: 1,
            ) {
                View(flex_direction: FlexDirection::Row, margin_bottom: 1) {
                    Text(content: "Keyboard shortcuts", color: theme::fg(), weight: Weight::Bold)
                    View(flex_grow: 1.0) {}
                    Text(content: "Esc", color: theme::fg(), weight: Weight::Bold)
                    Text(content: " close", color: theme::comment())
                }
                View(flex_direction: FlexDirection::Row) {
                    #(columns)
                }
            }
        }
    }
}
