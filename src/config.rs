//! `config.txt` parsing and writing - port of `AppWindow.ReadConfig` and
//! `CreateItems.CreateColor`.
//!
//! Behaviour kept from the original:
//! * the file is read next to the executable (or the first CLI argument, also
//!   resolved next to the executable),
//! * a line is `name=value`, the value ends at the second `=` (`Split("=")[1]`),
//! * names and string values are compared verbatim (`fading=yes`),
//! * numbers/colours tolerate surrounding whitespace (`int.Parse` behaviour),
//! * duplicate or missing names and malformed values abort startup.
//!
//! The settings window writes this file back. It stays parseable by the
//! original C# build: `name=value` only, no comments, no blank lines, and every
//! name the C# version reads is always written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::lang::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
    pub key_amount: u32,
    /// `key1..keyN`, in order.
    pub keys: Vec<String>,
    /// `displayKey1..keyN` (`""` means "keep the key name").
    pub display_keys: Vec<String>,
    pub key_size: i32,
    pub bar_speed: f32,
    pub margin: i32,
    pub outline_thickness: i32,
    pub fading: bool,
    pub key_counter: bool,
    pub background_image: String,
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
    /// Config lines this build does not manage, kept verbatim so saving does not
    /// throw away settings written by another version.
    extras: Vec<(String, String)>,
}

impl Config {
    /// The configuration as ordered `name`/`value` pairs. This is the single
    /// source of truth for what the file contains.
    fn entries(&self) -> Vec<(String, String)> {
        let mut entries: Vec<(String, String)> = Vec::with_capacity(32 + self.extras.len());

        let mut push = |name: String, value: String| entries.push((name, value));
        push("keyAmount".into(), self.key_amount.to_string());
        for (index, key) in self.keys.iter().enumerate() {
            push(format!("key{}", index + 1), key.clone());
        }
        for (index, display) in self.display_keys.iter().enumerate() {
            push(format!("displayKey{}", index + 1), display.clone());
        }
        push("keyCounter".into(), yes_no(self.key_counter));
        push("windowHeight".into(), self.window_height.to_string());
        push("windowWidth".into(), self.window_width.to_string());
        push("keySize".into(), self.key_size.to_string());
        push("barSpeed".into(), format_float(self.bar_speed));
        push("margin".into(), self.margin.to_string());
        push(
            "outlineThickness".into(),
            self.outline_thickness.to_string(),
        );
        push("fading".into(), yes_no(self.fading));
        push("backgroundColor".into(), color_text(self.background_color));
        push("keyColor".into(), color_text(self.key_color));
        push("borderColor".into(), color_text(self.border_color));
        push("barColor".into(), color_text(self.bar_color));
        push("fontColor".into(), color_text(self.font_color));
        push("pressFontColor".into(), color_text(self.press_font_color));
        push("backgroundImage".into(), self.background_image.clone());
        push("maxFPS".into(), self.max_fps.to_string());
        push("language".into(), self.language.code().to_string());
        push("uiScale".into(), format_float(self.ui_scale));
        push(
            "transparentBackground".into(),
            yes_no(self.transparent_background),
        );
        push("clickThrough".into(), yes_no(self.click_through));
        push("alwaysOnTop".into(), yes_no(self.always_on_top));
        for (name, value) in &self.extras {
            push(name.clone(), value.clone());
        }
        entries
    }

    /// Adds a key with an unused letter as its binding.
    pub fn add_key(&mut self) {
        let candidate = ('A'..='Z')
            .map(|letter| letter.to_string())
            .find(|letter| !self.keys.iter().any(|key| key == letter))
            .unwrap_or_else(|| "A".to_string());
        self.keys.push(candidate);
        self.display_keys.push(String::new());
        self.key_amount += 1;
    }

    /// Removes a key, keeping the file consistent.
    pub fn remove_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
            self.display_keys.remove(index);
            self.key_amount = self.keys.len() as u32;
        }
    }
}

