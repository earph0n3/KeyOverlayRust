# KeyOverlay (Rust)

[![Build](https://github.com/earph0n3/KeyOverlayRust/actions/workflows/build.yml/badge.svg)](https://github.com/earph0n3/KeyOverlayRust/actions/workflows/build.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)

**KeyOverlay** is a lightweight, native Windows overlay for rhythm games such as
osu!, but it is not tied to a specific title or use case. It shows live
keyboard and mouse presses, hit bars, custom labels, and optional hit counters
in a compact window for play, recording, or any other setup that needs a visual
input display.

[简体中文](README.zh-CN.md)

> Windows only. This project is a Rust rewrite of
> [Blondazz/KeyOverlay](https://github.com/Blondazz/KeyOverlay), with TOML
> presets and a built-in settings window.

## Highlights

- Live keyboard and mouse input visualization.
- Animated hit bars, optional fading, and per-key hit counters.
- Settings window with a live preview; changes apply immediately.
- Global `Ctrl+Alt+K` shortcut to open settings while another application has
  focus.
- Transparent layered mode, click-through mode, and always-on-top mode.
- Custom key labels, RGBA colors, window dimensions, margins, animation speed,
  and frame-rate control.
- English and Chinese interface, with DPI-aware UI scaling.
- Self-contained executable: the font, icon, and first-run preset template
  are embedded in the binary.
- Presets live in `presets/*.toml`; saves preserve comments and options
  unknown to this build.

## Requirements

- Windows.
- Rust 1.88 or newer with Cargo.

The application does not require .NET or CSFML. Cross-platform builds are not
supported by this version.

## Build and run

```powershell
git clone https://github.com/earph0n3/KeyOverlayRust.git
cd KeyOverlayRust
cargo build --release --locked
.\target\release\keyoverlay.exe
```

For development, run the debug build from the repository root:

```powershell
cargo run --locked
```

The release executable is `target/release/keyoverlay.exe`. On first run, the
program creates `presets/` and `Resources/` beside the executable, writes the
embedded template as `presets/default.toml`, and starts with that preset.

To use another preset, put a file named `xxx.toml` in `presets/`, then pass its
file name as the first argument:

```powershell
.\target\release\keyoverlay.exe mania.toml
```

The settings window lists every `*.toml` file in `presets/`; selecting one loads
it immediately when there are no unsaved edits. If the current preset has
unsaved changes, the window asks whether to save, discard, or cancel before
switching. An old root-level `config.toml` is migrated to
`presets/default.toml` when the preset directory is first created.

## Configure the overlay

Press **`Ctrl+Alt+K`** to open the settings window. Clicking the overlay also
opens it unless `click_through` is enabled.

| Action | Behavior |
| --- | --- |
| Bind a key | Click a binding, then press a keyboard key or mouse button. |
| Edit a label | Type into the label field next to the binding; leave it empty to use the key name. |
| Apply changes | The overlay and preview update immediately. |
| Select a preset | Choose a saved `*.toml` file; unsaved edits prompt before switching. |
| Create a preset | Click `New`, enter a name, and click `Create`; `.toml` is added automatically when omitted. |
| Delete a preset | Click `Delete` to remove the selected custom preset; `default.toml` is protected. |
| Save / Reload | `Save preset` writes the selected TOML file; `Reload` reads it back from disk. |
| Close | `Esc` or `Close` hides the settings window. |
| Move the overlay | Press and drag the overlay. A click without moving opens settings. |

When `click_through = true`, the overlay receives no mouse input. Use the
keyboard shortcut and configure its position in the selected preset.

## Presets
`presets/default.toml` is the commented template containing every supported
option. Every preset is a file named `xxx.toml` inside `presets/`. The settings
window can create a preset from the current live configuration with `New`, and
`Delete` removes the selected custom preset. The `Save preset` button writes
the currently selected file.
The most important fields are:

```toml
keys = ["Z", "X", "mLeft"]
display_keys = ["", "", "M1"]

key_size = 70
window_width = 240
window_height = 700
bar_speed = 600.0
fading = true
key_counter = false
max_fps = 60                 # 0 = uncapped

background_color = "#000000FF"
key_color = "#00000000"
border_color = "#FFFFFFFF"
bar_color = "#FFFFFF64"
font_color = "#FFFFFFFF"
press_font_color = "#FFFFFFFF"
background_image = ""
background_mode = "original" # original, stretch, fill, fit, or tile

transparent_background = false
click_through = false
always_on_top = false
language = ""               # "en", "zh", or "" for the Windows UI language
ui_scale = 1.0
```
The old C# build's `config.txt` is not compatible with this rewrite. Start from
`presets/default.toml` instead. Saving through the settings window keeps
comments and unknown TOML keys in the selected preset.

`keys` accepts the keyboard names used by the application, including letters,
function keys, modifiers, navigation keys, and mouse names such as `mLeft`,
`mRight`, `mMiddle`, `mXButton1`, and `mXButton2`. `display_keys` is an
optional parallel list of labels; an empty entry keeps the key's normal name.
Colors use `#RRGGBB` or `#RRGGBBAA`.


`background_mode` controls how the selected image is drawn:

| Value | Behavior |
| --- | --- |
| `original` | Draw at the original size from the top-left corner. |
| `stretch` | Stretch independently to the full window size. |
| `fill` | Preserve the aspect ratio and crop the overflow. |
| `fit` | Preserve the aspect ratio and leave `background_color` around the image. |
| `tile` | Repeat the image at its original size. |

## Transparency and window behavior

The default window is opaque. If you use the overlay in a recording or screen
capture, remove `background_color` with the capture tool's chroma key when
needed.

For a compositor-based setup:

- `transparent_background = true` enables a layered window with per-pixel
  alpha, so a chroma key is not required.
- `click_through = true` lets mouse clicks pass to the window behind the
  overlay. It also implies layered rendering.
- `always_on_top = true` keeps the overlay above other windows.

Display Capture can include a transparent overlay because Windows composites it
into the desktop. Whether Window Capture preserves the alpha channel depends on
the capture method. Game Capture captures the game itself, not an external
overlay; use a desktop or window capture mode when the overlay must be visible.

## Background images

The first run creates an empty `Resources/` directory beside the executable.
Place PNG or JPEG files there. The settings window scans that directory and
shows the available files in a list; click the current file to choose another
one, or choose `(none)` to disable the background.

The selected file is stored in `background_image`:

```toml
background_image = "keyboard.png"
background_mode = "fill"
```

The image is rendered according to `background_mode`. A missing or unreadable
background image is skipped and reported without preventing the overlay from
starting.

## Troubleshooting

- Configuration and startup errors are written to `errorMessage.txt` beside the
  executable.
- Invalid key names are written to `keyErrorMessage.txt`.
- If `Ctrl+Alt+K` does not open settings, another application may already own
  the global shortcut.
- Chinese UI text and custom labels require a CJK font available on Windows;
  the bundled Consolas font does not contain CJK glyphs.

## Development

The CI workflow runs the same checks used by the project:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

This project is a from-scratch Rust rewrite of
[Blondazz/KeyOverlay](https://github.com/Blondazz/KeyOverlay).

The current implementation was created with AI-assisted development. CI covers
formatting, Clippy, tests, and the release build, but runtime behavior should
still be verified on the Windows and rhythm-game setup where you plan to use it.
