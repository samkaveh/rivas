use anyhow::Result;
use clap::Parser;
use std::io::Read;
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

    let content = match &cli.file {
        Some(path) => std::fs::read_to_string(path)?,
        None => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
    };

    let caps = output::capabilities::TermCaps::detect()?;
    if !caps.has_kitty {
        anyhow::bail!("Terminal does not support Kitty, use Kitty, WezTerm or Ghostty.")
    }

    let theme = match cli.theme.as_str() {
        "light" => render::theme::Theme::light(),
        _ => render::theme::Theme::dark(),
    };

    let mut viewer = viewer::Viewer::new(content, cli.file.clone(), theme)?;

    viewer.run()
}