pub fn load(dir: &Path, file_name: &str) -> Result<Config, String> {
    let path = resolve(dir, file_name);
    let bytes =
        std::fs::read(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|e| format!("{} is not valid UTF-8: {e}", path.display()))?;
    // config.txt ships with a UTF-8 BOM.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    let mut entries: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        // Blank lines and `#`/`;` comments are ignored, so the shipped
        // config.txt can document itself.
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let mut parts = line.splitn(3, '=');
        let name = parts.next().unwrap_or_default();
        let Some(value) = parts.next() else {
            return Err(format!("{}: line without '=': {line}", path.display()));
        };
        if entries.insert(name, value).is_some() {
            return Err(format!(
                "{}: duplicate config key \"{name}\"",
                path.display()
            ));
        }
    }

    let get = |name: &str| -> Result<&str, String> {
        entries
            .get(name)
            .copied()
            .ok_or_else(|| format!("{}: missing config key \"{name}\"", path.display()))
    };

    let window_width = uint(get("windowWidth")?, "windowWidth")?;
    let window_height = uint(get("windowHeight")?, "windowHeight")?;
    let key_amount = uint(get("keyAmount")?, "keyAmount")?;

    let mut keys = Vec::with_capacity(key_amount as usize);
    let mut display_keys = Vec::with_capacity(key_amount as usize);
    for index in 1..=key_amount {
        keys.push(get(&format!("key{index}"))?.to_string());
        display_keys.push(
            entries
                .get(format!("displayKey{index}").as_str())
                .copied()
                .unwrap_or_default()
                .to_string(),
        );
    }

    let config = Config {
        window_width,
        window_height,
        key_amount,
        keys,
        display_keys,
        key_size: int(get("keySize")?, "keySize")?,
        bar_speed: float(get("barSpeed")?, "barSpeed")?,
        margin: int(get("margin")?, "margin")?,
        outline_thickness: int(get("outlineThickness")?, "outlineThickness")?,
        fading: get("fading")? == "yes",
        key_counter: get("keyCounter")? == "yes",
        background_image: get("backgroundImage")?.to_string(),
        background_color: color(get("backgroundColor")?, "backgroundColor")?,
        key_color: color(get("keyColor")?, "keyColor")?,
        border_color: color(get("borderColor")?, "borderColor")?,
        bar_color: color(get("barColor")?, "barColor")?,
        font_color: color(get("fontColor")?, "fontColor")?,
        press_font_color: color(get("pressFontColor")?, "pressFontColor")?,
        max_fps: uint(get("maxFPS")?, "maxFPS")?,
        // Absent means "follow the Windows UI language".
        language: entries
            .get("language")
            .and_then(|value| Language::parse(value))
            .unwrap_or_else(Language::system),
        ui_scale: entries
            .get("uiScale")
            .map(|value| float(value, "uiScale"))
            .transpose()?
            .unwrap_or(1.0)
            .clamp(0.5, 4.0),
        // New in this build; absent from older config.txt files.
        transparent_background: entries.get("transparentBackground").copied() == Some("yes"),
        click_through: entries.get("clickThrough").copied() == Some("yes"),
        always_on_top: entries.get("alwaysOnTop").copied() == Some("yes"),
        extras: Vec::new(),
    };

    Ok(Config {
        extras: unmanaged(&entries, &config),
        ..config
    })
}

/// Where `config.txt` lives: next to the executable as shipped, or - for
/// `cargo run`, where the executable sits in `target/` - in the working
/// directory.
fn resolve(dir: &Path, file_name: &str) -> PathBuf {
    let next_to_executable = dir.join(file_name);
    if next_to_executable.is_file() {
        return next_to_executable;
    }
    let in_working_directory = PathBuf::from(file_name);
    if in_working_directory.is_file() {
        return in_working_directory;
    }
    next_to_executable
}

/// Writes the configuration back, replacing the file `load` read.
pub fn save(dir: &Path, file_name: &str, config: &Config) -> Result<(), String> {
    let path = resolve(dir, file_name);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, merge(&existing, config))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}

/// Rewrites the managed values in place, so comments, ordering and settings this
/// build does not know about survive a save from the settings window. Missing
/// keys are appended; a managed key that appears twice keeps only the first.
fn merge(existing: &str, config: &Config) -> String {
    let managed = config.entries();
    let mut written = vec![false; managed.len()];
    let mut text = String::with_capacity(existing.len() + 256);

    for line in existing.lines() {
        let trimmed = line.trim_start();
        let is_comment = trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';');
        if !is_comment
            && let Some(name) = line.split('=').next()
            && let Some(index) = managed.iter().position(|(key, _)| key == name)
        {
            if !written[index] {
                push_line(&mut text, &managed[index].0, &managed[index].1);
                written[index] = true;
            }
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }

    for (index, (name, value)) in managed.iter().enumerate() {
        if !written[index] {
            push_line(&mut text, name, value);
        }
    }
    text
}

/// Names handled by this build; everything else is kept verbatim.
fn managed_names(config: &Config) -> Vec<String> {
    let mut names = vec![
        "keyAmount".to_string(),
        "keyCounter".to_string(),
        "windowHeight".to_string(),
        "windowWidth".to_string(),
        "keySize".to_string(),
        "barSpeed".to_string(),
        "margin".to_string(),
        "outlineThickness".to_string(),
        "fading".to_string(),
        "backgroundColor".to_string(),
        "keyColor".to_string(),
        "borderColor".to_string(),
        "barColor".to_string(),
        "fontColor".to_string(),
        "pressFontColor".to_string(),
        "backgroundImage".to_string(),
        "maxFPS".to_string(),
        "language".to_string(),
        "uiScale".to_string(),
        "transparentBackground".to_string(),
        "clickThrough".to_string(),
        "alwaysOnTop".to_string(),
    ];
    for index in 1..=config.key_amount as usize {
        names.push(format!("key{index}"));
        names.push(format!("displayKey{index}"));
    }
    names
}

