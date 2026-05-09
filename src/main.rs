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
use crate::components::editor::NvimEditor;

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
    /// Open a side-by-side markdown editor and live preview.
    #[arg(short, long)]
    edit: bool,
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

    smol::block_on(
        element!(App(file_path, content: content.as_str(), edit: cli.edit)).fullscreen(),
    )?;
    Ok(())
}

#[derive(Default, Props)]
struct AppProps<'a> {
    file_path: Option<PathBuf>,
    content: &'a str,
    edit: bool,
}

#[component]
fn App<'a>(props: &AppProps<'a>, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);
    let path = props.file_path.clone().unwrap_or_default();
    let path_name = path
        .to_str()
        .filter(|name| !name.is_empty())
        .unwrap_or("untitled.md")
        .to_string();
    let content = hooks.use_state(|| props.content.to_string());
    let mut mouse_captured = hooks.use_state(|| false);
    let mut edit_mode = hooks.use_state(|| props.edit);

    hooks.use_terminal_events(move |event| match event {
        TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
            match code {
                KeyCode::Char('q') | KeyCode::Esc if !edit_mode.get() => should_exit.set(true),
                KeyCode::Char('e') if !edit_mode.get() => edit_mode.set(true),
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
    let current_content = content.read().clone();
    let on_change = hooks.use_async_handler(move |next_content: String| {
        let mut content = content.clone();
        async move {
            content.set(next_content);
        }
    });

    if edit_mode.get() {
        let editor_width = (width / 2).max(1);
        let preview_width = width.saturating_sub(editor_width).max(1);

        element! {
            View(flex_direction: FlexDirection::Row, width, height) {
                View(width: editor_width, height, overflow: Overflow::Hidden) {
                    NvimEditor(
                        filename: path_name,
                        initial_content: current_content.clone(),
                        viewport_width: editor_width,
                        viewport_height: height,
                        on_change
                    )
                }
                View(width: 1, height, background_color: Color::AnsiValue(238)) {}
                View(width: preview_width.saturating_sub(1), height, overflow: Overflow::Hidden) {
                    Document(content: current_content, file_path: path, viewport_height: height as u32, viewport_width: preview_width.saturating_sub(1) as u32)
                }
            }
        }
    } else {
        element! {
            View(flex_direction: FlexDirection::Column,  width, height) {
                Document(content: current_content, file_path: path, viewport_height: height as u32, viewport_width: width as u32 )
            }
        }
    }
}
