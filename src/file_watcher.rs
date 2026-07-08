use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
    path: PathBuf,
    last_event: Instant,
}

impl FileWatcher {
    pub fn new(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        // Watch the parent directory to handle file replacement
        if let Some(parent) = path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        }

        Ok(Self {
            watcher,
            rx,
            path,
            last_event: Instant::now(),
        })
    }

    pub fn check_for_changes(&mut self) -> Option<String> {
        // Debounce events (100ms)
        if self.last_event.elapsed() < Duration::from_millis(100) {
            return None;
        }

        match self.rx.try_recv() {
            Ok(Ok(event)) => {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        // Check if our file was modified
                        if event.paths.iter().any(|p| p == &self.path) {
                            self.last_event = Instant::now();
                            // Read the new content
                            std::fs::read_to_string(&self.path).ok()
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            Ok(Err(_)) => None,
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    }
}
