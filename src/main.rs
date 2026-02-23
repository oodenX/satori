mod cli;
mod config;
mod history;
mod i18n;
mod init;
#[cfg(feature = "gui")]
mod overlay;
#[cfg(feature = "gui-slint")]
mod overlay_slint;
mod prompt;
mod provider;
#[cfg(any(feature = "gui", feature = "gui-slint"))]
mod screenshot;
mod translate;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Commands, OutputMode, Position};

fn main() -> Result<()> {
    // Suppress noisy MESA driver warnings on Intel GPUs
    if std::env::var_os("MESA_LOG_LEVEL").is_none() {
        // SAFETY: called before any other threads are spawned
        unsafe { std::env::set_var("MESA_LOG_LEVEL", "error") };
    }

    let cli = Cli::parse();

    // Handle subcommands first
    if let Some(command) = cli.command {
        return match command {
            Commands::Init => init::run_init(),
            Commands::Completions { shell } => {
                let mut cmd = <Cli as clap::CommandFactory>::command();
                clap_complete::generate(shell, &mut cmd, "satori", &mut std::io::stdout());
                Ok(())
            }
            Commands::History { action } => {
                use cli::HistoryAction;
                match action {
                    Some(HistoryAction::Search { query }) => {
                        let entries = history::search_entries(&query)?;
                        history::print_entries(&entries);
                    }
                    Some(HistoryAction::Clear) => {
                        history::clear()?;
                        println!("History cleared.");
                    }
                    Some(HistoryAction::List { limit }) => {
                        let entries = history::list_entries(Some(limit))?;
                        history::print_entries(&entries);
                    }
                    None => {
                        let entries = history::list_entries(Some(20))?;
                        history::print_entries(&entries);
                    }
                }
                Ok(())
            }
        };
    }

    // Load config (optional — CLI-only usage is possible for some paths)
    let config = config::load_config().ok();

    let settings = config.as_ref().map(|c| &c.settings);

    // Apply renderer setting before any GTK initialization
    #[cfg(feature = "gui")]
    if let Some(renderer) = settings.and_then(|s| s.renderer.as_deref())
        && renderer == "cairo"
        && std::env::var_os("GSK_RENDERER").is_none()
    {
        // SAFETY: called before any other threads are spawned and before GTK init
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }

    // Resolve profile name: CLI --profile > config active_profile
    let profile_name = cli
        .profile
        .as_deref()
        .or(settings.map(|s| s.active_profile.as_str()));

    let profile = profile_name.and_then(|name| config.as_ref().and_then(|c| c.profiles.get(name)));

    let profile = profile
        .with_context(|| "No profile found. Run `satori init` to create a configuration.")?;

    let provider =
        provider::create_provider(profile).context("Failed to create translation provider")?;

    // Resolve all options: CLI flags override config values
    let target_lang = cli
        .target
        .clone()
        .or(settings.map(|s| s.target_lang.clone()))
        .unwrap_or_else(i18n::detect_target_lang);
    let style = cli
        .style
        .clone()
        .or(settings.map(|s| s.translation_style.clone()))
        .unwrap_or_else(|| "general".to_string());
    let position = cli
        .pos
        .or(settings.and_then(|s| s.last_pos))
        .unwrap_or(Position::BottomRight);

    // Determine output mode
    let output_mode = cli.output.unwrap_or_else(|| {
        if cli.image.is_some() {
            OutputMode::Terminal
        } else {
            OutputMode::Overlay
        }
    });

    match output_mode {
        OutputMode::Terminal => run_terminal(&cli, &provider, &target_lang, &style),
        OutputMode::Overlay => run_overlay(cli, provider, target_lang, style, position, settings),
    }
}

