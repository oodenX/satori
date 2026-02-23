use std::io::{self, Write};

use anyhow::{Context, Result};

use crate::config::{self, AppConfig, Profile, Settings};
use crate::i18n;

/// Provider presets for interactive selection.
struct ProviderPreset {
    name: &'static str,
    driver: &'static str,
    base_url: &'static str,
    default_model: &'static str,
    api_key_env: Option<&'static str>,
    note: &'static str,
}

const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        name: "OpenRouter (aggregator, many models)",
        driver: "openai_compatible",
        base_url: "https://openrouter.ai/api/v1",
        default_model: "google/gemini-2.5-flash",
        api_key_env: Some("SATORI_OPENROUTER_KEY"),
        note: "Vision ✓ — Access 200+ models via one API key",
    },
    ProviderPreset {
        name: "Google Gemini",
        driver: "openai_compatible",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        default_model: "gemini-2.5-flash",
        api_key_env: Some("SATORI_GEMINI_KEY"),
        note: "Vision ✓ — Free tier available, excellent multimodal",
    },
    ProviderPreset {
        name: "OpenAI",
        driver: "openai_compatible",
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o",
        api_key_env: Some("SATORI_OPENAI_KEY"),
        note: "Vision ✓ — gpt-4o, gpt-4o-mini support images",
    },
    ProviderPreset {
        name: "Anthropic Claude",
        driver: "openai_compatible",
        base_url: "https://api.anthropic.com/v1",
        default_model: "claude-sonnet-4-20250514",
        api_key_env: Some("SATORI_ANTHROPIC_KEY"),
        note: "Vision ✓ — Excellent translation quality",
    },
    ProviderPreset {
        name: "xAI Grok",
        driver: "openai_compatible",
        base_url: "https://api.x.ai/v1",
        default_model: "grok-2-vision-1212",
        api_key_env: Some("SATORI_XAI_KEY"),
        note: "Vision ✓ — Use grok-2-vision model for images",
    },
    ProviderPreset {
        name: "DeepSeek",
        driver: "openai_compatible",
        base_url: "https://api.deepseek.com/v1",
        default_model: "deepseek-chat",
        api_key_env: Some("SATORI_DEEPSEEK_KEY"),
        note: "⚠ Vision ✗ — deepseek-chat is text-only; use via OpenRouter for vision models",
    },
    ProviderPreset {
        name: "Ollama (local)",
        driver: "ollama",
        base_url: "http://localhost:11434",
        default_model: "llava",
        api_key_env: None,
        note: "Vision ✓ — llava, llava-llama3, minicpm-v support images",
    },
];

const STYLES: &[(&str, &str)] = &[
    ("general", "General purpose translation"),
    ("manga", "Manga / Comics (onomatopoeia, speech bubbles)"),
    ("novel", "Visual novels (narrative, dialogue)"),
    ("game", "Games (UI, menus, quests)"),
];

const POSITIONS: &[(&str, &str)] = &[
    ("bottom-right", "Bottom right (default)"),
    ("bottom-left", "Bottom left"),
    ("top-right", "Top right"),
    ("top-left", "Top left"),
    ("center", "Center"),
];

