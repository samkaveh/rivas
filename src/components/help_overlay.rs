use iocraft::prelude::*;

use crate::theme;

#[derive(Default, Props)]
pub struct HelpOverlayProps {}

#[component]
pub fn HelpOverlay(_props: &HelpOverlayProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();

    element! {
        View(
            width,
            height,
            flex_direction: FlexDirection::Column,
            background_color: Color::Rgb { r: 0, g: 0, b: 0 },
            padding_left: 2,
            padding_right: 2,
            padding_top: 1,
            padding_bottom: 1,
        ) {
            View(
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                border_style: BorderStyle::Round,
                border_color: theme::BLUE,
                background_color: theme::DARK_BG,
                padding_left: 1,
                padding_right: 1,
                padding_top: 1,
                padding_bottom: 1,
            ) {
                View(width: 100pct, justify_content: JustifyContent::Center) {
                    Text(content: " Rivas Help ", color: theme::YELLOW, weight: Weight::Bold)
                }
                View(width: 100pct, height: 1, justify_content: JustifyContent::Center) {
                    Text(content: "Press F1 or Esc to close  |  arrows to scroll", color: theme::COMMENT)
                }
                View(width: 100pct, height: 1) {}

                ScrollView(
                    keyboard_scroll: Some(true),
                    scrollbar_thumb_color: Some(theme::FG),
                    scrollbar_track_color: Some(theme::DARK_BG),
                ) {
                    View(flex_direction: FlexDirection::Row) {
                        View(flex_grow: 1.0, flex_direction: FlexDirection::Column, padding_right: 2) {
                            Text(content: "NAVIGATION", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  j/k        Scroll down/up", color: theme::FG)
                            Text(content: "  gg/G       Jump top/bottom", color: theme::FG)
                            Text(content: "  Ctrl+d/u   Half page down/up", color: theme::FG)
                            Text(content: "  Ctrl+f/b   Full page down/up", color: theme::FG)
                            Text(content: "  h/l        Move left/right", color: theme::FG)
                            Text(content: "  w/b/e      Word fwd/bwd/end", color: theme::FG)
                            Text(content: "  0/^/$      Line start/^/end", color: theme::FG)
                            Text(content: "  {/}        Paragraph jump", color: theme::FG)
                            Text(content: "  f/t/F/T    Find char", color: theme::FG)
                            Text(content: "  ;/,        Repeat find", color: theme::FG)
                            Text(content: "  PgUp/PgDn  Page up/down", color: theme::FG)
                            Text(content: "  Home/End   Top/bottom", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "EDITING", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  i/I        Insert / Insert at start", color: theme::FG)
                            Text(content: "  a/A        Append / Append at end", color: theme::FG)
                            Text(content: "  o/O        Open line below/above", color: theme::FG)
                            Text(content: "  s          Substitute character", color: theme::FG)
                            Text(content: "  r          Replace character", color: theme::FG)
                            Text(content: "  x/X        Delete char / Backspace", color: theme::FG)
                            Text(content: "  J          Join lines", color: theme::FG)
                            Text(content: "  ~          Toggle case", color: theme::FG)
                            Text(content: "  >>/<<      Indent / Dedent", color: theme::FG)
                        }

                        View(flex_grow: 1.0, flex_direction: FlexDirection::Column) {
                            Text(content: "OPERATORS + MOTIONS", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  d{motion}  Delete with motion", color: theme::FG)
                            Text(content: "  c{motion}  Change with motion", color: theme::FG)
                            Text(content: "  y{motion}  Yank with motion", color: theme::FG)
                            Text(content: "  dd/cc/yy   Delete/change/yank line", color: theme::FG)
                            Text(content: "  >/<        Indent/dedent", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "VISUAL MODE", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  v          Enter visual mode", color: theme::FG)
                            Text(content: "  d/x        Delete selection", color: theme::FG)
                            Text(content: "  y          Yank selection", color: theme::FG)
                            Text(content: "  c          Change selection", color: theme::FG)
                            Text(content: "  Esc/v      Exit visual mode", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "SEARCH", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  /          Search forward", color: theme::FG)
                            Text(content: "  ?          Search backward", color: theme::FG)
                            Text(content: "  n/N        Next/prev match", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "CLIPBOARD", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  p/P        Paste after/before", color: theme::FG)
                            Text(content: "  y          Yank (copy)", color: theme::FG)
                            Text(content: "  u/Ctrl+r   Undo / Redo", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "COMMAND MODE (:)", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  :w         Write file", color: theme::FG)
                            Text(content: "  :q         Quit", color: theme::FG)
                            Text(content: "  :q!        Force quit", color: theme::FG)
                            Text(content: "  :wq/:x     Save and quit", color: theme::FG)
                            Text(content: "  :N         Jump to line N", color: theme::FG)
                            View(width: 100pct, height: 1) {}

                            Text(content: "GENERAL", color: theme::YELLOW, weight: Weight::Bold)
                            Text(content: "  Ctrl+p     Find file", color: theme::FG)
                            Text(content: "  F1         Toggle this help", color: theme::CYAN)
                            Text(content: "  :q         Quit", color: theme::FG)
                            Text(content: "  ZZ/ZQ      Save+quit / Quit", color: theme::FG)
                        }
                    }
                }
            }
        }
    }
}
