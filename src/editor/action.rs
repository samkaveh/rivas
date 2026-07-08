#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionType {
    Inclusive,
    Exclusive,
    Line,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Motion {
    CharLeft,
    CharRight,
    LineUp,
    LineDown,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    LineFirstNonBlank,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    ParagraphForward,
    ParagraphBackward,
    FindForward { target: char, before: bool },
    FindBackward { target: char, before: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    // Movement
    MoveCursor(Motion),

    // Insert Mode Edits
    InsertChar(char),
    InsertNewline,
    DeleteCharBack,
    DeleteCharForward,

    // Operators
    ExecuteOperator {
        op: char,
        motion: Motion,
        motion_type: MotionType,
    },
    LineWiseOperator(char),

    // Normal Mode Edits
    ReplaceChar(char),
    Paste { after: bool },
    JoinLines,
    ToggleCase,

    // History
    Undo,
    Redo,

    // Modes & Commands
    EnterMode(Mode),
    ExecuteCommand(String),
    Search { forward: bool },
    RepeatSearch { reverse: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    Command,
    Search { forward: bool },
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::Command => "COMMAND",
            Mode::Search { forward: true } => "SEARCH↓",
            Mode::Search { forward: false } => "SEARCH↑",
        }
    }
}
