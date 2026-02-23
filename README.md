# Satori (悟り)

Wayland screen translator for manga and visual novels, designed for the Niri compositor.

Satori captures a user-selected screen region or reads an image file, sends it to a Vision LLM for OCR and context-aware translation, and displays the result in terminal or as a floating overlay window.

## Features

- **Vision LLM translation** — OCR + translation in one step via OpenAI-compatible APIs or local Ollama
- **Dual output modes** — terminal (stdout) or floating Wayland overlay
- **Image file input** — translate any PNG/JPG/WebP file directly from the command line
- **Wayland-native** — uses `slurp` for region selection, `grim` for screenshots, `gtk4-layer-shell` for the overlay
- **In-memory processing** — screenshots never touch disk
- **Multi-provider** — supports OpenRouter, Google Gemini, OpenAI, Anthropic Claude, xAI Grok, DeepSeek, Ollama, and any OpenAI-compatible endpoint
- **i18n** — auto-detects system locale for default target language
- **Translation styles** — context-aware prompts for manga, visual novels, games, or general use
- **Interactive setup** — `satori init` wizard to create your config
- **Shell completions** — Bash, Zsh, Fish via `satori completions`
- **Translation history** — browse, search, and navigate past translations
- **Overlay controls** — screenshot, copy, prev/next buttons right in the overlay
- **Optional GUI** — compile without GTK for lightweight server/terminal-only usage
- **Customizable overlay** — position, font, colors, opacity
- **Adaptive sizing** — overlay window auto-sizes to content, resizable by dragging edges
- **Position memory** — overlay remembers drag position between sessions (dynamic mode)
- **Low-memory mode** — optional Cairo software renderer to reduce GPU memory usage

## Requirements

