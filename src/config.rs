//! Preset TOML parsing and writing.
//!
//! Presets live in the `presets/` directory. The file is TOML: real comments,
//! real booleans and one array per list of keys, so it reads the way it is
//! meant to be edited by hand. Saving edits the parsed document in place,
//! which keeps every comment and any key this build does not know about.

use std::path::{Path, PathBuf};

use toml_edit::{Array, DocumentMut, Item, value};

use crate::lang::Language;
use crate::layout;

/// The commented template that ships inside the executable, so a lone
/// `keyoverlay.exe` can create its own preset on first run.
const TEMPLATE: &str = include_str!("../presets/default.toml");
pub const DEFAULT_PRESET: &str = "default.toml";
const PRESETS_DIR: &str = "presets";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundMode {
    #[default]
    Original,
    Stretch,
    Fill,
    Fit,
    Tile,
}

impl BackgroundMode {
    pub const ALL: [Self; 5] = [
        Self::Original,
        Self::Stretch,
        Self::Fill,
        Self::Fit,
        Self::Tile,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Stretch => "stretch",
            Self::Fill => "fill",
            Self::Fit => "fit",
            Self::Tile => "tile",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "original" => Some(Self::Original),
            "stretch" => Some(Self::Stretch),
            "fill" => Some(Self::Fill),
            "fit" => Some(Self::Fit),
            "tile" => Some(Self::Tile),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Original => 0,
            Self::Stretch => 1,
            Self::Fill => 2,
            Self::Fit => 3,
            Self::Tile => 4,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
    /// Keys to watch, in the order they are drawn.
    pub keys: Vec<String>,
    /// Parallel to `keys`; `""` means "keep the key name".
    pub display_keys: Vec<String>,
    pub key_size: i32,
    pub bar_speed: f32,
    pub margin: i32,
    pub outline_thickness: i32,
    pub fading: bool,
    pub key_counter: bool,
    pub background_image: String,
    pub background_mode: BackgroundMode,
    pub background_color: Color,
    pub key_color: Color,
    pub border_color: Color,
    pub bar_color: Color,
    pub font_color: Color,
    pub press_font_color: Color,
    pub max_fps: u32,
    /// UI language of the settings window.
    pub language: Language,
    /// Multiplier on top of the display's DPI for the settings window.
    pub ui_scale: f32,
    /// Layered window with per-pixel alpha, so the background can be transparent.
    pub transparent_background: bool,
    /// Let the mouse pass through the overlay to whatever is behind it.
    pub click_through: bool,
    pub always_on_top: bool,
}

impl Config {
    /// Adds a key with an unused letter as its binding, widening the overlay so
    /// the keys already there keep their spacing.
    pub fn add_key(&mut self) {
        let candidate = ('A'..='Z')
            .map(|letter| letter.to_string())
            .find(|letter| !self.keys.iter().any(|key| key == letter))
            .unwrap_or_else(|| "A".to_string());
        let step = self.key_step();
        self.keys.push(candidate);
        self.display_keys.push(String::new());
        self.window_width = self.window_width.saturating_add(step);
    }

    /// Removes a key, keeping the file consistent and giving back the room it
    /// took up.
    pub fn remove_key(&mut self, index: usize) {
        if index < self.keys.len() {
            let step = self.key_step();
            self.keys.remove(index);
            self.display_keys.truncate(self.keys.len());
            self.window_width = self
                .window_width
                .saturating_sub(step)
                .max(self.minimum_width());
        }
    }

    /// Room one key takes at the current spacing.
    fn key_step(&self) -> u32 {
        layout::key_step(
            self.keys.len() as u32,
            self.key_size,
            self.outline_thickness,
            self.margin,
            self.window_width,
        )
    }

