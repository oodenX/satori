use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "satori",
    version,
    about = "Wayland screen translator for manga and visual novels",
    long_about = "Satori captures a screen region or reads an image file, sends it to a Vision LLM\nfor OCR and context-aware translation, and displays the result in terminal or as a floating overlay."
)]
pub struct Cli {
    /// Image file to translate (png, jpg, webp). If omitted, captures screen via slurp/grim.
    pub image: Option<String>,

    /// Target translation language (overrides config)
    #[arg(short = 't', long)]
    pub target: Option<String>,

    /// Translation style: general, manga, novel, game
    #[arg(short = 's', long)]
    pub style: Option<String>,

    /// Output mode
    #[arg(short = 'o', long, value_enum)]
    pub output: Option<OutputMode>,

    /// Overlay position on screen
    #[arg(short = 'p', long, value_enum)]
    pub pos: Option<Position>,

    /// Config profile to use (overrides active_profile)
    #[arg(long)]
    pub profile: Option<String>,

    /// Text color in CSS format, e.g. "#ffffff" (overlay only)
    #[arg(long)]
    pub color: Option<String>,

    /// Font family, e.g. "Noto Sans CJK SC" (overlay only)
    #[arg(long)]
    pub font: Option<String>,

    /// Background color in CSS format, e.g. "#000000" (overlay only)
    #[arg(long)]
    pub background_color: Option<String>,

    /// Background opacity 0.0-1.0 (overlay only)
    #[arg(long)]
    pub background_opacity: Option<f64>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive configuration wizard
    Init,
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// View translation history
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
}

#[derive(Subcommand)]
pub enum HistoryAction {
    /// List recent translations (default)
    List {
        /// Number of entries to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },
    /// Search history by keyword
    Search {
        /// Search query
        query: String,
    },
    /// Clear all history
    Clear,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputMode {
    /// Print translation to terminal
    Terminal,
    /// Show translation in floating overlay window
    Overlay,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Center => write!(f, "center"),
            Self::TopLeft => write!(f, "top-left"),
            Self::TopRight => write!(f, "top-right"),
            Self::BottomLeft => write!(f, "bottom-left"),
            Self::BottomRight => write!(f, "bottom-right"),
        }
    }
}

impl std::fmt::Display for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal => write!(f, "terminal"),
            Self::Overlay => write!(f, "overlay"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_no_args() {
        let cli = Cli::parse_from(["satori"]);
        assert!(cli.image.is_none());
        assert!(cli.command.is_none());
        assert!(cli.target.is_none());
    }

    #[test]
    fn parse_image_arg() {
        let cli = Cli::parse_from(["satori", "test.png"]);
        assert_eq!(cli.image.as_deref(), Some("test.png"));
    }

    #[test]
    fn parse_with_options() {
        let cli = Cli::parse_from([
            "satori",
            "img.jpg",
            "--target",
            "简体中文",
            "--style",
            "manga",
            "--output",
            "terminal",
            "--pos",
            "top-right",
        ]);
        assert_eq!(cli.image.as_deref(), Some("img.jpg"));
        assert_eq!(cli.target.as_deref(), Some("简体中文"));
        assert_eq!(cli.style.as_deref(), Some("manga"));
        assert_eq!(cli.output, Some(OutputMode::Terminal));
        assert_eq!(cli.pos, Some(Position::TopRight));
    }

    #[test]
    fn parse_init_subcommand() {
        let cli = Cli::parse_from(["satori", "init"]);
        assert!(matches!(cli.command, Some(Commands::Init)));
    }

    #[test]
    fn parse_completions_subcommand() {
        let cli = Cli::parse_from(["satori", "completions", "bash"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Completions {
                shell: clap_complete::Shell::Bash
            })
        ));
    }
}