/// Run the interactive `satori init` wizard.
pub fn run_init() -> Result<()> {
    println!("🔧 Satori Configuration Wizard\n");

    if config::config_exists() {
        print!("⚠  Config file already exists. Overwrite? [y/N] ");
        io::stdout().flush()?;
        let answer = read_line()?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
        println!();
    }

    // 1. Target language
    let detected = i18n::detect_target_lang();
    println!("📝 Available target languages:");
    for (i, (_, name)) in i18n::LANGUAGES.iter().enumerate() {
        let marker = if *name == detected { " (detected)" } else { "" };
        println!("  {}: {}{}", i + 1, name, marker);
    }
    print!("\nSelect target language [default: {}]: ", detected);
    io::stdout().flush()?;
    let lang_input = read_line()?;
    let target_lang = if lang_input.trim().is_empty() {
        detected
    } else if let Ok(idx) = lang_input.trim().parse::<usize>() {
        if idx >= 1 && idx <= i18n::LANGUAGES.len() {
            i18n::LANGUAGES[idx - 1].1.to_string()
        } else {
            lang_input.trim().to_string()
        }
    } else {
        lang_input.trim().to_string()
    };
    println!("  → {}\n", target_lang);

    // 2. Translation style
    println!("🎨 Translation styles:");
    for (i, (_, desc)) in STYLES.iter().enumerate() {
        println!("  {}: {}", i + 1, desc);
    }
    print!("\nSelect style [default: 1]: ");
    io::stdout().flush()?;
    let style_input = read_line()?;
    let style_idx = style_input
        .trim()
        .parse::<usize>()
        .unwrap_or(1)
        .saturating_sub(1)
        .min(STYLES.len() - 1);
    let translation_style = STYLES[style_idx].0.to_string();
    println!("  → {}\n", STYLES[style_idx].1);

    // 3. Provider
    println!("🤖 LLM Provider:");
    for (i, preset) in PRESETS.iter().enumerate() {
        println!("  {}: {}", i + 1, preset.name);
        println!("     {}", preset.note);
    }
    print!("\nSelect provider [default: 1]: ");
    io::stdout().flush()?;
    let provider_input = read_line()?;
    let preset_idx = provider_input
        .trim()
        .parse::<usize>()
        .unwrap_or(1)
        .saturating_sub(1)
        .min(PRESETS.len() - 1);
    let preset = &PRESETS[preset_idx];
    println!("  → {}\n", preset.name);

    // 4. Model
    print!("🧠 Model name [default: {}]: ", preset.default_model);
    io::stdout().flush()?;
    let model_input = read_line()?;
    let model = if model_input.trim().is_empty() {
        preset.default_model.to_string()
    } else {
        model_input.trim().to_string()
    };
    println!("  → {}\n", model);

    // 5. API key (cloud only)
    let api_key_env = if let Some(default_env) = preset.api_key_env {
        print!(
            "🔑 Environment variable for API key [default: {}]: ",
            default_env
        );
        io::stdout().flush()?;
        let key_input = read_line()?;
        let env_name = if key_input.trim().is_empty() {
            default_env.to_string()
        } else {
            key_input.trim().to_string()
        };

        // Check if the env var is set
        if std::env::var(&env_name).is_err() {
            println!(
                "\n  ⚠  ${} is not set. Remember to set it before running satori:",
                env_name
            );
            println!("     export {}=\"your-api-key-here\"\n", env_name);
        } else {
            println!("  → {} ✓\n", env_name);
        }
        Some(env_name)
    } else {
        println!("  ℹ  No API key needed for local Ollama.\n");
        None
    };

    // 6. Default overlay position
    println!("📍 Default overlay position:");
    for (i, (_, desc)) in POSITIONS.iter().enumerate() {
        println!("  {}: {}", i + 1, desc);
    }
    print!("\nSelect position [default: 1]: ");
    io::stdout().flush()?;
    let pos_input = read_line()?;
    let pos_idx = pos_input
        .trim()
        .parse::<usize>()
        .unwrap_or(1)
        .saturating_sub(1)
        .min(POSITIONS.len() - 1);
    let last_pos_str = POSITIONS[pos_idx].0;
    let last_pos: crate::cli::Position = match last_pos_str {
        "bottom-left" => crate::cli::Position::BottomLeft,
        "top-right" => crate::cli::Position::TopRight,
        "top-left" => crate::cli::Position::TopLeft,
        "center" => crate::cli::Position::Center,
        _ => crate::cli::Position::BottomRight,
    };
    println!("  → {}\n", POSITIONS[pos_idx].1);

    // 7. Position mode
    println!("🔄 Position mode:");
    println!("  1: Dynamic (default — remember last position between sessions)");
    println!("  2: Fixed (reset to default position each time)");
    print!("\nSelect position mode [default: 1]: ");
    io::stdout().flush()?;
    let mode_input = read_line()?;
    let position_mode = match mode_input.trim() {
        "2" => {
            println!("  → Fixed position\n");
            "fixed".to_string()
        }
        _ => {
            println!("  → Dynamic position\n");
            "dynamic".to_string()
        }
    };

    // 8. GTK renderer (memory optimization)
    println!("🖥  GTK Renderer:");
    println!("  1: Auto (default — uses GPU via Vulkan/OpenGL)");
    println!("  2: Cairo (software — lower memory, slightly higher CPU)");
    print!("\nSelect renderer [default: 1]: ");
    io::stdout().flush()?;
    let renderer_input = read_line()?;
    let renderer = match renderer_input.trim() {
        "2" => {
            println!("  → Cairo (software renderer)\n");
            Some("cairo".to_string())
        }
        _ => {
            println!("  → Auto (GPU renderer)\n");
            None
        }
    };

    // 9. GUI backend
    println!("🧩 GUI Backend:");
    println!("  1: GTK4 (default — Wayland layer-shell overlay, always-on-top)");
    println!("  2: Slint (lightweight — regular window, lower memory)");
    println!("     Note: Slint uses a normal window without Wayland layer-shell.");
    println!("     The overlay won't stay above other windows automatically.");
    print!("\nSelect GUI backend [default: 1]: ");
    io::stdout().flush()?;
    let backend_input = read_line()?;
    let gui_backend = match backend_input.trim() {
        "2" => {
            println!(
                "  → Slint (build with: cargo build --features gui-slint --no-default-features)\n"
            );
            "slint".to_string()
        }
        _ => {
            println!("  → GTK4\n");
            "gtk4".to_string()
        }
    };

    // Build config
    let profile_name = preset
        .name
        .split_once(' ')
        .map(|(first, _)| first)
        .unwrap_or(preset.name)
        .to_lowercase();
    let profile = Profile {
        driver: preset.driver.to_string(),
        api_key_env,
        base_url: preset.base_url.to_string(),
        model,
        timeout_sec: 15,
    };

    let mut profiles = std::collections::HashMap::new();
    profiles.insert(profile_name.clone(), profile);

    let config = AppConfig {
        settings: Settings {
            active_profile: profile_name,
            target_lang,
            translation_style,
            ui_opacity: 0.9,
            last_pos: Some(last_pos),
            last_margins: None,
            font: None,
            color: None,
            background_color: None,
            background_opacity: None,
            renderer,
            position_mode,
            gui_backend,
        },
        profiles,
    };

    // Serialize and show preview
    let toml_str = toml::to_string_pretty(&config).context("Failed to serialize config")?;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", toml_str);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    print!("\nSave this configuration? [Y/n] ");
    io::stdout().flush()?;
    let confirm = read_line()?;
    if confirm.trim().eq_ignore_ascii_case("n") {
        println!("Aborted.");
        return Ok(());
    }

    // Write config
    let config_dir = config::config_dir();
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create {}", config_dir.display()))?;

    let config_path = config_dir.join("config.toml");
    std::fs::write(&config_path, &toml_str)
        .with_context(|| format!("Failed to write {}", config_path.display()))?;

    println!("\n✅ Config saved to {}", config_path.display());
    println!("\nRun `satori` to start translating!");

    Ok(())
}

fn read_line() -> Result<String> {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;
    Ok(input)
}