    /// Width at which the keys sit edge to edge, with no spacing left to give
    /// back.
    fn minimum_width(&self) -> u32 {
        let width = (self.key_size + self.outline_thickness * 2).max(1) as u32;
        (self.margin.max(0) as u32 * 2).saturating_add(width * self.keys.len() as u32)
    }
}

pub fn ensure_presets(dir: &Path) -> Result<(), String> {
    let presets = dir.join(PRESETS_DIR);
    std::fs::create_dir_all(&presets)
        .map_err(|error| format!("Could not create {}: {error}", presets.display()))?;

    let default = presets.join(DEFAULT_PRESET);
    if !default.is_file() {
        let legacy = [dir.join("config.toml"), PathBuf::from("config.toml")]
            .into_iter()
            .find(|path| path.is_file());
        if let Some(legacy) = legacy {
            std::fs::copy(&legacy, &default).map_err(|error| {
                format!(
                    "Could not migrate {} to {}: {error}",
                    legacy.display(),
                    default.display()
                )
            })?;
        } else {
            std::fs::write(&default, TEMPLATE)
                .map_err(|error| format!("Could not write {}: {error}", default.display()))?;
        }
    }
    Ok(())
}

pub fn ensure_resources(dir: &Path) -> Result<(), String> {
    let resources = dir.join("Resources");
    std::fs::create_dir_all(&resources)
        .map_err(|error| format!("Could not create {}: {error}", resources.display()))
}
pub fn load(dir: &Path, file_name: &str) -> Result<Config, String> {
    ensure_presets(dir)?;
    ensure_resources(dir)?;
    let path = resolve(dir, file_name)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        // First run, or the file was deleted: leave the template behind so
        // there is something to edit, and carry on with it.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::write(&path, TEMPLATE)
                .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
            TEMPLATE.to_string()
        }
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };

    // A file saved by a Windows editor may carry a BOM.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let document = text
        .parse::<DocumentMut>()
        .map_err(|error| format!("{}: {error}", path.display()))?;

    let missing = |name: &str| format!("{}: there is no {name} key", path.display());
    let wrong = |name: &str, what: &str| format!("{}: {name} must be {what}", path.display());

    let mut keys = Vec::new();
    for (index, entry) in list(&document, "keys", &path)?.iter().enumerate() {
        let name = entry
            .as_str()
            .ok_or_else(|| wrong(&format!("keys[{index}]"), "a string"))?;
        keys.push(name.to_string());
    }
    let mut display_keys = Vec::new();
    if let Ok(entries) = list(&document, "display_keys", &path) {
        for entry in entries {
            display_keys.push(entry.as_str().unwrap_or_default().to_string());
        }
    }
    display_keys.resize(keys.len(), String::new());

    let config = Config {
        window_width: uint(&document, "window_width", &path)?,
        window_height: uint(&document, "window_height", &path)?,
        keys,
        display_keys,
        key_size: int(&document, "key_size", &path)?,
        bar_speed: float(&document, "bar_speed", &path)?,
        margin: int(&document, "margin", &path)?,
        outline_thickness: int(&document, "outline_thickness", &path)?,
        fading: boolean(&document, "fading", &path)?,
        key_counter: boolean(&document, "key_counter", &path)?,
        background_image: document
            .get("background_image")
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_string(),
        background_mode: document
            .get("background_mode")
            .and_then(Item::as_str)
            .and_then(BackgroundMode::parse)
            .unwrap_or_default(),
        background_color: color(&document, "background_color", &path)?,
        key_color: color(&document, "key_color", &path)?,
        border_color: color(&document, "border_color", &path)?,
        bar_color: color(&document, "bar_color", &path)?,
        font_color: color(&document, "font_color", &path)?,
        press_font_color: color(&document, "press_font_color", &path)?,
        max_fps: uint(&document, "max_fps", &path)?,
        // Absent or empty means "follow the Windows UI language".
        language: document
            .get("language")
            .and_then(Item::as_str)
            .and_then(Language::parse)
            .unwrap_or_else(Language::system),
        ui_scale: document
            .get("ui_scale")
            .and_then(Item::as_float)
            .map(|value| value as f32)
            .unwrap_or(1.0)
            .clamp(0.5, 4.0),
        transparent_background: flag(&document, "transparent_background"),
        click_through: flag(&document, "click_through"),
        always_on_top: flag(&document, "always_on_top"),
    };

    if config.keys.is_empty() {
        return Err(missing("keys"));
    }
    Ok(config)
}

