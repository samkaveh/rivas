use iocraft::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
#[cfg(test)]
use syntect::highlighting::ThemeSet;
use syntect::{easy::HighlightLines, parsing::SyntaxSet};

/// Bundled syntect syntax set (loaded once).
pub static SS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_nonewlines);
/// Bundled syntect theme set (loaded once; used by benchmarks in tests).
#[cfg(test)]
pub static TS: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Converts an app palette color into a syntect color.
fn scol(c: Color) -> syntect::highlighting::Color {
    match c {
        Color::Rgb { r, g, b } => syntect::highlighting::Color { r, g, b, a: 0xff },
        _ => syntect::highlighting::Color::WHITE,
    }
}

/// Builds a syntect theme from the active app palette, so code blocks and the
/// Markdown source view use the same colors as the rest of the interface
/// (keywords, strings, numbers… all follow Tokyo Night, Nord, Gruvbox, …).
fn palette_theme(p: &crate::theme::Theme) -> syntect::highlighting::Theme {
    use std::str::FromStr;
    use syntect::highlighting::{
        FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
    };
    let rule = |scopes: &str, color: Color, font: Option<FontStyle>| ThemeItem {
        scope: ScopeSelectors::from_str(scopes).expect("valid scope selector"),
        style: StyleModifier {
            foreground: Some(scol(color)),
            background: None,
            font_style: font,
        },
    };
    let italic = Some(FontStyle::ITALIC);
    let bold = Some(FontStyle::BOLD);
    Theme {
        name: Some(p.name.to_string()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(scol(p.fg)),
            background: Some(scol(p.dark_bg)),
            ..ThemeSettings::default()
        },
        scopes: vec![
            // ── Code ──────────────────────────────────────────────────────
            rule("comment, punctuation.definition.comment", p.comment, italic),
            rule("string, string.quoted, punctuation.definition.string", p.green, None),
            rule("constant.character.escape, string.regexp", p.cyan, None),
            rule("constant.numeric, constant.language, constant.character, constant.other", p.orange, None),
            rule("keyword, storage, storage.modifier", p.magenta, None),
            rule("keyword.operator, punctuation.accessor", p.cyan, None),
            rule("storage.type, entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, support.type, support.class", p.teal, None),
            rule("entity.name.function, support.function, meta.function-call entity.name, variable.function", p.blue, None),
            rule("variable.parameter, meta.parameter", p.yellow, None),
            rule("entity.name.tag", p.red, None),
            rule("entity.other.attribute-name, support.type.property-name, meta.object-literal.key, variable.other.member", p.teal, None),
            rule("entity.name.namespace, entity.name.module, support.module", p.cyan, None),
            rule("meta.attribute, meta.annotation, entity.name.label", p.yellow, None),
            rule("invalid", p.red, None),
            // ── Markdown source ───────────────────────────────────────────
            rule("markup.heading, entity.name.section", p.blue, bold),
            rule("punctuation.definition.heading", p.comment, bold),
            rule("markup.bold", p.fg, bold),
            rule("markup.italic", p.fg, italic),
            rule("markup.raw, markup.inline.raw, markup.raw.inline", p.green, None),
            rule("markup.underline.link, meta.link.inline.description, string.other.link", p.blue, None),
            rule("meta.link.inline, punctuation.definition.link, punctuation.definition.metadata", p.comment, None),
            rule("markup.quote, punctuation.definition.blockquote", p.comment, italic),
            rule("punctuation.definition.list, punctuation.definition.list_item, markup.list.numbered punctuation, beginning.punctuation", p.blue, None),
            rule("punctuation.definition.bold, punctuation.definition.italic, punctuation.definition.raw, punctuation.definition.strikethrough", p.comment, None),
            rule("meta.separator, markup.hr", p.dark_grey, None),
            rule("markup.deleted", p.red, None),
            rule("markup.inserted", p.green, None),
        ],
    }
}

/// The syntax theme for the active app theme (built once per palette).
fn highlight_theme() -> std::sync::Arc<syntect::highlighting::Theme> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    static CACHE: LazyLock<Mutex<HashMap<usize, Arc<syntect::highlighting::Theme>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let idx = crate::theme::index();
    let mut cache = CACHE.lock().unwrap();
    cache
        .entry(idx)
        .or_insert_with(|| Arc::new(palette_theme(crate::theme::current())))
        .clone()
}

/// Highlights each line of `text` under the given syntax token and returns
/// byte-aligned `(text, color)` spans (one vec per logical line).
///
/// Falls back to plain text when the token is unknown and to `fallback` when a
/// line fails to highlight.
pub fn highlight_source(text: &str, token: &str, fallback: Color) -> Vec<Vec<(String, Color)>> {
    let syntax = SS
        .find_syntax_by_token(token)
        .unwrap_or_else(|| SS.find_syntax_plain_text());
    let theme = highlight_theme();
    let mut highlighter = HighlightLines::new(syntax, &theme);

    let mut lines = Vec::new();
    for line in text.lines() {
        match highlighter.highlight_line(line, &SS) {
            Ok(regions) => {
                lines.push(
                    regions
                        .iter()
                        .map(|(style, t)| {
                            let color = Color::Rgb {
                                r: style.foreground.r,
                                g: style.foreground.g,
                                b: style.foreground.b,
                            };
                            (t.to_string(), color)
                        })
                        .collect(),
                );
            }
            Err(_) => {
                lines.push(vec![(line.to_string(), fallback)]);
            }
        }
    }
    lines
}

/// A component hook that caches per-line syntax-highlighted spans, recomputing
/// only when the `(token, text)` pair changes. Call it unconditionally; pass an
/// empty source (or a plain token) when highlighting is not wanted this frame.
pub trait UseHighlight {
    /// Returns syntax-highlighted per-line spans for `text`.
    fn use_cached_highlight(
        &mut self,
        text: &str,
        token: &str,
        fallback: Color,
    ) -> Vec<Vec<(String, Color)>>;
}

impl UseHighlight for Hooks<'_, '_> {
    fn use_cached_highlight(
        &mut self,
        text: &str,
        token: &str,
        fallback: Color,
    ) -> Vec<Vec<(String, Color)>> {
        let new_hash = {
            let mut hasher = DefaultHasher::new();
            token.hash(&mut hasher);
            text.hash(&mut hasher);
            // Recompute when the theme changes (highlight colors follow it).
            crate::theme::index().hash(&mut hasher);
            hasher.finish()
        };

        let mut prev_hash = self.use_ref(|| 0u64);
        let mut cache = self.use_ref(|| Vec::<Vec<(String, Color)>>::new());
        if *prev_hash.read() != new_hash {
            prev_hash.set(new_hash);
            cache.set(highlight_source(text, token, fallback));
        }
        cache.read().clone()
    }
}

/// Convenience for components that only need the base foreground fallback.
pub const MARKDOWN: &str = "markdown";
pub const LATEX: &str = "latex";
pub const HTML: &str = "html";
