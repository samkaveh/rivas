use iocraft::prelude::*;
use std::sync::LazyLock;
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

static SS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_nonewlines);
static TS: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Default, Props)]
pub struct CodeBlockProps {
    pub language: Option<String>,
    pub code: String,
}

#[component]
pub fn CodeBlock(props: &CodeBlockProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let lang_label = props.language.clone().unwrap_or_else(|| "code".to_string());
    let code = props.code.clone();
    let syntax = props
        .language
        .as_deref()
        .and_then(|l| SS.find_syntax_by_token(l))
        .unwrap_or_else(|| SS.find_syntax_plain_text());
    let theme = &TS.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut highlighted_lines = Vec::new();

    for line in code.lines() {
        match highlighter.highlight_line(line, &SS) {
            Ok(regions) => {
                let line_spans: Vec<(String, Color)> = regions
                    .iter()
                    .map(|(style, text)| {
                        let r = style.foreground.r;
                        let g = style.foreground.g;
                        let b = style.foreground.b;
                        let color = Color::Rgb { r, g, b };
                        (text.to_string(), color)
                    })
                    .collect();
                highlighted_lines.push(line_spans);
            }
            Err(_) => {
                highlighted_lines.push(vec![(line.to_string(), Color::White)]);
            }
        }
    }

    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, margin_bottom: 1, background_color: Color::Rgb{r: 26, g: 27, b: 30}) {
            View(margin_bottom: 1) {
                Text(content: lang_label, color: Color::Blue)
            }
            View(flex_direction: FlexDirection::Column) {
                #(highlighted_lines.iter().map(|line_spans| {
                    element! {
                        View(flex_direction: FlexDirection::Row) {
                            #(line_spans.iter().map(|(text, color)| {
                                element! { Text(content: text.clone(), color: *color) }.into_any()
                            }))
                        }
                    }
                    .into_any()
                }))
            }
        }
    }
}
