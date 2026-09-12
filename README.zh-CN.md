# KeyOverlay（Rust）

[![构建](https://github.com/earph0n3/KeyOverlayRust/actions/workflows/build.yml/badge.svg)](https://github.com/earph0n3/KeyOverlayRust/actions/workflows/build.yml)
[![许可证：GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)

**KeyOverlay** 是一个面向节奏类游戏（例如 osu!）的轻量级 Windows 原生按键悬浮层，
但它不绑定某一款游戏或特定使用场景。它会显示实时键盘/鼠标按键、击打光条、自定义
显示名和可选的击打次数，可用于游玩辅助、录制、桌面展示，或任何需要实时按键可视化的场景。

[English](README.md)

> 仅支持 Windows。本项目是 [Blondazz/KeyOverlay](https://github.com/Blondazz/KeyOverlay)
> 的 Rust 重写版，使用 TOML 配置文件，并提供内置设置窗口。

## 功能

- 实时显示键盘和鼠标输入。
- 击打光条、可选渐隐效果和按键击打计数。
- 带实时预览的设置窗口，修改立即作用于悬浮层。
- 全局 `Ctrl+Alt+K` 快捷键：即使其他程序处于焦点，也能打开设置。
- 真透明分层窗口、鼠标穿透和窗口置顶。
- 自定义按键显示名、RGBA 颜色、窗口尺寸、边距、动画速度和帧率。
- 中英文界面，以及适配 DPI 的界面缩放。
- 单文件可执行程序：字体、图标和首次运行配置模板都已编译进程序。
- 保存配置时保留注释，以及当前版本不认识的配置项。

## 环境要求

- Windows。
- Rust 1.88 或更新版本，以及 Cargo。

程序不需要 .NET 或 CSFML。当前版本不支持跨平台构建。

## 构建与运行

```powershell
git clone https://github.com/earph0n3/KeyOverlayRust.git
cd KeyOverlayRust
cargo build --release --locked
.\target\release\keyoverlay.exe
```

开发时可以在项目根目录直接运行调试版本：

```powershell
cargo run --locked
```

Release 可执行文件位于 `target/release/keyoverlay.exe`。如果找不到配置文件，程序会把带完整
注释的 `config.toml` 和空的 `Resources/` 目录写到可执行文件旁边，然后用其中的默认值启动。

如果要使用其他配置文件，把文件名或路径作为第一个参数传入：

```powershell
.\target\release\keyoverlay.exe .\profiles\mania.toml
```

配置查找顺序是：先检查可执行文件所在目录，再检查当前工作目录。因此相对路径既适合
打包后的程序，也适合源码目录运行。

## 设置悬浮层

按 **`Ctrl+Alt+K`** 打开设置窗口。未开启 `click_through` 时，直接点击悬浮层也可以打开。

| 操作 | 行为 |
| --- | --- |
| 绑定按键 | 点击绑定项，再按下键盘键或鼠标键。 |
| 编辑显示名 | 在绑定右侧的显示名输入框中输入；留空则使用按键原名。 |
| 应用修改 | 悬浮层和预览会立即更新。 |
| 保存 / 重新载入 | `Save` 写入 TOML；`Reload` 从磁盘重新读取。 |
| 关闭 | `Esc` 或 `Close` 隐藏设置窗口。 |
| 移动悬浮层 | 按住并拖动悬浮层；按下但不移动则打开设置。 |

开启 `click_through = true` 后，悬浮层完全收不到鼠标输入。此时请用快捷键打开设置，
并在 `config.toml` 中调整位置。

## 配置文件

`config.toml` 是带注释的完整模板，包含所有支持的配置项。常用字段如下：

```toml
keys = ["Z", "X", "mLeft"]
display_keys = ["", "", "M1"]

key_size = 70
window_width = 240
window_height = 700
bar_speed = 600.0
fading = true
key_counter = false
max_fps = 60                 # 0 = 不限帧率

background_color = "#000000FF"
key_color = "#00000000"
border_color = "#FFFFFFFF"
bar_color = "#FFFFFF64"
font_color = "#FFFFFFFF"
press_font_color = "#FFFFFFFF"
background_image = ""
background_mode = "original" # original、stretch、fill、fit、tile

transparent_background = false
click_through = false
always_on_top = false
language = ""               # "en"、"zh"，或留空跟随 Windows 界面语言
ui_scale = 1.0
```

`keys` 支持程序内置的键盘名称，包括字母、功能键、修饰键、导航键，以及 `mLeft`、
`mRight`、`mMiddle`、`mXButton1`、`mXButton2` 等鼠标名称。`display_keys` 是与 `keys`
一一对应的可选显示名列表；某项留空就使用按键原名。颜色格式为 `#RRGGBB` 或
`#RRGGBBAA`。

原 C# 版本的 `config.txt` 与这个重写版不兼容，请从仓库里的 `config.toml` 模板开始。
在设置窗口中保存时，文件里的注释和未知 TOML 配置项会保留。

`background_mode` 控制背景图的显示方式：

| 值 | 行为 |
| --- | --- |
| `original` | 从左上角按原始尺寸绘制。 |
| `stretch` | 不保持比例，拉伸到铺满整个窗口。 |
| `fill` | 保持比例并铺满窗口，超出部分裁剪。 |
| `fit` | 保持比例完整显示，空出的部分使用 `background_color`。 |
| `tile` | 按原始尺寸重复平铺。 |

## 透明背景与窗口行为

默认悬浮层是不透明窗口。如果用于录制或屏幕捕获，需要去掉背景时，可以在捕获工具中对
`background_color` 使用色键。

如果希望使用桌面合成的透明效果：

- `transparent_background = true` 开启带逐像素 alpha 的分层窗口，不需要色键。
- `click_through = true` 让鼠标点击穿过悬浮层，落到后面的窗口；它也会隐含开启分层渲染。
- `always_on_top = true` 让悬浮层保持在其他窗口之上。

Windows 会把透明悬浮层合成到桌面，所以「显示器捕获」可以包含它。「窗口捕获」是否保留
alpha 取决于捕获方式。「游戏捕获」只捕获游戏本身，不会自动合成外部悬浮层；如果需要同时
显示悬浮层，请使用桌面捕获或合适的窗口捕获模式。

## 背景图

首次运行会在可执行文件旁边创建空的 `Resources/` 目录。把 PNG 或 JPEG 放进去后，设置窗口
会扫描目录，并以列表展示可用文件；点击当前文件即可选择其他图片，也可以选择「无」来关闭
背景图。

选中的文件写入 `background_image`：

```toml
background_image = "keyboard.png"
background_mode = "fill"
```

图片会按 `background_mode` 的设置绘制。背景图缺失或无法读取时，程序会跳过它并报告问题，
不会因此阻止悬浮层启动。

## 故障排查

- 配置或启动错误会写入可执行文件旁边的 `errorMessage.txt`。
- 无效的按键名称会写入 `keyErrorMessage.txt`。
- 如果 `Ctrl+Alt+K` 没有反应，可能是其他程序已经占用了这个全局快捷键。
- 中文界面和自定义中文显示名需要 Windows 中存在 CJK 字体；程序内置的 Consolas 不包含中文
  字形。

## 开发

CI 工作流会执行与项目一致的检查：

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

## 许可证

GPL-3.0-or-later，详见 [LICENSE](LICENSE)。

本项目是 [Blondazz/KeyOverlay](https://github.com/Blondazz/KeyOverlay) 的 Rust 从零重写版。

当前实现使用了 AI 辅助开发。CI 会检查格式、Clippy、测试和 Release 构建，但在正式使用前，
仍应先实际验证运行效果。
