use anyhow::Result;
use clap::Parser;
use iocraft::prelude::*;
use std::fs;
use std::io::{IsTerminal, Read, stdin};
use std::path::PathBuf;
mod assets;
mod components;
mod document;
mod output;

use crate::components::document::Document;

#[derive(Parser)]
#[command(
    name = "rivas",
    about = "Terminal markdown viewer and editor with pixel perfect rendering"
)]
struct Cli {
    /// Markdown file to view (reads stdin if omitted)
    file: Option<PathBuf>,
    /// Theme: dark, light
    #[arg(short, long, default_value = "dark")]
    theme: String,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    let (content, file_path) = match &cli.file {
        // CASE 1: User provided a path
        Some(path) => {
            if !path.exists() {
                // Create the file (and parent directories) if it doesn't exist
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::File::create(path)?;
                (String::new(), Some(path.clone()))
            } else {
                (fs::read_to_string(path)?, Some(path.clone()))
            }
        }
        // CASE 2: No path provided - Check for Pipe vs. New Default File
        None => {
            if !stdin().is_terminal() {
                // Data is being piped in (e.g., `echo "# Hi" | rivas`)
                let mut s = String::new();
                stdin().read_to_string(&mut s)?;
                (s, None)
            } else {
                // No pipe and no arg: Open a default new file
                let default_path = PathBuf::from("untitled.md");
                if !default_path.exists() {
                    fs::File::create(&default_path)?;
                }
                (String::new(), Some(default_path))
            }
        }
    };

    // Terminal capability check
    let caps = output::capabilities::TermCaps::detect()?;
    if !caps.has_kitty {
        anyhow::bail!("Terminal does not support Kitty, use Kitty, WezTerm or Ghostty.")
    }

    smol::block_on(element!(App(file_path, content: content.as_str())).fullscreen())?;
    Ok(())
}

#[derive(Default, Props)]
struct AppProps<'a> {
    file_path: Option<PathBuf>,
    content: &'a str,
}

#[component]
fn App<'a>(props: &AppProps<'a>, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);
    let path = props.file_path.clone().unwrap_or_default();
    let _path_name = path.to_str().unwrap_or_default();
    let content = props.content;
    let mut mouse_captured = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => should_exit.set(true),
                KeyCode::Char('m') => mouse_captured.set(true),
                _ => {}
            }
        }
        _ => {}
    });

    if should_exit.get() {
        system.exit();
    }

    system.set_mouse_capture(mouse_captured.get());

    element! {
        View(flex_direction: FlexDirection::Column,  width, height) {
            Document(content: content, file_path: path, viewport_height: height as u32, viewport_width: width as u32 )
        }
    }
}