pub fn save(dir: &Path, file_name: &str, config: &Config) -> Result<(), String> {
    ensure_presets(dir)?;
    let path = resolve(dir, file_name)?;
    // Saving into the template keeps every comment when the file is gone.
    let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| TEMPLATE.to_string());
    let mut document = existing
        .parse::<DocumentMut>()
        .map_err(|error| format!("{}: {error}", path.display()))?;

    set(&mut document, "keys", strings(&config.keys));
    set(&mut document, "display_keys", strings(&config.display_keys));
    set(
        &mut document,
        "window_width",
        value(config.window_width as i64),
    );
    set(
        &mut document,
        "window_height",
        value(config.window_height as i64),
    );
    set(&mut document, "key_size", value(config.key_size as i64));
    set(&mut document, "bar_speed", value(config.bar_speed as f64));
    set(&mut document, "margin", value(config.margin as i64));
    set(
        &mut document,
        "outline_thickness",
        value(config.outline_thickness as i64),
    );
    set(&mut document, "fading", value(config.fading));
    set(&mut document, "key_counter", value(config.key_counter));
    set(
        &mut document,
        "background_image",
        value(config.background_image.as_str()),
    );
    set(
        &mut document,
        "background_mode",
        value(config.background_mode.code()),
    );
    set(
        &mut document,
        "background_color",
        value(hex(config.background_color)),
    );
    set(&mut document, "key_color", value(hex(config.key_color)));
    set(
        &mut document,
        "border_color",
        value(hex(config.border_color)),
    );
    set(&mut document, "bar_color", value(hex(config.bar_color)));
    set(&mut document, "font_color", value(hex(config.font_color)));
    set(
        &mut document,
        "press_font_color",
        value(hex(config.press_font_color)),
    );
    set(&mut document, "max_fps", value(config.max_fps as i64));
    set(&mut document, "language", value(config.language.code()));
    set(&mut document, "ui_scale", value(config.ui_scale as f64));
    set(
        &mut document,
        "transparent_background",
        value(config.transparent_background),
    );
    set(&mut document, "click_through", value(config.click_through));
    set(&mut document, "always_on_top", value(config.always_on_top));

    std::fs::write(&path, document.to_string())
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}
pub fn create(dir: &Path, name: &str, config: &Config) -> Result<String, String> {
    let file_name = normalize_preset_name(name)?;
    ensure_presets(dir)?;
    let path = resolve(dir, &file_name)?;
    if path.exists() {
        return Err(format!("Preset already exists: {}", path.display()));
    }
    save(dir, &file_name, config)?;
    Ok(file_name)
}

pub fn delete(dir: &Path, file_name: &str) -> Result<(), String> {
    let file_name = normalize_preset_name(file_name)?;
    if file_name == DEFAULT_PRESET {
        return Err("The default preset cannot be deleted".to_string());
    }
    ensure_presets(dir)?;
    let path = resolve(dir, &file_name)?;
    std::fs::remove_file(&path)
        .map_err(|error| format!("Could not delete {}: {error}", path.display()))
}

/// Replaces a value while keeping the comments around it: the key's own decor
/// (the lines above) and the value's (a trailing comment on the same line).
fn set(document: &mut DocumentMut, name: &str, new: Item) {
    match document.get_mut(name) {
        Some(existing) => {
            let decor = existing.as_value().map(|value| value.decor().clone());
            let mut new = new;
            if let (Some(decor), Some(value)) = (decor, new.as_value_mut()) {
                *value.decor_mut() = decor;
            }
            *existing = new;
        }
        None => document[name] = new,
    }
}

fn strings(values: &[String]) -> Item {
    let mut array = Array::new();
    for value in values {
        array.push(value.as_str());
    }
    value(array)
}

pub fn list_presets(dir: &Path) -> Result<Vec<String>, String> {
    ensure_presets(dir)?;
    let presets = dir.join(PRESETS_DIR);
    let mut names = std::fs::read_dir(&presets)
        .map_err(|error| format!("Could not read {}: {error}", presets.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let path = entry.path();
            let extension = path.extension()?.to_str()?;
            if !extension.eq_ignore_ascii_case("toml") {
                return None;
            }
            Some(path.file_name()?.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    names.sort_by(|left, right| {
        (left != DEFAULT_PRESET)
            .cmp(&(right != DEFAULT_PRESET))
            .then_with(|| left.cmp(right))
    });
    Ok(names)
}

fn valid_preset_file_name(file_name: &str) -> bool {
    let candidate = Path::new(file_name);
    let is_file_name = candidate.file_name().and_then(|name| name.to_str()) == Some(file_name);
    let is_toml = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
    let has_stem = candidate
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| !stem.is_empty());
    let has_invalid_character = file_name.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    });
    let has_windows_trailing_space = file_name.ends_with(' ') || file_name.ends_with('.');
    is_file_name && is_toml && has_stem && !has_invalid_character && !has_windows_trailing_space
}

