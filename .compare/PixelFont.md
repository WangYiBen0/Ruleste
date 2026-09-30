# PixelFont / PixelFontSize 代码对比分析

> **C# 原版**：`references/source/Celeste/Monocle/PixelFont.cs`（162 行）＋ `PixelFontSize.cs`（约 300 行）＋ `PixelFontCharacter.cs`
> **Rust 实现**：`crates/ruleste-core/src/data/font.rs`（734 行）＋ `src/interface/renderer.rs::draw_pixel_texts`（BMFont 渲染）

## 1. 数据结构对比

| C# | Rust | 状态 |
|----|------|------|
| `PixelFont.Face` | `PixelFont::face` | ✅ |
| `PixelFont.Sizes` | `PixelFont::sizes: Vec<PixelFontSize>` | ✅ |
| `PixelFontSize.Textures`（MTexture 多页） | `PixelFontSize.page_textures: Vec<String>`（页基名，由渲染层上传 PNG） | 🟠 |
| `PixelFontSize.Characters` | `characters: HashMap<char, PixelFontCharacter>` | ✅ |
| `PixelFontSize.LineHeight` | `line_height: i32` | ✅ |
| `PixelFontSize.Size` | `size: f32` | ✅ |
| `PixelFontSize.Outline` | `outline: bool` | ✅ |
| `PixelFontCharacter.XOffset/YOffset/XAdvance` | `x_offset/y_offset/x_advance` | ✅ |
| `PixelFontCharacter.Kerning` | `kerning: HashMap<char, i32>` | ✅ |
| `PixelFontSize.managedTextures`（Dispose） | Rust 自动 Drop | ✅ |

## 2. 方法对比

| C# | Rust | 状态 |
|----|------|------|
| `AddFontSize(path, atlas?, outline?)` | `PixelFont::load`（解析 `.fnt`） | 🟠 单文件一次加载全部 size；无追加 |
| `AddFontSize(path, data, atlas?, outline?)` | 内部 `parse_font` | 🟠 |
| `Get(float size)` | `PixelFont::get`（最小 ≥ size；否则最大） | ✅ |
| `Has(float size)` | 缺失（`get` 后比较可替代） | 🟡 |
| `Draw(float baseSize, string, position, justify, scale, color[, edgeDepth, edgeColor, stroke, strokeColor])` | `renderer::draw_pixel_texts` | 🟠 见下 |
| `PixelFontSize.AutoNewline(text, width)` | `PixelFontSize::auto_newline` | ✅ 本轮已实现 |
| `PixelFontSize.Measure(char/string)` | `measure(&str) -> (i32,i32)` | ✅ |
| `PixelFontSize.WidthToNextLine(text, start)` | `width_to_next_line` | ✅ |
| `PixelFontSize.HeightOf(text)` | 缺失（`measure` 返回高度已含换行） | 🟡 |
| `PixelFontSize.Get(int id)` | `characters.get(&char)` | 🟠 id→char |
| `PixelFontSize.Draw(...)`（字形 loop） | `draw_pixel_line` | 🟠 |

## 3. 渲染能力差异（重点）

C# `PixelFontSize.Draw` 支持：`justify`（0..1 的 X/Y 比例）、`scale`、**edgeDepth/edgeColor**（低缘投影）、**stroke/strokeColor**（描边）、`Outline` flag（用 outline 字形页）。
Rust `renderer::draw_pixel_texts` 目前支持：多行、`Justify`（Left/Center/Right）、`outline: Option<Color>`（1px 描边）、按 `size` 选取字号再缩放。

| 能力 | C# | Rust | 状态 |
|------|-----|------|------|
| 多行 + kerning | ✅ | ✅（`measure`/绘制循环带 kerning） | ✅ |
| justify（9 点） | Vector2 任意比例 | Justify 枚举 3 种水平 | 🟠 |
| scale（xy） | ✅ 可非等比 | 无（仅字号统一缩放） | 🔴 |
| edgeDepth/edgeColor（低缘阴影） | ✅ | 缺失 | 🔴 |
| stroke 任意厚度 | ✅ | 仅 1px | 🔴 |
| Outline 页（Everest） | ✅ | `outline: bool` 解析但渲染只用第一页 | 🔴 |
| 自动换行 AutoNewline | ✅ | `auto_newline` + 渲染层按 320px 视口宽调用 | ✅ 本轮已实现 |
| 多字号同文件追加 | ✅ | 单次 load | 🟠 |

## 4. 现有实现细节

`font.rs` 已完整解析 `.fnt` 的 `<info>/<common>/<pages>/<chars>/<kernings>`，并支持 `PixelFont::get` 选字号 + 渲染层 `baseSize/that_size` 缩放（对齐 C# `scale *= baseSize / pixelFontSize.Size`）。`draw_pixel_line` 每条纹理会按 `region` 绘制并累计 `x_advance + kerning`。Text 命令（`draw.rs::Text`）携带 `size`（默认 64 对话字号）与 `outline`，已在主循环由 `draw_texts` 消费。

## 5. 建议优先级

| 优先级 | 项目 | 原因 |
|--------|------|------|
| ~~P0~~ | ~~`AutoNewline`~~ | ✅ 本轮已实现（`PixelFontSize::auto_newline`，含单测 `auto_newline_wraps_words_and_splits_long_words` / `auto_newline_preserves_whitespace_tokens`），渲染层 `draw_pixel_texts` 按 320px 逻辑视口宽换行 |
| P1 | edgeDepth/edgeColor + 任意 stroke | 菜单/过场文本视觉 |
| P1 | justify 垂直与 scale | `TextCentered`/`OutlineTextCentered` |
| P2 | 多字号加载合并 | 字号追加场景少 |
| P2 | Outline 页面 | Everest/异变向 |
