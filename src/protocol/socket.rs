use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use super::types::*;

pub struct SocketServer {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl SocketServer {
    pub fn new(socket_path: PathBuf) -> std::io::Result<Self> {
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            socket_path,
        })
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn accept_connections(&self) -> Vec<UnixStream> {
        let mut streams = Vec::new();
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).ok();
                    streams.push(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        streams
    }

    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

pub struct SocketClient {
    stream: UnixStream,
}

impl SocketClient {
    pub fn connect(socket_path: &PathBuf) -> std::io::Result<Self> {
        let stream = UnixStream::connect(socket_path)?;
        stream.set_nonblocking(false)?;
        Ok(Self { stream })
    }

    pub fn send_message(&mut self, msg: &ClientMessage) -> std::io::Result<ServerMessage> {
        let json = serde_json::to_string(msg)?;
        writeln!(self.stream, "{}", json)?;
        self.stream.flush()?;

        let mut reader = BufReader::new(&self.stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        let server_msg: ServerMessage = serde_json::from_str(&response.trim())?;
        Ok(server_msg)
    }
}