pub fn normalize_preset_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Preset name cannot be empty".to_string());
    }
    let file_name = if name.to_ascii_lowercase().ends_with(".toml") {
        name.to_string()
    } else {
        format!("{name}.toml")
    };
    if !valid_preset_file_name(&file_name) {
        return Err("Preset name must be a valid .toml file name".to_string());
    }
    Ok(file_name)
}

/// Presets are file names, never paths, and always use the `.toml` extension.
fn resolve(dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    if !valid_preset_file_name(file_name) {
        return Err(format!(
            "Preset name must be a .toml file name: {file_name}"
        ));
    }
    Ok(dir.join(PRESETS_DIR).join(file_name))
}

fn uint(document: &DocumentMut, name: &str, path: &Path) -> Result<u32, String> {
    let value = integer(document, name, path)?;
    u32::try_from(value).map_err(|_| {
        format!(
            "{}: {name} = {value} is not a positive number",
            path.display()
        )
    })
}

fn int(document: &DocumentMut, name: &str, path: &Path) -> Result<i32, String> {
    let value = integer(document, name, path)?;
    i32::try_from(value).map_err(|_| {
        format!(
            "{}: {name} = {value} does not fit in a number",
            path.display()
        )
    })
}

fn integer(document: &DocumentMut, name: &str, path: &Path) -> Result<i64, String> {
    document
        .get(name)
        .and_then(Item::as_integer)
        .ok_or_else(|| format!("{}: {name} must be a whole number", path.display()))
}

fn float(document: &DocumentMut, name: &str, path: &Path) -> Result<f32, String> {
    document
        .get(name)
        .and_then(Item::as_float)
        .map(|value| value as f32)
        .ok_or_else(|| format!("{}: {name} must be a number", path.display()))
}

fn boolean(document: &DocumentMut, name: &str, path: &Path) -> Result<bool, String> {
    document
        .get(name)
        .and_then(Item::as_bool)
        .ok_or_else(|| format!("{}: {name} must be true or false", path.display()))
}

fn flag(document: &DocumentMut, name: &str) -> bool {
    document.get(name).and_then(Item::as_bool).unwrap_or(false)
}

fn color(document: &DocumentMut, name: &str, path: &Path) -> Result<Color, String> {
    let text = document.get(name).and_then(Item::as_str).ok_or_else(|| {
        format!(
            "{}: {name} must be a colour like \"#RRGGBBAA\"",
            path.display()
        )
    })?;
    parse_color(text).ok_or_else(|| {
        format!(
            "{}: {name} = \"{text}\" is not a colour like \"#RRGGBBAA\"",
            path.display()
        )
    })
}

fn list<'a>(document: &'a DocumentMut, name: &str, path: &Path) -> Result<&'a Array, String> {
    document
        .get(name)
        .and_then(Item::as_array)
        .ok_or_else(|| format!("{}: there is no {name} list", path.display()))
}

/// `#RRGGBB` (opaque) or `#RRGGBBAA`.
fn parse_color(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    let number = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(Color {
            r: (number >> 16) as u8,
            g: (number >> 8) as u8,
            b: number as u8,
            a: 255,
        }),
        8 => Some(Color {
            r: (number >> 24) as u8,
            g: (number >> 16) as u8,
            b: (number >> 8) as u8,
            a: number as u8,
        }),
        _ => None,
    }
}

