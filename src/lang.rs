//! UI language.
//!
//! All window strings live in one table, so adding a language means filling in
//! every field once - the compiler keeps the languages in sync.

use std::path::PathBuf;

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    Zh,
}

impl Language {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en-gb" | "english" => Some(Self::En),
            "zh" | "zh-cn" | "zh-hans" | "cn" | "chinese" => Some(Self::Zh),
            _ => None,
        }
    }

    /// Stored in `config.txt`.
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    /// Shown in the settings window.
    pub fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Zh => "中文",
        }
    }

    /// Used when `config.txt` carries no `language` key: follow Windows, so a
    /// Chinese system starts up in Chinese.
    pub fn system() -> Self {
        // `GetUserDefaultUILanguage` returns a LANGID; the low 10 bits are the
        // primary language, 0x04 = Chinese (simplified and traditional alike).
        let id = unsafe { GetUserDefaultUILanguage() };
        if id & 0x03ff == 0x0004 {
            Self::Zh
        } else {
            Self::En
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::En => Self::Zh,
            Self::Zh => Self::En,
        }
    }
}

pub struct Text {
    pub title: &'static str,
    /// Two `{}` placeholders: the hotkey and the config file name.
    pub note: &'static str,
    pub close: &'static str,
    pub live_preview: &'static str,
    pub save: &'static str,
    pub reload: &'static str,
    pub status_applied: &'static str,
    /// One `{}`: the config file name.
    pub status_saved: &'static str,
    pub status_reloaded: &'static str,

    pub section_interface: &'static str,
    pub ui_scale: &'static str,
    pub ui_scale_hint: &'static str,

    pub section_keys: &'static str,
    pub add_key: &'static str,
    pub binding_hint: &'static str,
    pub key_name: &'static str,
    pub press_key: &'static str,
    pub release_key: &'static str,

    pub section_layout: &'static str,
    pub key_size: &'static str,
    pub margin: &'static str,
    pub outline: &'static str,
    pub bar_speed: &'static str,
    pub window_width: &'static str,
    pub window_height: &'static str,
    pub max_fps: &'static str,
    pub key_counter: &'static str,

    pub section_appearance: &'static str,
    pub fading: &'static str,
    pub color_names: [&'static str; 6],
    pub channels: [&'static str; 4],

    pub section_window: &'static str,
    pub transparent: &'static str,
    pub click_through: &'static str,
    pub always_on_top: &'static str,
    pub background_image: &'static str,
    pub none: &'static str,
    pub missing_suffix: &'static str,
}

pub static EN: Text = Text {
    title: "KeyOverlay settings",
    note: "{} toggles this window  |  Edits apply live, Save writes {}",
    close: "Close",
    live_preview: "Live preview",
    save: "Save to config.txt",
    reload: "Reload file",
    status_applied: "Applied to the overlay",
    status_saved: "Saved to {}",
    status_reloaded: "Reloaded from disk",
    section_interface: "Interface",
    ui_scale: "UI scale",
    ui_scale_hint: "Text size (1.00 = auto)",
    section_keys: "Keys",
    add_key: "+ Add key",
    binding_hint: "Click a binding, then press the key you want to use",
    key_name: "(key name)",
    press_key: "press a key...",
    release_key: "release...",
    section_layout: "Layout and animation",
    key_size: "Key size",
    margin: "Margin",
    outline: "Outline",
    bar_speed: "Bar speed",
    window_width: "Window width",
    window_height: "Window height",
    max_fps: "Max FPS",
    key_counter: "Key counter under each key",
    section_appearance: "Appearance",
    fading: "Fading overlay at the top",
    color_names: ["Background", "Key", "Border", "Bar", "Font", "Pressed font"],
    channels: ["Red", "Green", "Blue", "Alpha"],
    section_window: "Window and streaming",
    transparent: "Transparent background (layered window, no chroma key needed)",
    click_through: "Click-through (mouse ignores the overlay)",
    always_on_top: "Always on top",
    background_image: "Background image",
    none: "(none)",
    missing_suffix: " (missing)",
};

pub static ZH: Text = Text {
    title: "KeyOverlay 设置",
    note: "{} 开关本窗口  |  修改立即生效，保存写入 {}",
    close: "关闭",
    live_preview: "实时预览",
    save: "保存到 config.txt",
    reload: "重新载入",
    status_applied: "已应用到悬浮层",
    status_saved: "已保存到 {}",
    status_reloaded: "已从磁盘重新载入",
    section_interface: "界面",
    ui_scale: "界面缩放",
    ui_scale_hint: "文字与控件大小（1.00=自动）",
    section_keys: "按键",
    add_key: "+ 添加按键",
    binding_hint: "点击绑定按钮，然后按下你要用的键",
    key_name: "(显示键名)",
    press_key: "请按键...",
    release_key: "请松开...",
    section_layout: "布局与动画",
    key_size: "键大小",
    margin: "边距",
    outline: "描边",
    bar_speed: "条速",
    window_width: "窗口宽",
    window_height: "窗口高",
    max_fps: "最大帧率",
    key_counter: "每个键下方显示计数",
    section_appearance: "外观",
    fading: "顶部渐隐效果",
    color_names: ["背景", "键面", "边框", "进度条", "文字", "按下文字"],
    channels: ["红", "绿", "蓝", "不透明度"],
    section_window: "窗口与直播",
    transparent: "透明背景（分层窗口，无需色键）",
    click_through: "鼠标穿透（点击不挡住游戏）",
    always_on_top: "窗口置顶",
    background_image: "背景图",
    none: "(无)",
    missing_suffix: "（缺失）",
};

pub fn text(language: Language) -> &'static Text {
    match language {
        Language::En => &EN,
        Language::Zh => &ZH,
    }
}

/// Fills the `{}` placeholders of a template.
pub fn fill(template: &str, first: &str, second: &str) -> String {
    let mut out = template.replacen("{}", first, 1);
    out = out.replacen("{}", second, 1);
    out
}

/// A CJK-capable face from the system font directory. The embedded Consolas has
/// no CJK glyphs, so Chinese labels and Chinese `displayKey` text come from here.
///
/// Returns the font file together with the collection index to use.
pub fn load_cjk_font() -> Option<Vec<u8>> {
    let dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/Windows"))
        .join("Fonts");
    // Ordered by how good they look as UI text; every Windows install with
    // Chinese support has at least one of these.
    for name in [
        "msyh.ttc",
        "Deng.ttf",
        "simhei.ttf",
        "msyh.ttf",
        "simsun.ttc",
        "msjh.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            return Some(bytes);
        }
    }
    None
}
