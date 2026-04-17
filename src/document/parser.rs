use super::model::*;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn parse_markdown(source: &str) -> Document {
    let options = Options::ENABLE_MATH
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(source, options);
    let events: Vec<Event> = parser.collect();
    let mut pos = 0;
    let blocks = parse_blocks(&events, &mut pos);
    Document { blocks }
}

/// Parse events into blocks. Advances `pos` past consumed events.
/// Returns when it hits and `End` event (parent consider closing) or EOF.
fn parse_blocks(events: &[Event], pos: &mut usize) -> Vec<Block> {
    let mut blocks = Vec::new();
    while *pos < events.len() {
        match &events[*pos] {
            Event::Start(tag) => {
                let tag = tag.clone();
                *pos += 1;
                if let Some(block) = parse_block_tag(&tag, events, pos) {
                    blocks.push(block);
                }
            }
            Event::End(_) => {
                *pos += 1;
                return blocks;
            }
            Event::Rule => {
                blocks.push(Block::ThematicBreak);
                *pos += 1;
            }
            Event::DisplayMath(m) => {
                blocks.push(Block::Math {
                    content: m.to_string(),
                    display: true,
                });
                *pos += 1;
            }
            Event::Html(h) => {
                blocks.push(Block::Html {
                    content: h.to_string(),
                });
                *pos += 1;
            }
            Event::Text(t) => {
                blocks.push(Block::Paragraph {
                    content: vec![Inline::Text(t.to_string())],
                });
                *pos += 1;
            }
            _ => {
                *pos += 1;
            }
        }
    }
    blocks
}

fn parse_block_tag(tag: &Tag, events: &[Event], pos: &mut usize) -> Option<Block> {
    match tag {
        Tag::Heading { level, id, .. } => {
            let inlines = parse_inlines(events, pos);
            let id_str = id
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| slugify(&inlines));
            Some(Block::Heading {
                level: heading_to_u8(*level),
                content: inlines,
                id: id_str,
            })
        }
        Tag::Paragraph => {
            let inlines = parse_inlines(events, pos);
            // Promote single-image paragraphs to block images
            if inlines.len() == 1 {
                if let Inline::Image { alt, url } = &inlines[0] {
                    return Some(Block::Image {
                        alt: alt.clone(),
                        url: url.clone(),
                        title: None,
                    });
                }
                if let Inline::Math(m) = &inlines[0] {
                    return Some(Block::Math {
                        content: m.to_string(),
                        display: true,
                    });
                }
            }
            Some(Block::Paragraph { content: inlines })
        }
        Tag::CodeBlock(kind) => {
            let lang = match kind {
                CodeBlockKind::Fenced(l) if !l.is_empty() => Some(l.to_string()),
                _ => None,
            };
            let code = collect_text(events, pos);
            if lang.as_deref() == Some("mermaid") {
                return Some(Block::Mermaid { source: code });
            }
            if lang.as_deref() == Some("math") {
                return Some(Block::Math {
                    content: code,
                    display: true,
                });
            }
            Some(Block::Code {
                language: lang,
                code,
            })
        }
        Tag::BlockQuote(_) => {
            let children = parse_blocks(events, pos);
            Some(Block::Quote { children })
        }
        Tag::List(start) => {
            let ordered = start.is_some();
            let mut items = Vec::new();
            while *pos < events.len() {
                match &events[*pos] {
                    Event::Start(Tag::Item) => {
                        *pos += 1;
                        let mut checked = None;
                        if let Some(Event::TaskListMarker(c)) = events.get(*pos) {
                            checked = Some(*c);
                            *pos += 1;
                        }
                        let content = parse_blocks(events, pos);
                        items.push(ListItem { checked, content });
                    }
                    Event::End(TagEnd::List(_)) => {
                        *pos += 1;
                        break;
                    }
                    _ => {
                        *pos += 1;
                    }
                }
            }
            Some(Block::List {
                ordered,
                start: *start,
                items,
            })
        }
        Tag::Table(aligns) => {
            let alignments: Vec<Alignment> = aligns
                .iter()
                .map(|a| match a {
                    pulldown_cmark::Alignment::Left => Alignment::Left,
                    pulldown_cmark::Alignment::Right => Alignment::Right,
                    pulldown_cmark::Alignment::Center => Alignment::Center,
                    pulldown_cmark::Alignment::None => Alignment::None,
                })
                .collect();
            let mut headers = Vec::new();
            let mut rows: Vec<Vec<TableCell>> = Vec::new();
            let mut in_head = false;
            while *pos < events.len() {
                match &events[*pos] {
                    Event::Start(Tag::TableHead) => {
                        in_head = true;
                        *pos += 1;
                    }
                    Event::End(TagEnd::TableHead) => {
                        in_head = false;
                        *pos += 1;
                    }
                    Event::Start(Tag::TableRow) => {
                        if !in_head {
                            rows.push(Vec::new());
                        }
                        *pos += 1;
                    }
                    Event::End(TagEnd::TableRow) => {
                        *pos += 1;
                    }
                    Event::Start(Tag::TableCell) => {
                        *pos += 1;
                        let inlines = parse_inlines(events, pos);
                        let cell = TableCell { content: inlines };
                        if in_head {
                            headers.push(cell);
                        } else if let Some(row) = rows.last_mut() {
                            row.push(cell);
                        }
                    }
                    Event::End(TagEnd::Table) => {
                        *pos += 1;
                        break;
                    }
                    _ => {
                        *pos += 1;
                    }
                }
            }
            Some(Block::Table {
                headers,
                alignments,
                rows,
            })
        }
        _ => {
            skip_to_end(events, pos);
            None
        }
    }
}

