use crate::components::editor::Mode;
use iocraft::prelude::*;

#[derive(Props)]
pub struct LineRendererProps {
    pub line: String,
    pub line_start_off: usize,
    pub h_scroll: usize,
    pub effective_width: usize,
    pub mode: Mode,
    pub vis_start: Option<usize>,
    pub vis_end: Option<usize>,
    pub cursor_line_idx: Option<usize>,
    pub current_line_idx: usize,
    pub cursor_rel_off: usize,
    pub cursor_fg: Color,
    pub cursor_bg_final: Color,
    pub cursor_char: &'static str,
}

impl Default for LineRendererProps {
    fn default() -> Self {
        Self {
            line: String::new(),
            line_start_off: 0,
            h_scroll: 0,
            effective_width: 80,
            mode: Mode::Normal,
            vis_start: None,
            vis_end: None,
            cursor_line_idx: None,
            current_line_idx: 0,
            cursor_rel_off: 0,
            cursor_fg: crate::theme::FG,
            cursor_bg_final: crate::theme::BG,
            cursor_char: " ",
        }
    }
}

#[component]
pub fn LineRenderer(props: &LineRendererProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let line = &props.line;
    let h_scroll = props.h_scroll;
    let effective_width = props.effective_width;
    let mode = props.mode.clone();
    let vis_start = props.vis_start;
    let vis_end = props.vis_end;
    let line_start_off = props.line_start_off;
    let cursor_line_idx = props.cursor_line_idx;
    let current_line_idx = props.current_line_idx;
    let cursor_rel_off = props.cursor_rel_off;
    let cursor_fg = props.cursor_fg;
    let cursor_bg_final = props.cursor_bg_final;
    let cursor_char = props.cursor_char;

    let line_chars: Vec<char> = line.chars().collect();
    let visible_chars: Vec<char> = line_chars
        .iter()
        .skip(h_scroll)
        .take(effective_width)
        .copied()
        .collect();
    let visible_text: String = visible_chars.iter().collect();

    if mode == Mode::Visual {
        if let (Some(start), Some(end)) = (vis_start, vis_end) {
            let mut line_parts: Vec<(bool, String)> = Vec::new();
            let mut current_char_idx = h_scroll;
            
            for &c in &visible_chars {
                let byte_off = line.char_indices().nth(current_char_idx).map(|(i, _)| i).unwrap_or(line.len());
                let abs_off = line_start_off + byte_off;
                let is_selected = abs_off >= start && abs_off < end;
                
                if let Some(last) = line_parts.last_mut() {
                    if last.0 == is_selected {
                        last.1.push(c);
                        current_char_idx += 1;
                        continue;
                    }
                }
                line_parts.push((is_selected, c.to_string()));
                current_char_idx += 1;
            }
            return element! {
                View(flex_direction: FlexDirection::Row) {
                    #(line_parts.iter().map(|(selected, text)| element! {
                        Text(content: text.clone(), color: if *selected { crate::theme::MAGENTA } else { crate::theme::FG }, wrap: TextWrap::NoWrap)
                    }))
                }
            }.into_any();
        } else {
            return element! { Text(content: visible_text, color: crate::theme::FG, wrap: TextWrap::NoWrap) }.into_any();
        }
    } else if Some(current_line_idx) == cursor_line_idx {
        let char_col = line.char_indices().take_while(|&(i, _)| i < cursor_rel_off).count();
        
        if char_col >= h_scroll && char_col < h_scroll + visible_chars.len() {
            let visible_col = char_col - h_scroll;
            let visible_chars_vec: Vec<char> = visible_text.chars().collect();
            let before: String = visible_chars_vec[..visible_col].iter().collect();
            let after_with_char = &visible_chars_vec[visible_col..];
            
            if let Some(&c) = after_with_char.first() {
                let after: String = after_with_char[1..].iter().collect();
                return element! {
                    View(flex_direction: FlexDirection::Row) {
                        Text(content: before, color: crate::theme::FG, wrap: TextWrap::NoWrap)
                        View(background_color: cursor_bg_final, width: 1) {
                            Text(content: c.to_string(), color: cursor_fg, wrap: TextWrap::NoWrap)
                        }
                        Text(content: after, color: crate::theme::FG, wrap: TextWrap::NoWrap)
                    }
                }.into_any();
            } else {
                return element! {
                    View(flex_direction: FlexDirection::Row) {
                        Text(content: before, color: crate::theme::FG, wrap: TextWrap::NoWrap)
                        View(background_color: cursor_bg_final, width: 1) {
                            Text(content: cursor_char, color: cursor_fg, wrap: TextWrap::NoWrap)
                        }
                        Text(content: "", color: crate::theme::FG, wrap: TextWrap::NoWrap)
                    }
                }.into_any();
            }
        } else {
            return element! { Text(content: visible_text, color: crate::theme::FG, wrap: TextWrap::NoWrap) }.into_any();
        }
    } else {
        return element! { Text(content: visible_text, color: crate::theme::FG, wrap: TextWrap::NoWrap) }.into_any();
    }
}
