use crate::output::kitty;
use anyhow::Result;
use clap::Parser;
use iocraft::prelude::*;
use std::fs;
use std::io::{IsTerminal, Read, Write, stdin};
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
    let should_exit = hooks.use_state(|| false);
    let path = props.file_path.clone().unwrap_or_default();
    let path_name = path
        .to_str()
        .filter(|name| !name.is_empty())
        .unwrap_or("untitled.md")
        .to_string();
    let content = hooks.use_state(|| props.content.to_string());
    let mouse_captured = hooks.use_state(|| false);
    let edit_mode = hooks.use_state(|| props.edit);
    let mermaid_scale = hooks.use_state(|| 1.0f32);
    let editor_line = hooks.use_ref(|| 0usize);

    hooks.use_terminal_events({
        let mut mermaid_scale = mermaid_scale.clone();
        let mut should_exit = should_exit.clone();
        let mut edit_mode = edit_mode.clone();
        let mut mouse_captured = mouse_captured.clone();
        move |event| match event {
            TerminalEvent::Key(KeyEvent {
                code,
                modifiers: _,
                kind,
                ..
            }) if kind != KeyEventKind::Release => match code {
                KeyCode::Char('q') | KeyCode::Esc if !edit_mode.get() => should_exit.set(true),
                KeyCode::Char('e') if !edit_mode.get() => edit_mode.set(true),
                KeyCode::Char('m') => mouse_captured.set(true),
                KeyCode::Char('+') | KeyCode::Char('=') if !edit_mode.get() => {
                    mermaid_scale.set(mermaid_scale.get() + 0.1);
                }
                KeyCode::Char('-') if !edit_mode.get() => {
                    mermaid_scale.set((mermaid_scale.get() - 0.1).max(0.1));
                }
                _ => {}
            },
            _ => {}
        }
    });

    hooks.use_effect(
        {
            let _edit_mode = edit_mode.get();
            move || {
                if !kitty::is_supported() {
                    return;
                }
                let mut stdout = std::io::stdout().lock();
                kitty::delete_all(&mut stdout);
                let _ = stdout.flush();
            }
        },
        edit_mode.get(),
    );

    if should_exit.get() {
        if kitty::is_supported() {
            let mut stdout = std::io::stdout().lock();
            kitty::delete_all(&mut stdout);
            let _ = stdout.flush();
        }
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
    let on_view = hooks.use_async_handler(move |()| {
        let mut edit_mode = edit_mode.clone();
        async move {
            edit_mode.set(false);
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
                        on_change,
                        cursor_ref: Some(editor_line),
                        on_view
                    )
                }
                View(width: 1, height, background_color: Color::AnsiValue(238)) {}
                View(width: preview_width.saturating_sub(1), height, flex_direction: FlexDirection::Column, overflow: Overflow::Hidden) {
                    Document(content: current_content, file_path: path, viewport_height: height.saturating_sub(3) as u32, viewport_width: preview_width.saturating_sub(1) as u32, keyboard_navigation: Some(false), follow_ref: Some(editor_line), scale: Some(mermaid_scale.get()))
                    View(width: 100pct, background_color: Color::AnsiValue(238)) {
                        Text(content: " PREVIEW ", color: Color::AnsiValue(250), weight: Weight::Bold)
                    }
                    View(width: 100pct) {
                        Text(content: " :view returns to rendered view ", color: Color::AnsiValue(242))
                    }
                    View(width: 100pct, background_color: Color::AnsiValue(234), flex_direction: FlexDirection::Row) {
                        Text(content: " live markdown preview ", color: Color::AnsiValue(242))
                        View(flex_grow: 1.0) {}
                        Text(content: format!(" Zoom: {:.1}x ", mermaid_scale.get()), color: Color::AnsiValue(242))
                    }
                }
            }
        }
    } else {
        element! {
            View(flex_direction: FlexDirection::Column,  width, height) {
                Document(content: current_content, file_path: path, viewport_height: height.saturating_sub(1) as u32, viewport_width: width as u32, keyboard_navigation: Some(true), follow_ref: None, scale: Some(mermaid_scale.get()))
                View(width: 100pct, height: 1, background_color: Color::AnsiValue(236), flex_direction: FlexDirection::Row) {
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " q ", color: Color::Black)
                    }
                    Text(content: " Quit ")
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " e ", color: Color::Black)
                    }
                    Text(content: " Edit ")
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " j/k ", color: Color::Black)
                    }
                    Text(content: " Scroll ")
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " gg/G ", color: Color::Black)
                    }
                    Text(content: " Top/Bottom ")
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " + ", color: Color::Black)
                    }
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " - ", color: Color::Black)
                    }
                    Text(content: " Zoom ")
                    View(background_color: Color::AnsiValue(244)) {
                        Text(content: " m ", color: Color::Black)
                    }
                    Text(content: " Mouse ")
                    View(flex_grow: 1.0) {}
                    Text(content: format!(" Zoom: {:.1}x ", mermaid_scale.get()), color: Color::AnsiValue(245))
                }
            }
        }
    }
}