/// Parse inline events until matching the End tag
fn parse_inlines(events: &[Event], pos: &mut usize) -> Vec<Inline> {
    let mut inlines = Vec::new();
    while *pos < events.len() {
        match &events[*pos] {
            Event::Text(t) => {
                inlines.push(Inline::Text(t.to_string()));
                *pos += 1;
            }
            Event::Code(c) => {
                inlines.push(Inline::Code(c.to_string()));
                *pos += 1;
            }
            Event::InlineMath(m) => {
                inlines.push(Inline::Math(m.to_string()));
                *pos += 1;
            }
            Event::DisplayMath(m) => {
                inlines.push(Inline::Math(m.to_string()));
                *pos += 1;
            }
            Event::SoftBreak => {
                inlines.push(Inline::SoftBreak);
                *pos += 1;
            }
            Event::HardBreak => {
                inlines.push(Inline::HardBreak);
                *pos += 1;
            }
            Event::Start(Tag::Strong) => {
                *pos += 1;
                let children = parse_inlines(events, pos);
                inlines.push(Inline::Bold(children));
            }
            Event::Start(Tag::Emphasis) => {
                *pos += 1;
                let children = parse_inlines(events, pos);
                inlines.push(Inline::Italic(children));
            }
            Event::Start(Tag::Strikethrough) => {
                *pos += 1;
                let children = parse_inlines(events, pos);
                inlines.push(Inline::Strikethrough(children));
            }
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                let url = dest_url.to_string();
                let title = if title.is_empty() {
                    None
                } else {
                    Some(title.to_string())
                };
                *pos += 1;
                let children = parse_inlines(events, pos);
                inlines.push(Inline::Link {
                    text: children,
                    url,
                    title,
                });
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let url = dest_url.to_string();
                *pos += 1;
                let children = parse_inlines(events, pos);
                let alt = inlines_to_text(&children);
                inlines.push(Inline::Image { alt, url });
            }
            Event::End(_) => {
                *pos += 1;
                return inlines;
            }
            _ => {
                *pos += 1;
            }
        }
    }
    inlines
}

/// Collect raw text from events until the matching End tag.
fn collect_text(events: &[Event], pos: &mut usize) -> String {
    let mut s = String::new();
    while *pos < events.len() {
        match &events[*pos] {
            Event::Text(t) => {
                s.push_str(t.as_ref());
                *pos += 1;
            }
            Event::End(_) => {
                *pos += 1;
                return s;
            }
            _ => {
                *pos += 1;
            }
        }
    }
    s
}

fn skip_to_end(events: &[Event], pos: &mut usize) {
    let mut depth = 1u32;
    while *pos < events.len() && depth > 0 {
        match &events[*pos] {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
        *pos += 1;
    }
}

fn inlines_to_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) => s.push_str(t),
            Inline::Code(c) | Inline::Math(c) => s.push_str(c),
            Inline::Bold(ch) | Inline::Italic(ch) | Inline::Strikethrough(ch) => {
                s.push_str(&inlines_to_text(ch))
            }
            Inline::Link { text, .. } => s.push_str(&inlines_to_text(text)),
            Inline::SoftBreak => s.push(' '),
            Inline::HardBreak => s.push('\n'),
            _ => {}
        }
    }
    s
}

fn heading_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn slugify(inlines: &[Inline]) -> String {
    inlines_to_text(inlines)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading() {
        let doc = parse_markdown("# Title");
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn code_block() {
        let doc = parse_markdown("```python\ndef main():\n\t\t\t\tprint('Hello')\n```");
        match &doc.blocks[0] {
            Block::Code { language, code } => {
                assert_eq!(language.as_deref(), Some("python"));
                assert!(code.contains("def main()"))
            }
            _ => panic!("Expected a Code Block!"),
        }
    }

    #[test]
    fn mermaid_detection() {
        let doc = parse_markdown("```mermaid\ngraph LR\n A-->B\n```");
        assert!(matches!(&doc.blocks[0], Block::Mermaid { .. }));
    }

    #[test]
    fn display_math() {
        let doc = parse_markdown(
            r#"
$$
e^{i\pi} + 1 = 0
$$
        "#,
        );
        assert!(matches!(&doc.blocks[0], Block::Math { display: true, .. }));
    }

    #[test]
    fn inline_math() {
        let doc = parse_markdown("The equation $x^2$ is famous.");
        if let Block::Paragraph { content } = &doc.blocks[0] {
            assert!(content.iter().any(|i| matches!(i, Inline::Math(_))));
        } else {
            panic!("Paragraph is expected!")
        }
    }

    #[test]
    fn image() {
        let doc = parse_markdown("![alt](img.png)");
        assert!(matches!(&doc.blocks[0], Block::Image { .. }));
    }

    #[test]
    fn nested_list() {
        let doc = parse_markdown("- [x] A\n - [ ] B\n - C\n- D");
        assert!(matches!(&doc.blocks[0], Block::List { .. }));
    }

    #[test]
    fn table() {
        let doc = parse_markdown("| A | B |\n|---|---|\n| 1 | 2 |");
        assert!(matches!(&doc.blocks[0], Block::Table { .. }));
    }
}
