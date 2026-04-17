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

pub struct Document {
    pub blocks: Vec<Block>,
}