- **Wayland compositor** — overlay mode works on any wlroots-based compositor:
  - [Niri](https://github.com/YaLTeR/niri), [Hyprland](https://hyprland.org/), [Sway](https://swaywm.org/), [River](https://isaacfreund.com/software/river/), and others
  - KDE Plasma and GNOME support planned for future releases
- `slurp` and `grim` — for screen capture (not needed for image file translation)
- GTK4, libadwaita, gtk4-layer-shell — for overlay mode (optional compile-time feature)
- **Terminal-only mode** has no display server requirements — works on headless servers

## Installation

### From source

```sh
# Full build (with GUI overlay)
cargo install --path .

# Terminal-only build (no GTK dependencies needed)
cargo install --path . --no-default-features
```

### NixOS / Nix

```sh
nix run github:oodenX/satori
# or add to flake inputs
nix profile install github:oodenX/satori
```

### Deb / RPM

Download from [GitHub Releases](https://github.com/oodenX/satori/releases).

### Arch Linux

A `PKGBUILD` is provided in the repository under `pkg/PKGBUILD`:

```sh
cd pkg
makepkg -si
```

## Quick Start

```sh
satori init              # interactive setup wizard
satori                   # capture screen region → overlay
satori image.png         # translate image file → terminal
satori image.png -o overlay  # translate image file → overlay
```

## Usage

```
satori [OPTIONS] [IMAGE]           Translate (screenshot or file)
satori init                        Interactive configuration wizard
satori completions <SHELL>         Generate shell completions (bash, zsh, fish)

Arguments:
  [IMAGE]  Image file to translate (png, jpg, webp). If omitted, captures screen.

Options:
  -t, --target <LANG>              Target language (overrides config)
  -s, --style <STYLE>              Translation style: general, manga, novel, game
  -o, --output <MODE>              Output mode: terminal, overlay
  -p, --pos <POSITION>             Overlay position: center, top-left, top-right,
                                   bottom-left, bottom-right
      --profile <NAME>             Config profile to use
      --color <COLOR>              Text color, CSS format (overlay only)
      --font <FONT>                Font family (overlay only)
      --background-color <COLOR>   Background color, CSS format (overlay only)
      --background-opacity <FLOAT> Background opacity 0.0-1.0 (overlay only)
  -h, --help                       Show help
  -V, --version                    Show version
```

### Output mode logic

| Input | Default output | Description |
|-------|---------------|-------------|
| No image | overlay | Screenshot → floating overlay (requires GUI) |
| Image file | terminal | File → stdout |
| `--output terminal` | terminal | Force terminal output |
| `--output overlay` | overlay | Force overlay output (requires GUI) |

### Examples

```sh
# Capture and translate a screen region (overlay)
satori

# Translate an image file (terminal output)
satori manga_page.png

# Translate to English with manga style
satori screenshot.png --target English --style manga

# Use overlay positioned at bottom-right with custom styling
satori --pos bottom-right --font "Noto Sans CJK SC" --color "#ffffff"

# Use a specific config profile
satori --profile deepseek

# Generate Zsh completions
satori completions zsh > ~/.zfunc/_satori
```

### Shell Completions

```sh
# Bash
satori completions bash > ~/.local/share/bash-completion/completions/satori

# Zsh
satori completions zsh > ~/.zfunc/_satori

# Fish
satori completions fish > ~/.config/fish/completions/satori.fish
```

### Overlay Controls

- `Esc` or click close button: dismiss overlay
- `Ctrl+C`: copy translation to clipboard
- `📷` button: take a new screenshot
- `◀` / `▶` buttons: browse translation history
- Drag window to move, drag right/bottom edge to resize

### Compositor keybindings

**Niri** (`~/.config/niri/config.kdl`):
```kdl
binds {
    Mod+S { spawn "satori"; }
}
```

**Hyprland** (`~/.config/hypr/hyprland.conf`):
```ini
bind = $mainMod, S, exec, satori
```

**Sway** (`~/.config/sway/config`):
```
bindsym $mod+s exec satori
```

**River** (`~/.config/river/init`):
```sh
riverctl map normal Super S spawn satori
```

## Configuration

Run `satori init` for interactive setup, or create `~/.config/satori/config.toml` manually:

```toml
[settings]
active_profile = "openrouter"
target_lang = "简体中文"
translation_style = "manga"
ui_opacity = 0.9
last_pos = "bottom-right"
font = "Noto Sans CJK SC"
color = "#ffffff"
background_color = "#1a1a2e"
background_opacity = 0.85
position_mode = "dynamic"
# renderer = "cairo"  # uncomment for lower memory usage

[profiles.openrouter]
driver = "openai_compatible"
api_key_env = "SATORI_OPENROUTER_KEY"
base_url = "https://openrouter.ai/api/v1"
model = "google/gemini-2.5-flash"
timeout_sec = 15

[profiles.ollama]
driver = "ollama"
base_url = "http://localhost:11434"
model = "llava"
```

Set your API key:

```sh
export SATORI_OPENROUTER_KEY="sk-your-key-here"
```

### Settings

| Field | Description | Default |
|-------|-------------|---------|
| `active_profile` | Name of the profile to use | *(required)* |
| `target_lang` | Target translation language | Auto-detected from locale |
| `translation_style` | `general`, `manga`, `novel`, or `game` | `general` |
| `ui_opacity` | Overlay window opacity (0.0–1.0) | `0.9` |
| `last_pos` | Last overlay position (persisted automatically) | `center` |
| `font` | Default font family for overlay | System default |
| `color` | Default text color for overlay | System default |
| `background_color` | Default background color for overlay | System default |
| `background_opacity` | Default background opacity for overlay | `0.9` |
| `position_mode` | `dynamic` (remember position) or `fixed` (reset each time) | `dynamic` |
| `renderer` | GTK renderer: `auto` (GPU) or `cairo` (software, lower memory) | `auto` |

### Profile fields

| Field | Description |
|-------|-------------|
| `driver` | `openai_compatible` or `ollama` |
| `api_key_env` | Environment variable name containing the API key (cloud only) |
| `base_url` | API endpoint URL |
| `model` | Model identifier |
| `timeout_sec` | Request timeout in seconds (default: 15) |

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `gui` | ✅ | GTK4 overlay, screenshot capture, layer-shell |

Build without GUI for a lightweight terminal-only binary:

```sh
cargo build --release --no-default-features
```

This removes the dependency on GTK4, libadwaita, and gtk4-layer-shell. The resulting binary only supports `satori <IMAGE>` with terminal output.

## Building

```sh
cargo build                          # debug build (with GUI)
cargo build --release                # release build
cargo build --no-default-features    # terminal-only build
cargo test                           # run tests
cargo clippy -- -D warnings          # lint
```

## License

[MIT](LICENSE)