fn hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A label that needs the CJK fallback font: the tests use it so a display
    /// name keeps surviving the file as UTF-8, which is what display names are
    /// for in the first place.
    const CJK_LABEL: &str = "跳";

    fn sample() -> Config {
        Config {
            window_width: 240,
            window_height: 700,
            keys: vec!["Z".into(), "X".into()],
            display_keys: vec![String::new(), CJK_LABEL.into()],
            key_size: 70,
            bar_speed: 600.0,
            margin: 25,
            outline_thickness: 5,
            fading: true,
            key_counter: false,
            background_image: String::new(),
            background_mode: BackgroundMode::Original,
            background_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            key_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            border_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            bar_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 100,
            },
            font_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            press_font_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            max_fps: 60,
            language: Language::Zh,
            ui_scale: 1.0,
            transparent_background: false,
            click_through: false,
            always_on_top: false,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyoverlay-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_preset_is_created_from_the_shipped_template() {
        let dir = scratch("first-run");
        // A name that cannot exist in the working directory, so the lookup
        // lands on the preset path next to the executable.
        let file = format!("first-run-{}.toml", std::process::id());

        let config = load(&dir, &file).unwrap();
        assert!(
            dir.join("Resources").is_dir(),
            "first run must create the Resources directory"
        );
        assert!(
            dir.join("presets").is_dir(),
            "first run must create the presets directory"
        );
        assert!(
            dir.join("presets").join(DEFAULT_PRESET).is_file(),
            "first run must create default.toml"
        );

        let written = std::fs::read_to_string(dir.join("presets").join(&file)).unwrap();
        assert!(
            written.lines().any(|line| line.starts_with('#')),
            "the file created on first run has nothing to explain it: {written}"
        );
        assert_eq!(config.keys, vec!["Z".to_string(), "X".to_string()]);
        // and the next start reads back what this one used
        let reloaded = load(&dir, &file).unwrap();
        assert_eq!(reloaded.window_width, config.window_width);
        assert_eq!(reloaded.key_size, config.key_size);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_config_is_migrated_to_default_preset() {
        let dir = scratch("legacy-migration");
        let legacy = TEMPLATE.replace("window_width = 240", "window_width = 333");
        std::fs::write(dir.join("config.toml"), &legacy).unwrap();

        let config = load(&dir, DEFAULT_PRESET).unwrap();

        assert_eq!(config.window_width, 333);
        assert_eq!(
            std::fs::read_to_string(dir.join("presets").join(DEFAULT_PRESET)).unwrap(),
            legacy
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn preset_names_cannot_escape_the_presets_directory() {
        let dir = scratch("preset-name");

        assert!(load(&dir, "../outside.toml").is_err());
        assert!(load(&dir, "not-a-preset.txt").is_err());
        assert!(!dir.join("outside.toml").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preset_listing_only_returns_toml_files_with_default_first() {
        let dir = scratch("list-presets");
        ensure_presets(&dir).unwrap();
        std::fs::write(dir.join("presets").join("mania.toml"), TEMPLATE).unwrap();
        std::fs::write(dir.join("presets").join("notes.txt"), "ignore").unwrap();
        std::fs::create_dir(dir.join("presets").join("nested.toml")).unwrap();

        assert_eq!(
            list_presets(&dir).unwrap(),
            vec!["default.toml".to_string(), "mania.toml".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn presets_can_be_created_with_a_normalized_name_and_deleted() {
        let dir = scratch("create-delete");
        let config = sample();

        let created = create(&dir, "raid", &config).unwrap();
        assert_eq!(created, "raid.toml");
        assert_eq!(load(&dir, &created).unwrap().keys, config.keys);
        assert!(create(&dir, "raid.toml", &config).is_err());
        assert!(delete(&dir, &created).is_ok());
        assert!(!dir.join("presets").join(&created).exists());
        assert!(delete(&dir, DEFAULT_PRESET).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preset_name_normalization_rejects_paths_and_empty_names() {
        assert_eq!(normalize_preset_name("  raid  ").unwrap(), "raid.toml");
        assert_eq!(normalize_preset_name("raid.TOML").unwrap(), "raid.TOML");
        assert!(normalize_preset_name("").is_err());
        assert!(normalize_preset_name("../raid").is_err());
        assert!(normalize_preset_name("raid?.toml").is_err());
    }

    #[test]
    fn background_mode_defaults_for_old_files_and_round_trips() {
        let dir = scratch("background-mode");
        let file = "background.toml";
        ensure_presets(&dir).unwrap();
        let old_template = TEMPLATE.replace(
            "background_mode = \"original\" # original, stretch, fill, fit, or tile\n",
            "",
        );
        std::fs::write(dir.join("presets").join(file), old_template).unwrap();

        let old_config = load(&dir, file).unwrap();
        assert_eq!(old_config.background_mode, BackgroundMode::Original);

        let mut config = sample();
        config.background_mode = BackgroundMode::Fill;
        save(&dir, file, &config).unwrap();
        let reloaded = load(&dir, file).unwrap();
        assert_eq!(reloaded.background_mode, BackgroundMode::Fill);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_keeps_comments_and_unknown_keys() {
        let dir = scratch("save-comments");
        let file = "comments.toml";
        ensure_presets(&dir).unwrap();
        let mut document = TEMPLATE.to_string();
        document.push_str("\n# a newer build wrote this\nfuture_option = 42\n");
        std::fs::write(dir.join("presets").join(file), &document).unwrap();

        save(&dir, file, &sample()).unwrap();

        let written = std::fs::read_to_string(dir.join("presets").join(file)).unwrap();
        assert!(
            written.contains("# distance from the window edge"),
            "the template's comments must survive: {written}"
        );
        assert!(
            written.contains("future_option = 42"),
            "unknown keys must survive: {written}"
        );
        assert!(
            written.contains("key_size = 70"),
            "managed values must be written: {written}"
        );
        assert!(
            written.contains("keys = [\"Z\", \"X\"]"),
            "the key list must be written: {written}"
        );
        // and the result still loads, which is what the app does next time
        let reloaded = load(&dir, file).unwrap();
        assert_eq!(reloaded.keys, vec!["Z".to_string(), "X".to_string()]);
        assert_eq!(reloaded.display_keys[1], CJK_LABEL);
        assert_eq!(reloaded.bar_color.a, 100);
        assert_eq!(reloaded.language, Language::Zh);

        // a second save is a no-op, so the file stays stable
        save(&dir, file, &reloaded).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("presets").join(file)).unwrap(),
            written
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_with_a_custom_label_loads() {
        let dir = scratch("labels");
        let file = "labels.toml";
        ensure_presets(&dir).unwrap();
        let text = TEMPLATE.replace(
            "keys = [\"Z\", \"X\"]\ndisplay_keys = [\"\", \"\"]",
            &format!("keys = [\"Z,{CJK_LABEL}\", \"MouseLeft\"]\ndisplay_keys = [\"\", \"click\"]"),
        );
        std::fs::write(dir.join("presets").join(file), text).unwrap();

        let config = load(&dir, file).unwrap();
        assert_eq!(config.keys.len(), 2);
        assert_eq!(config.keys[0], format!("Z,{CJK_LABEL}"));
        assert_eq!(config.display_keys[1], "click");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_colour_must_be_hex() {
        let dir = scratch("colour");
        let file = "colour.toml";
        ensure_presets(&dir).unwrap();
        let text = TEMPLATE.replace(
            "background_color = \"#000000FF\"",
            "background_color = \"black\"",
        );
        std::fs::write(dir.join("presets").join(file), text).unwrap();

        let Err(error) = load(&dir, file) else {
            panic!("a colour that is not hex should be rejected");
        };
        assert!(
            error.contains("background_color"),
            "the error should name the key: {error}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_key_makes_room_instead_of_squeezing() {
        let mut config = sample();
        let before = spacing(&config);
        config.add_key();
        assert_eq!(config.keys.len(), 3);
        let after = spacing(&config);
        assert!(
            (after - before).abs() < 1.5,
            "keys moved from {before} apart to {after}: the overlay did not grow"
        );
    }

    #[test]
    fn remove_key_gives_the_room_back() {
        let mut config = sample();
        config.add_key();
        let width_before = config.window_width;
        let spacing_before = spacing(&config);
        config.remove_key(2);
        assert_eq!(config.keys.len(), 2);
        assert!(
            config.window_width < width_before,
            "window stayed {} wide after a key was removed",
            config.window_width
        );
        assert!(
            (spacing(&config) - spacing_before).abs() < 1.5,
            "the keys left behind moved"
        );
    }

    /// Distance between the first two squares, i.e. what the user sees as the
    /// gap between keys.
    fn spacing(config: &Config) -> f32 {
        let squares = crate::layout::create_squares(
            config.keys.len() as u32,
            config.outline_thickness,
            config.key_size,
            config.margin,
            config.window_width,
            1.0,
        );
        squares[1].x - squares[0].x
    }
}
