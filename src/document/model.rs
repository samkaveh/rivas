/// Block-level elements
#[derive(Debug, Clone)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
        id: String,
    },
    Paragraph {
        content: Vec<Inline>,
    },
    Code {
        language: Option<String>,
        code: String,
    },
    Mermaid {
        source: String,
    },
    Math {
        content: String,
        display: bool,
    },
    Quote {
        children: Vec<Block>,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Table {
        headers: Vec<TableCell>,
        alignments: Vec<Alignment>,
        rows: Vec<Vec<TableCell>>,
    },
    ThematicBreak,
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
    Html {
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub content: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct TableCell {
    pub content: Vec<Inline>,
}

#[derive(Debug, Clone, Copy)]
pub enum Alignment {
    Left,
    Center,
    Right,
    None,
}

/// Inline elements
#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Math(String),
    Link {
        text: Vec<Inline>,
        url: String,
        title: Option<String>,
    },
    Image {
        alt: String,
        url: String,
    },
    SoftBreak,
    HardBreak,
}

#[derive(Clone)]
pub struct Document {
    pub blocks: Vec<Block>,
}

/// Flatten a tree of Inlines into a single plain-text string.
/// Used by both the parser (for alt-text / slugs) and the renderer (for link labels / table cells).
pub fn inlines_to_text(inlines: &[Inline]) -> String {
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
