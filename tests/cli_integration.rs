use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use rivas::protocol::{ClientMessage, CommandRequest, ServerMessage};
use rivas::Editor;

fn handle_connection(stream: UnixStream) {
    let mut editor = Editor::new("test.md", "hello\nworld");
    let read_stream = stream.try_clone().unwrap();
    let reader = BufReader::new(read_stream);
    let mut writer = stream;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<ClientMessage>(&line) {
            Ok(ClientMessage::Command(cmd)) => {
                let result = editor.execute_vim(&cmd.command);

                let response = rivas::protocol::CommandResponse {
                    id: cmd.id,
                    success: result.success,
                    message: result.message,
                    cursor: rivas::protocol::CursorPosition::new(
                        result.cursor.row,
                        result.cursor.col,
                    ),
                    modified: result.modified,
                    error_code: None,
                };

                serde_json::to_string(&ServerMessage::CommandResponse(response)).unwrap()
            }
            Ok(ClientMessage::Query(query)) => {
                let response = match query.query {
                    rivas::protocol::QueryType::Cursor => rivas::protocol::QueryResponse::Cursor {
                        id: query.id,
                        position: rivas::protocol::CursorPosition::new(
                            editor.cursor().row,
                            editor.cursor().col,
                        ),
                    },
                    rivas::protocol::QueryType::Content => rivas::protocol::QueryResponse::Content {
                        id: query.id,
                        content: editor.content(),
                    },
                    _ => rivas::protocol::QueryResponse::Status {
                        id: query.id,
                        cursor: rivas::protocol::CursorPosition::new(
                            editor.cursor().row,
                            editor.cursor().col,
                        ),
                        modified: editor.is_modified(),
                        mode: format!("{:?}", editor.mode()),
                        filename: "test.md".to_string(),
                        line_count: editor.content().lines().count(),
                    },
                };

                serde_json::to_string(&ServerMessage::QueryResponse(response)).unwrap()
            }
            Err(e) => {
                serde_json::to_string(&ServerMessage::Error {
                    id: "0".to_string(),
                    message: format!("Invalid message: {}", e),
                    error_code: None,
                })
                .unwrap()
            }
        };

        if writeln!(writer, "{}", response).is_err() {
            break;
        }
        if writer.flush().is_err() {
            break;
        }
    }
}

#[test]
fn test_socket_communication() {
    let socket_path = PathBuf::from(format!(
        "/tmp/rivas-test-{}.sock",
        std::process::id()
    ));
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).unwrap();
    }

    let listener = UnixListener::bind(&socket_path).unwrap();
    listener.set_nonblocking(false).unwrap();

    // Server thread
    let _server = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_connection(stream);
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    // Client
    let stream = UnixStream::connect(&socket_path).unwrap();
    let write_stream = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut writer = write_stream;

    // Send command
    let request = CommandRequest {
        id: "test-1".to_string(),
        command: "l".to_string(),
        args: vec![],
    };
    let msg = ClientMessage::Command(request);
    let json = serde_json::to_string(&msg).unwrap();
    writeln!(writer, "{}", json).unwrap();
    writer.flush().unwrap();

    // Read response
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    let server_msg: ServerMessage = serde_json::from_str(response.trim()).unwrap();
    match server_msg {
        ServerMessage::CommandResponse(resp) => {
            assert!(resp.success);
            assert_eq!(resp.cursor.row, 0);
            assert_eq!(resp.cursor.col, 1);
        }
        _ => panic!("Expected CommandResponse"),
    }

    drop(reader);
    drop(writer);
    let _ = std::fs::remove_file(&socket_path);
}

#[test]
fn test_multiple_commands() {
    let socket_path = PathBuf::from(format!(
        "/tmp/rivas-test-multi-{}.sock",
        std::process::id()
    ));
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).unwrap();
    }

    let listener = UnixListener::bind(&socket_path).unwrap();
    listener.set_nonblocking(false).unwrap();

    // Server with multi-line content
    let _server = std::thread::spawn(move || {
        let mut editor = Editor::new("test.md", "line1\nline2\nline3");
        if let Ok((stream, _)) = listener.accept() {
            let read_stream = stream.try_clone().unwrap();
            let reader = BufReader::new(read_stream);
            let mut writer = stream;

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };

                if line.is_empty() {
                    continue;
                }

                let response = match serde_json::from_str::<ClientMessage>(&line) {
                    Ok(ClientMessage::Command(cmd)) => {
                        let result = editor.execute_vim(&cmd.command);

                        let response = rivas::protocol::CommandResponse {
                            id: cmd.id,
                            success: result.success,
                            message: result.message,
                            cursor: rivas::protocol::CursorPosition::new(
                                result.cursor.row,
                                result.cursor.col,
                            ),
                            modified: result.modified,
                            error_code: None,
                        };

                        serde_json::to_string(&ServerMessage::CommandResponse(response)).unwrap()
                    }
                    Ok(ClientMessage::Query(query)) => {
                        let response = match query.query {
                            rivas::protocol::QueryType::Content => {
                                rivas::protocol::QueryResponse::Content {
                                    id: query.id,
                                    content: editor.content(),
                                }
                            }
                            _ => {
                                rivas::protocol::QueryResponse::Content {
                                    id: query.id,
                                    content: editor.content(),
                                }
                            }
                        };
                        serde_json::to_string(&ServerMessage::QueryResponse(response)).unwrap()
                    }
                    Err(e) => {
                        serde_json::to_string(&ServerMessage::Error {
                            id: "0".to_string(),
                            message: format!("Invalid message: {}", e),
                            error_code: None,
                        })
                        .unwrap()
                    }
                };

                if writeln!(writer, "{}", response).is_err() {
                    break;
                }
                if writer.flush().is_err() {
                    break;
                }
            }
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    // Client
    let stream = UnixStream::connect(&socket_path).unwrap();
    let write_stream = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut writer = write_stream;

    let commands = vec!["2G", "dd", "G"];
    for cmd in commands {
        let request = CommandRequest {
            id: format!("cmd-{}", cmd),
            command: cmd.to_string(),
            args: vec![],
        };

        let msg = ClientMessage::Command(request);
        let json = serde_json::to_string(&msg).unwrap();
        writeln!(writer, "{}", json).unwrap();
        writer.flush().unwrap();

        let mut response = String::new();
        reader.read_line(&mut response).unwrap();

        let server_msg: ServerMessage = serde_json::from_str(response.trim()).unwrap();
        match server_msg {
            ServerMessage::CommandResponse(resp) => {
                assert!(resp.success, "Command '{}' failed: {}", cmd, resp.message);
            }
            _ => panic!("Expected CommandResponse for command: {}", cmd),
        }
    }

    // Query final content
    let request = rivas::protocol::QueryRequest {
        id: "final-content".to_string(),
        query: rivas::protocol::QueryType::Content,
    };
    let msg = ClientMessage::Query(request);
    let json = serde_json::to_string(&msg).unwrap();
    writeln!(writer, "{}", json).unwrap();
    writer.flush().unwrap();

    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    let server_msg: ServerMessage = serde_json::from_str(response.trim()).unwrap();
    match server_msg {
        ServerMessage::QueryResponse(resp) => match resp {
            rivas::protocol::QueryResponse::Content { content, .. } => {
                assert_eq!(content, "line1\nline3");
            }
            _ => panic!("Expected Content response"),
        },
        _ => panic!("Expected QueryResponse"),
    }

    drop(reader);
    drop(writer);
    let _ = std::fs::remove_file(&socket_path);
}
