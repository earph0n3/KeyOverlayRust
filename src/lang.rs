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

    /// Stored in the selected preset TOML file.
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

    /// Used when the selected preset carries no `language` key: follow Windows,
    /// so a Chinese system starts up in Chinese.
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
    /// One `{}` placeholder: the hotkey that opens the window.
    pub note: &'static str,
    pub close: &'static str,
    pub live_preview: &'static str,
    pub preset: &'static str,
    pub save_preset: &'static str,
    pub new_preset: &'static str,
    pub delete_preset: &'static str,
    pub create_preset: &'static str,
    pub cancel: &'static str,
    pub preset_name_hint: &'static str,
    pub unsaved_title: &'static str,
    pub unsaved_message: &'static str,
    pub save_and_switch: &'static str,
    pub discard_and_switch: &'static str,
    pub cancel_switch: &'static str,
    pub status_created: &'static str,
    pub status_deleted: &'static str,
    pub status_unsaved_delete: &'static str,
    pub reload: &'static str,
    pub reset: &'static str,
    pub status_applied: &'static str,
    pub status_saved: &'static str,
    pub status_loaded: &'static str,
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
    pub background_mode: &'static str,
    pub background_modes: [&'static str; 5],
    pub none: &'static str,
    pub missing_suffix: &'static str,
}

pub static EN: Text = Text {
    title: "KeyOverlay presets",
    note: "{} opens this window  |  Edits apply live, Save preset writes the selected TOML",
    close: "Close",
    live_preview: "Live preview",
    preset: "Preset",
    save_preset: "Save preset",
    new_preset: "New",
    delete_preset: "Delete",
    create_preset: "Create",
    cancel: "Cancel",
    preset_name_hint: "new preset name",
    unsaved_title: "Unsaved changes",
    unsaved_message: "Save changes before switching to {}?",
    save_and_switch: "Save and switch",
    discard_and_switch: "Discard and switch",
    cancel_switch: "Cancel",
    status_created: "Preset created: {}",
    status_deleted: "Preset deleted: {}",
    status_unsaved_delete: "Save the preset before deleting it",
    reload: "Reload",
    reset: "Reset",
    status_applied: "Applied to the overlay",
    status_saved: "Preset saved",
    status_loaded: "Preset loaded",
    status_reloaded: "Preset reloaded from disk",
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
    section_window: "Window and resources",
    transparent: "Transparent background (layered window, no chroma key needed)",
    click_through: "Click-through (mouse ignores the overlay)",
    always_on_top: "Always on top",
    background_image: "Background image",
    background_mode: "Background mode",
    background_modes: [
        "Original size",
        "Stretch",
        "Fill (crop)",
        "Fit (letterbox)",
        "Tile",
    ],
    none: "(none)",
    missing_suffix: " (missing)",
};

pub static ZH: Text = Text {
    title: "KeyOverlay 预设",
    note: "{} 开关本窗口  |  修改立即生效，保存预设后写入 TOML",
    close: "关闭",
    live_preview: "实时预览",
    preset: "预设",
    save_preset: "保存预设",
    new_preset: "新建",
    delete_preset: "删除",
    create_preset: "创建",
    cancel: "取消",
    preset_name_hint: "输入预设名称",
    unsaved_title: "有未保存的修改",
    unsaved_message: "切换到 {} 前是否保存当前修改？",
    save_and_switch: "保存并切换",
    discard_and_switch: "放弃并切换",
    cancel_switch: "取消",
    status_created: "预设已创建：{}",
    status_deleted: "预设已删除：{}",
    status_unsaved_delete: "请先保存预设再删除",
    reload: "重新载入",
    reset: "重置",
    status_applied: "已应用到悬浮层",
    status_saved: "预设已保存",
    status_loaded: "预设已加载",
    status_reloaded: "已从磁盘重新载入预设",
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
    section_window: "窗口与资源",
    transparent: "透明背景（分层窗口，无需色键）",
    click_through: "鼠标穿透（点击不挡住游戏）",
    always_on_top: "窗口置顶",
    background_image: "背景图",
    background_mode: "背景图显示方式",
    background_modes: ["原始尺寸", "拉伸", "填充（裁剪）", "适应（留边）", "平铺"],
    none: "(无)",
    missing_suffix: "（缺失）",
};

pub fn text(language: Language) -> &'static Text {
    match language {
        Language::En => &EN,
        Language::Zh => &ZH,
    }
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