fn unmanaged(entries: &HashMap<&str, &str>, config: &Config) -> Vec<(String, String)> {
    let managed = managed_names(config);
    entries
        .iter()
        .filter(|entry| {
            let name: &str = entry.0;
            !managed.iter().any(|managed| managed.as_str() == name)
        })
        // `key5`/`displayKey5` left over from a larger key set would collide
        // with keys added later, so they are dropped rather than kept.
        .filter(|(name, _)| !is_indexed_key(name))
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect()
}

fn is_indexed_key(name: &str) -> bool {
    let digits = name
        .strip_prefix("displayKey")
        .or_else(|| name.strip_prefix("key"))
        .unwrap_or("");
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn yes_no(value: bool) -> String {
    if value {
        "yes".to_string()
    } else {
        "no".to_string()
    }
}

/// One `name=value` line.
fn push_line(text: &mut String, name: &str, value: &str) {
    text.push_str(name);
    text.push('=');
    text.push_str(value);
    text.push_str("\r\n");
}

fn color_text(color: Color) -> String {
    format!("{},{},{},{}", color.r, color.g, color.b, color.a)
}

/// `barSpeed` was parsed with `float.Parse`, so keep a round-trippable form.
fn format_float(value: f32) -> String {
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}

fn int(value: &str, name: &str) -> Result<i32, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("config: {name} = \"{value}\" is not an integer"))
}

fn uint(value: &str, name: &str) -> Result<u32, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("config: {name} = \"{value}\" is not a positive integer"))
}

fn float(value: &str, name: &str) -> Result<f32, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("config: {name} = \"{value}\" is not a number"))
}

/// `r,g,b,a` with 0-255 components; extra components are ignored, like the
/// original `Convert.ToByte` chain.
fn color(value: &str, name: &str) -> Result<Color, String> {
    let mut parts = value.split(',');
    let mut bytes = [0u8; 4];
    for byte in &mut bytes {
        let part = parts.next().ok_or_else(|| {
            format!("config: {name} = \"{value}\" needs 4 comma separated values")
        })?;
        *byte = part.trim().parse().map_err(|_| {
            format!("config: {name} = \"{value}\": \"{part}\" is not a value in 0-255")
        })?;
    }
    Ok(Color {
        r: bytes[0],
        g: bytes[1],
        b: bytes[2],
        a: bytes[3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        Config {
            window_width: 240,
            window_height: 700,
            key_amount: 2,
            keys: vec!["Z".into(), "X".into()],
            display_keys: vec![String::new(), "跳".into()],
            key_size: 70,
            bar_speed: 600.0,
            margin: 25,
            outline_thickness: 5,
            fading: true,
            key_counter: false,
            background_image: String::new(),
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
            extras: vec![("fromAnotherBuild".into(), "1".into())],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyoverlay-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_keeps_comments_and_unknown_keys() {
        let dir = scratch("save-comments");
        let file = "config.txt";
        std::fs::write(
            dir.join(file),
            "# how many keys\nkeyAmount=9\nkey1=Q\n\n# a newer build wrote this\nfutureOption=42\n",
        )
        .unwrap();

        save(&dir, file, &sample()).unwrap();

        let written = std::fs::read_to_string(dir.join(file)).unwrap();
        assert!(
            written.contains("# how many keys"),
            "comments must survive: {written}"
        );
        assert!(
            written.contains("futureOption=42"),
            "unknown keys must survive: {written}"
        );
        assert!(
            written.contains("keyAmount=2"),
            "managed values must be updated: {written}"
        );
        assert!(
            written.contains("displayKey2=跳"),
            "new values must be written: {written}"
        );
        // and the result still loads, which is what the app does next time
        let reloaded = load(&dir, file).unwrap();
        assert_eq!(reloaded.key_amount, 2);
        assert_eq!(reloaded.keys, vec!["Z".to_string(), "X".to_string()]);
        // the file's unknown key is kept as well as the ones this build read
        let unknown: Vec<&str> = reloaded
            .extras
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(unknown, vec!["futureOption", "fromAnotherBuild"]);
        // and no key is written twice
        assert_eq!(reloaded.extras.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_with_comments_loads() {
        let dir = scratch("load-comments");
        let file = "config.txt";
        let text: String = sample()
            .entries()
            .iter()
            .map(|(name, value)| format!("# {name}\n{name}={value}\n"))
            .collect();
        std::fs::write(dir.join(file), text).unwrap();

        let config = load(&dir, file).unwrap();
        assert_eq!(config.key_amount, 2);
        assert_eq!(config.display_keys[1], "跳");
        assert!(config.fading);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