fn run_terminal(
    cli: &Cli,
    provider: &provider::AnyProvider,
    target_lang: &str,
    style: &str,
) -> Result<()> {
    let image_data = match &cli.image {
        Some(path) => {
            std::fs::read(path).with_context(|| format!("Failed to read image file: {path}"))?
        }
        None => {
            #[cfg(any(feature = "gui", feature = "gui-slint"))]
            {
                screenshot::take_screenshot().context("Screenshot capture failed")?
            }
            #[cfg(not(any(feature = "gui", feature = "gui-slint")))]
            {
                anyhow::bail!(
                    "No image file specified and GUI feature is disabled.\n\
                     Provide an image path: satori <IMAGE>"
                );
            }
        }
    };

    let result = translate::translate_bytes(provider, &image_data, target_lang, style)?;

    // Record to history (best effort)
    let _ = history::record(
        &result.source_text,
        &result.translated_text,
        target_lang,
        style,
    );

    if !result.source_text.is_empty() {
        println!("Source:      {}", result.source_text);
    }
    if !result.translated_text.is_empty() {
        println!("Translation: {}", result.translated_text);
    }
    if result.source_text.is_empty() && result.translated_text.is_empty() {
        println!("No text detected in image.");
    }

    Ok(())
}

#[cfg(any(feature = "gui", feature = "gui-slint"))]
fn run_overlay(
    cli: Cli,
    provider: provider::AnyProvider,
    target_lang: String,
    style: String,
    position: Position,
    settings: Option<&config::Settings>,
) -> Result<()> {
    let image_data = match cli.image {
        Some(ref path) => Some(
            std::fs::read(path).with_context(|| format!("Failed to read image file: {path}"))?,
        ),
        None => None, // will screenshot in overlay
    };

    // Route to the appropriate GUI backend
    let gui_backend = settings.map(|s| s.gui_backend).unwrap_or_default();

    match gui_backend {
        #[cfg(feature = "gui-slint")]
        config::GuiBackend::Slint => {
            let _ = position;
            overlay_slint::run(provider, target_lang, style, image_data);
        }
        #[cfg(not(feature = "gui-slint"))]
        config::GuiBackend::Slint => {
            let _ = (image_data, position);
            anyhow::bail!(
                "Slint backend not compiled. Rebuild with: \
                 cargo build --no-default-features --features gui-slint"
            );
        }
        #[cfg(feature = "gui")]
        config::GuiBackend::Gtk4 => {
            let ui_opacity = settings.map(|s| s.ui_opacity).unwrap_or(0.9);

            let overlay_config = overlay::OverlayConfig {
                target_lang,
                translation_style: style,
                ui_opacity,
                position,
                font: cli.font.or(settings.and_then(|s| s.font.clone())),
                color: cli.color.or(settings.and_then(|s| s.color.clone())),
                background_color: cli
                    .background_color
                    .or(settings.and_then(|s| s.background_color.clone())),
                background_opacity: cli
                    .background_opacity
                    .or(settings.and_then(|s| s.background_opacity)),
                image_data,
                last_margins: settings
                    .filter(|s| s.position_mode == "dynamic")
                    .and_then(|s| s.last_margins),
            };

            overlay::run(provider, overlay_config);
        }
        #[cfg(not(feature = "gui"))]
        config::GuiBackend::Gtk4 => {
            anyhow::bail!(
                "GTK4 backend not compiled. Rebuild with default features, \
                 or set gui_backend = \"slint\" in config."
            );
        }
    }

    Ok(())
}

#[cfg(not(any(feature = "gui", feature = "gui-slint")))]
fn run_overlay(
    _cli: Cli,
    _provider: provider::AnyProvider,
    _target_lang: String,
    _style: String,
    _position: Position,
    _settings: Option<&config::Settings>,
) -> Result<()> {
    anyhow::bail!(
        "Overlay mode requires a GUI feature.\n\
         Rebuild with: cargo build --features gui       (GTK4)\n\
         Or:           cargo build --features gui-slint (Slint)\n\
         Or use terminal mode: satori <IMAGE> --output terminal"
    );
}
