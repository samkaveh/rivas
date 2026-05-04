use anyhow::Result;
use clap::Parser;
use std::fs;
use std::io::{IsTerminal, Read, stdin};
use std::path::PathBuf;

mod assets;
mod document;
mod output;
mod render;
mod viewer;

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

    // Theme selection
    let theme = match cli.theme.as_str() {
        "light" => render::theme::Theme::light(),
        _ => render::theme::Theme::dark(),
    };

    // Initialize and run viewer
    let mut viewer = viewer::Viewer::new(content, file_path, theme)?;
    viewer.run()
}
