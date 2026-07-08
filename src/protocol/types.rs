use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub id: String,
    pub success: bool,
    pub message: String,
    pub cursor: CursorPosition,
    pub modified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

impl CursorPosition {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub id: String,
    pub query: QueryType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    Cursor,
    Content,
    Modified,
    Mode,
    FileName,
    Status,
    Registers,
    UndoHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryResponse {
    Cursor {
        id: String,
        position: CursorPosition,
    },
    Content {
        id: String,
        content: String,
    },
    Modified {
        id: String,
        modified: bool,
    },
    Mode {
        id: String,
        mode: String,
    },
    FileName {
        id: String,
        filename: String,
    },
    Status {
        id: String,
        cursor: CursorPosition,
        modified: bool,
        mode: String,
        filename: String,
        line_count: usize,
    },
    Registers {
        id: String,
        registers: std::collections::HashMap<char, String>,
    },
    UndoHistory {
        id: String,
        undo_count: usize,
        redo_count: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Command(CommandRequest),
    Query(QueryRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    CommandResponse(CommandResponse),
    QueryResponse(QueryResponse),
    Error {
        id: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    ConnectionFailed,
    InvalidCommand,
    EditorError,
    FileNotFound,
    PermissionDenied,
    InvalidQuery,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::ConnectionFailed => "CONNECTION_FAILED",
            ErrorCode::InvalidCommand => "INVALID_COMMAND",
            ErrorCode::EditorError => "EDITOR_ERROR",
            ErrorCode::FileNotFound => "FILE_NOT_FOUND",
            ErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ErrorCode::InvalidQuery => "INVALID_QUERY",
        }
    }
}
