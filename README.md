# KeyOverlay (Rust)

A key overlay for osu! streaming: shows your press keys and a bar travelling up
for every hit.

This is a from-scratch **Rust** reimplementation of [Blondazz/KeyOverlay], the
original SFML/.NET application, and is licensed the same way (GPL-3.0). It is
Windows only, needs no .NET and no CSFML, and builds to a single executable.

[Blondazz/KeyOverlay]: https://github.com/Blondazz/KeyOverlay

```
cargo build --release
target/release/keyoverlay.exe
```

That executable is the whole program - the console font and the config template
are compiled into it - so it is the only file you have to copy. On the first run
there is no `config.toml` yet, so it writes the commented template next to itself
and starts with those defaults.

`config.toml` documents every option, and the same file can be edited in the
settings window. It is looked up next to the executable first
(as it is shipped, and as the release archive unpacks it) and then in the working
directory, so `cargo run` and `target/release/keyoverlay.exe` both find the file
in the project root. A path given as the first argument is resolved the same way.

## Settings window

`Ctrl+Alt+K` (or clicking the overlay) opens it. Every edit is applied to the
running overlay immediately; `Save` writes the file, `Reload`
reads it back, `Esc`/`Close` hides the window. The left pane is a live preview
with a checkerboard behind it, so a transparent background is visible as such.
Key bindings are captured by clicking a binding and pressing the key or mouse
button to use. Config lines this build does not know are preserved on save.

| Section | Controls |
|---|---|
| Keys | bindings, display names, add/remove keys |
| Layout and animation | key size, margin, outline, bar speed, window size, max FPS, key counter |
| Appearance | fading overlay, and a colour editor (r,g,b,a) for background, key, border, bar, font and pressed font |
| Window and streaming | transparent background, click-through, always on top, background image |
| Interface | language and UI scale (lower left) |

### Language

English and Chinese; the button in the top right switches instantly and `Save`
remembers it (`language=en` / `zh`, empty = follow the Windows UI language).

Chinese needs a CJK face. The bundled Consolas has none, so the UI and the
overlay fall back to a system font (`msyh.ttc`, then `Deng.ttf`, `simhei.ttf`,
...). Without any of them the settings window stays in English; Chinese
`displayKey` text then has no glyphs to draw.

### Size

The window is laid out from one scale factor: a base 1.1 on top of the display's
DPI, so a 150% display draws 1.65x. `UI scale` multiplies that, relative to the
display (so the value still makes sense after switching monitors), and is stored
as `ui_scale` (default 1.0, accepted range 0.5-4.0). `Reset` puts it back to 1.00.
The window grows and shrinks with its content, is centered when it opens and never
grows larger than the desktop.

Every slider shows its value in a small box that can be typed into: click it, type
a number and press Enter (or click away) to apply it, Escape to abandon it. The
slider itself stays for coarse dragging.

The overlay window itself stays in real pixels: `window_width`/`window_height` are
the captured output size, so the stream keeps exactly the resolution written in
the configuration instead of a DPI-upscaled, blurry copy.

## Streaming and transparency

The overlay is an ordinary opaque window by default, so the usual setup applies:
capture it in OBS and chroma key `background_color` out.

Two extra options change that:

| Option | Effect |
|---|---|
| `transparent_background = true` | Presents through a layered window (`UpdateLayeredWindow`), so the background is really transparent and no chroma key is needed. The fading trails fade into transparency as well. The alpha in `background_color` is what becomes transparent (0 = fully transparent, 1-254 = a see-through panel); the settings window's toggle sets it to 0 for you. Works with Display Capture, since the desktop compositor draws it. Whether Window Capture keeps the alpha depends on OBS's capture method. Game Capture only captures the game itself, so no external overlay can be composited into it either way. |
| `click_through = true` | The mouse ignores the overlay and clicks reach whatever is behind it. Implies the layered window. While it is on, the overlay cannot be clicked to open the settings - use `Ctrl+Alt+K`. |
| `always_on_top = true` | Keeps the overlay above other windows, which matters when it is captured as part of the desktop. |

With a transparent background, Windows hit-tests a layered window per pixel, so
only the drawn keys and labels can be pressed - a click on an empty part of the
overlay belongs to whatever is behind it. Dragging works there too (grab a key),
but the way to reach the settings from an empty area is `Ctrl+Alt+K`.

The three options above are `true`/`false` in the file, like `fading` and
`key_counter`.

## Moving the overlay

Press anywhere on the overlay and move to drag it - a transparent overlay has no
title bar at all, and the opaque one is easier to grab by its body than by the
title bar above it. Pressing without moving (a click) opens the settings instead.
With `click_through = true` the overlay receives no mouse input at all, so neither
works and the position comes from the config file.

## How it compares to the original

The rendering rules are a 1:1 port of the C# implementation - key geometry on
the 480x960 canvas, outlines drawn outside the shape, the bar growth and travel
per frame, the 255-strip fading overlay, SFML's text layout and origin - and the
configuration file is TOML rather than the original's `key=value` lines, so it
can carry comments, real booleans, one list per group of keys, and `#RRGGBBAA`
colours:

```toml
keys = ["Z", "X"]           # "Z,跳" shows 跳 instead of the key name
key_size = 70               # height of a square, in pixels
fading = true               # squares fade back after a hit
background_color = "#000000FF"
transparent_background = false
```

An existing `config.txt` is not read: both the value names and the way values
are written changed, so it is easiest to start from the new template and set the
options once in the settings window.

Deliberate differences:

- A single key divided by zero in the original (nothing was drawn); the one key
  is centered here.
- A missing `config.toml` is created from the template on startup instead of
  failing with an error.
- An invalid key name writes `keyErrorMessage.txt` and exits, instead of writing
  the file and then crashing on an index error.
- Missing configuration values and missing background images write
  `errorMessage.txt` naming the offending key or path, then exit.
- `+ Add key` in the settings window widens `window_width` by one key width plus
  the spacing that key gets, and `x` gives that room back, so adding keys never
  squeezes the ones already there.
- The background image is resolved next to the executable, not in the working
  directory.
- The font is rasterized as-is: SFML additionally emboldens it, so strokes here
  are about 1px thinner at the default size.
- Chinese `displayKey` text is drawn (the original had no CJK glyphs at all).
- Configuration files may contain `#`/`;` comments; the original parser cannot
  read those, so strip them if you point the C# build at this file.

## Licence

GPL-3.0, see [LICENSE](LICENSE), matching the original project by Blondazz which
this is derived from. `assets/consolab.ttf` is the same Consolas Bold face the
original ships.
