use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use rivas::protocol::{ClientMessage, CommandRequest, ErrorCode, QueryRequest, QueryType, ServerMessage, SocketClient};

#[derive(Parser)]
#[command(
    name = "rivas-cli",
    about = "CLI client for interacting with Rivas editor"
)]
struct Cli {
    /// Path to the Unix socket
    #[arg(short, long)]
    socket: Option<PathBuf>,

    /// Output format
    #[arg(short, long, default_value = "text", value_parser = ["text", "json"])]
    format: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a vim command
    Exec {
        /// Vim command to execute
        command: String,
    },
    /// Execute multiple vim commands
    ExecMany {
        /// Vim commands to execute (space-separated)
        commands: Vec<String>,
    },
    /// Query editor state
    Query {
        /// What to query: cursor, content, modified, mode, filename, status, registers, undo
        #[arg(short, long)]
        what: String,
    },
    /// Execute command and return JSON result
    Run {
        /// Vim command to execute
        command: String,
    },
    /// Show connection status
    Status,
}

fn find_socket_path() -> Option<PathBuf> {
    // Check common socket paths
    let patterns = [
        "/tmp/rivas-*.sock",
        "/tmp/rivas.sock",
    ];
    
    for pattern in &patterns {
        if let Ok(entries) = glob::glob(pattern) {
            for entry in entries.flatten() {
                if entry.exists() {
                    return Some(entry);
                }
            }
        }
    }
    
    None
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Find socket path
    let socket_path = match cli.socket {
        Some(path) => path,
        None => {
            // Try to find socket automatically
            find_socket_path()
                .context("No socket path specified and no Rivas socket found. Start Rivas with --watch flag.")?
        }
    };

    if cli.verbose {
        eprintln!("Connecting to: {}", socket_path.display());
    }

    let mut client = match SocketClient::connect(&socket_path) {
        Ok(client) => client,
        Err(e) => {
            let error_msg = format!("Failed to connect to Rivas at {}", socket_path.display());
            let detail = match e.kind() {
                std::io::ErrorKind::NotFound => {
                    format!("Socket not found. Is Rivas running with --watch? Error: {}", e)
                }
                std::io::ErrorKind::ConnectionRefused => {
                    format!("Connection refused. Is Rivas running? Error: {}", e)
                }
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied. Check socket permissions. Error: {}", e)
                }
                _ => format!("Error: {}", e),
            };
            
            if cli.format == "json" {
                let error_response = serde_json::json!({
                    "error": error_msg,
                    "detail": detail,
                    "socket": socket_path.display().to_string(),
                    "error_code": ErrorCode::ConnectionFailed.as_str(),
                });
                println!("{}", serde_json::to_string_pretty(&error_response)?);
            } else {
                eprintln!("ERROR: {}", error_msg);
                eprintln!("DETAIL: {}", detail);
                eprintln!("SOCKET: {}", socket_path.display());
            }
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Status => {
            // Just test connection
            let request = CommandRequest {
                id: format!("status-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()),
                command: "0".to_string(), // Go to first line (no-op)
                args: vec![],
            };

            let msg = ClientMessage::Command(request);
            match client.send_message(&msg) {
                Ok(ServerMessage::CommandResponse(_)) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::json!({
                            "connected": true,
                            "socket": socket_path.display().to_string()
                        }));
                    } else {
                        println!("Connected to Rivas at {}", socket_path.display());
                    }
                }
                Ok(_) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::json!({
                            "connected": false,
                            "error": "Unexpected response"
                        }));
                    } else {
                        eprintln!("ERROR: Unexpected response from server");
                    }
                    std::process::exit(1);
                }
                Err(e) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::json!({
                            "connected": false,
                            "error": e.to_string()
                        }));
                    } else {
                        eprintln!("ERROR: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
        Commands::Exec { command } => {
            let request = CommandRequest {
                id: format!("cmd-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()),
                command: command.clone(),
                args: vec![],
            };

            let msg = ClientMessage::Command(request);
            match client.send_message(&msg) {
                Ok(ServerMessage::CommandResponse(resp)) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::to_string_pretty(&resp)?);
                    } else {
                        if resp.success {
                            if !resp.message.is_empty() {
                                println!("OK: {}", resp.message);
                            } else {
                                println!("OK");
                            }
                        } else {
                            println!("ERROR: {}", resp.message);
                            if let Some(code) = &resp.error_code {
                                eprintln!("ERROR_CODE: {}", code);
                            }
                        }
                        println!("CURSOR: {}:{}", resp.cursor.row, resp.cursor.col);
                        if resp.modified {
                            println!("MODIFIED: true");
                        }
                    }
                    if !resp.success {
                        std::process::exit(1);
                    }
                }
                Ok(ServerMessage::Error { message, error_code, .. }) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::json!({
                            "success": false,
                            "message": message,
                            "error_code": error_code
                        }));
                    } else {
                        eprintln!("ERROR: {}", message);
                        if let Some(code) = error_code {
                            eprintln!("ERROR_CODE: {}", code);
                        }
                    }
                    std::process::exit(1);
                }
                Ok(_) => {
                    eprintln!("ERROR: Unexpected response");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::ExecMany { commands } => {
            let mut all_success = true;
            for cmd in &commands {
                let request = CommandRequest {
                    id: format!("cmd-{}", std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()),
                    command: cmd.clone(),
                    args: vec![],
                };

                let msg = ClientMessage::Command(request);
                match client.send_message(&msg) {
                    Ok(ServerMessage::CommandResponse(resp)) => {
                        if cli.format == "json" {
                            println!("{}", serde_json::to_string_pretty(&resp)?);
                        } else {
                            if resp.success {
                                if !resp.message.is_empty() {
                                    println!("OK: {}", resp.message);
                                } else {
                                    println!("OK");
                                }
                            } else {
                                println!("ERROR: {}", resp.message);
                                all_success = false;
                            }
                            println!("CURSOR: {}:{}", resp.cursor.row, resp.cursor.col);
                        }
                    }
                    Ok(ServerMessage::Error { message, error_code, .. }) => {
                        if cli.format == "json" {
                            println!("{}", serde_json::json!({
                                "success": false,
                                "message": message,
                                "error_code": error_code
                            }));
                        } else {
                            eprintln!("ERROR: {}", message);
                        }
                        all_success = false;
                    }
                    Ok(_) => {
                        eprintln!("ERROR: Unexpected response");
                        all_success = false;
                    }
                    Err(e) => {
                        eprintln!("ERROR: {}", e);
                        all_success = false;
                    }
                }
            }
            if !all_success {
                std::process::exit(1);
            }
        }
        Commands::Query { what } => {
            let query_type = match what.as_str() {
                "cursor" => QueryType::Cursor,
                "content" => QueryType::Content,
                "modified" => QueryType::Modified,
                "mode" => QueryType::Mode,
                "filename" => QueryType::FileName,
                "status" => QueryType::Status,
                _ => {
                    eprintln!("ERROR: Unknown query type: {}", what);
                    eprintln!("Valid types: cursor, content, modified, mode, filename, status");
                    std::process::exit(1);
                }
            };

            let request = QueryRequest {
                id: format!("q-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()),
                query: query_type,
            };

            let msg = ClientMessage::Query(request);
            match client.send_message(&msg)? {
                ServerMessage::QueryResponse(resp) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::to_string_pretty(&resp)?);
                    } else {
                        match &resp {
                            rivas::protocol::QueryResponse::Cursor { position, .. } => {
                                println!("{}:{}", position.row, position.col);
                            }
                            rivas::protocol::QueryResponse::Content { content, .. } => {
                                print!("{}", content);
                            }
                            rivas::protocol::QueryResponse::Modified { modified, .. } => {
                                println!("{}", if *modified { "true" } else { "false" });
                            }
                            rivas::protocol::QueryResponse::Mode { mode, .. } => {
                                println!("{}", mode);
                            }
                            rivas::protocol::QueryResponse::FileName { filename, .. } => {
                                println!("{}", filename);
                            }
                            rivas::protocol::QueryResponse::Status {
                                cursor,
                                modified,
                                mode,
                                filename,
                                line_count,
                                ..
                            } => {
                                println!("FILE: {}", filename);
                                println!("MODE: {}", mode);
                                println!("CURSOR: {}:{}", cursor.row, cursor.col);
                                println!("MODIFIED: {}", if *modified { "true" } else { "false" });
                                println!("LINES: {}", line_count);
                            }
                            rivas::protocol::QueryResponse::Registers { registers, .. } => {
                                for (name, value) in registers {
                                    println!("\"{}: {}", name, value);
                                }
                            }
                            rivas::protocol::QueryResponse::UndoHistory { undo_count, redo_count, .. } => {
                                println!("UNDO: {}", undo_count);
                                println!("REDO: {}", redo_count);
                            }
                        }
                    }
                }
                ServerMessage::Error { message, .. } => {
                    eprintln!("ERROR: {}", message);
                    std::process::exit(1);
                }
                _ => {
                    eprintln!("ERROR: Unexpected response");
                    std::process::exit(1);
                }
            }
        }
        Commands::Run { command } => {
            let request = CommandRequest {
                id: format!("cmd-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()),
                command: command.clone(),
                args: vec![],
            };

            let msg = ClientMessage::Command(request);
            let resp = match client.send_message(&msg)? {
                ServerMessage::CommandResponse(resp) => resp,
                ServerMessage::Error { message, .. } => {
                    eprintln!("ERROR: {}", message);
                    std::process::exit(1);
                }
                _ => {
                    eprintln!("ERROR: Unexpected response");
                    std::process::exit(1);
                }
            };

            // Always output JSON for --run
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
    }

    Ok(())
}
