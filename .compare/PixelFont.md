# PixelFont / PixelFontSize 代码对比分析

> **C# 原版**：`references/source/Celeste/Monocle/PixelFont.cs` + `PixelFontSize.cs` + `PixelFontCharacter.cs`
> **Rust 实现**：`src/data/font.rs`

---

## 1. 数据结构对比

### 1.1 `PixelFontCharacter`

| 字段 | C# | Rust | 状态 |
|---|---|---|---|
| 字符标识 | `int Character` | `char character` | 🟠 C# 用 `int`（Unicode 码点），Rust 直接用 `char` |
| 纹理 | `MTexture Texture`（引用 atlas 子纹理） | `GlyphRegion region`（仅存区域坐标） | 🟡 Rust 存区域而非纹理引用，渲染时需额外绑定 |
| X 偏移 | `int XOffset` | `i32 x_offset` | ✅ |
| Y 偏移 | `int YOffset` | `i32 y_offset` | ✅ |
| X 步进 | `int XAdvance` | `i32 x_advance` | ✅ |
| 字距调整 | `Dictionary<int, int> Kerning` | `HashMap<char, i32> kerning` | 🟠 C# 用 `int` 键（码点），Rust 用 `char` 键 |

### 1.2 `PixelFontSize`

| 字段 | C# | Rust | 状态 |
|---|---|---|---|
| 纹理列表 | `List<MTexture> Textures` | `Vec<String> page_textures`（存文件名） | 🟡 Rust 存名称而非纹理对象，需外部加载绑定 |
| 字符表 | `Dictionary<int, PixelFontCharacter> Characters` | `HashMap<char, PixelFontCharacter> characters` | 🟠 键类型不同 |
| 行高 | `int LineHeight` | `i32 line_height` | ✅ |
| 字号 | `float Size` | `f32 size` | ✅ |
| 是否描边 | `bool Outline` | `bool outline` | ✅ |
| 临时 StringBuilder | `StringBuilder temp`（用于 `AutoNewline`） | 无 | 🟡 Rust 无需手动管理，但缺少 `AutoNewline` |

### 1.3 `PixelFont`

| 字段 | C# | Rust | 状态 |
|---|---|---|---|
| 字体名 | `string Face` | `String face` | ✅ |
| 尺寸列表 | `List<PixelFontSize> Sizes` | `Vec<PixelFontSize> sizes` | ✅ |
| 管理纹理 | `List<VirtualTexture> managedTextures` | 无 | 🟡 Rust 无需显式释放，所有权系统处理 |

---

## 2. PixelFont 方法对比

### 2.1 构造 / 新建

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| 构造函数 | `PixelFont(string face)` | `PixelFont::new(face: impl Into<String>) -> Self` | ✅ 功能对齐 |

### 2.2 加载字号

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `AddFontSize(path, atlas, outline)` | 从 XML 文件加载，支持 atlas 查找和 VirtualTexture 降级；检查重复 size 后返回已有 | `PixelFont::load(path)` → `parse(xml_str)`：从文件读取并解析 | 🟠 **部分实现** |
| `AddFontSize(path, data, atlas, outline)` | 接受 `XmlElement data`，允许从已解析 XML 加载 | `PixelFont::parse(xml_str)`：接受字符串解析 | 🟠 **部分实现** |

**差异详情**：

- **多尺寸支持**：C# 版本支持同一个 `PixelFont` 中加载**多个不同 size**（通过 `AddFontSize` 多次调用），会按 `size` 排序并去重。Rust 版本 `parse()` 一次只创建**一个** `PixelFontSize`（只能从单个 XML 解析出单个字号），不支持多字号合并。
- **Atlas 集成**：C# 版本接受 `Atlas atlas` 参数，优先从 atlas 查找纹理；找不到时创建 `VirtualTexture`。Rust 版本不涉及 atlas，仅存 `page_textures` 文件名。
- **Outline 参数**：C# `AddFontSize` 接受 `outline` 标志位。Rust `parse()` 硬编码 `outline: false`。
- **重复检测**：C# 检查 `Sizes` 中是否已存在相同 `size`，存在则直接返回。Rust 无此检查。
- **排序**：C# 加载后按 `size` 升序排序。Rust 未排序（但因为每次只添加一个，也无意义）。
- **VirtualTexture 管理**：C# 维护 `managedTextures` 列表供 `Dispose` 释放。Rust 无需此机制。

### 2.3 获取字号

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Get(float size)` | 返回最小的 `size >= requested`，若全部小于则返回最大 | `PixelFont::get(base_size: f32) -> Option<&PixelFontSize>` | ✅ 逻辑对齐 |
| `Has(float size)` | 检查是否存在精确匹配的 size | 无对应方法 | 🔴 **缺失** |

**`Get` 差异**：
- C# 保证 `Sizes` 非空时至少有一个元素（无空列表保护）。
- Rust 返回 `Option`，空列表时返回 `None`，更安全。
- 逻辑完全一致。

### 2.4 Draw 方法族（PixelFont 级别）

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Draw(baseSize, char, position, justify, scale, color)` | 渲染单字符 | 无 | 🔴 **缺失** |
| `Draw(baseSize, text, position, justify, scale, color, edgeDepth, edgeColor, stroke, strokeColor)` | 渲染文本（全参数） | 无 | 🔴 **缺失** |
| `Draw(baseSize, text, position, color)` | 简化渲染 | 无 | 🔴 **缺失** |
| `Draw(baseSize, text, position, justify, scale, color)` | 带 justify/scale 渲染 | 无 | 🔴 **缺失** |
| `DrawOutline(baseSize, ...)` | 带描边渲染 | 无 | 🔴 **缺失** |
| `DrawEdgeOutline(baseSize, ...)` | 带边缘+描边渲染 | 无 | 🔴 **缺失** |

**说明**：C# 的 `PixelFont.Draw` 方法族的核心逻辑是：
1. 调用 `Get(baseSize * max(scale.X, scale.Y))` 选择字号
2. 计算 `scale *= baseSize / size` 调整缩放
3. 委托给 `PixelFontSize.Draw`

Rust 中这些方法**全部缺失**——字体解析层独立于渲染层，渲染由 `Renderer` 统一处理。这是合理的架构分离，但意味着 `PixelFont` 不具备独立渲染能力。

### 2.5 Dispose

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Dispose()` | 释放 managed textures，清空 Sizes | 无（Rust RAII） | ✅ 不需要 |

---

## 3. PixelFontSize 方法对比

### 3.1 AutoNewline

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `AutoNewline(text, width)` | 自动换行：按空格分词，超宽换行，单词超宽时逐字符断行 | 无 | 🔴 **缺失** |

**差异详情**：这是 `PixelFontSize` 中最复杂的方法。它通过正则 `(\s)` 分割文本，逐步测量单词宽度，在 `> width` 处插入换行。对于超长单词，逐字符测量并断行。Rust 版本完全没有对应实现。

### 3.2 Get (Character)

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Get(int id)` | 通过码点查找字符，未找到返回 `null` | `PixelFontSize.characters.get(&c)` 直接使用 HashMap | ✅ 语义对齐 |

### 3.3 Measure

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Measure(char text)` | 测量单字符宽度：返回 `(XAdvance, LineHeight)` | 无单独 char 版本，但 `measure(&str)` 能处理 | 🟡 近似——Rust 没有单字符特化版本 |
| `Measure(string text)` | 测量整段文本：逐字符累加宽、处理换行、应用 kerning | `PixelFontSize::measure(text: &str) -> (i32, i32)` | ✅ 逻辑对齐 |

**`Measure(string)` 差异**：
- C# 返回 `Vector2`（浮点），Rust 返回 `(i32, i32)`（整数像素）。
- C# 以 `(0, LineHeight)` 初始化（一行已有高度）。Rust 也以 `(0, line_height)` 初始化。
- 换行时，C# 累加 `LineHeight` 到 `Y`，并取当前行宽最大值。Rust 逻辑一致。
- Kerning 处理：C# 通过 `Characters.TryGetValue` + `Kerning.TryGetValue`。Rust 通过 `HashMap::get` + `peek()`。语义一致。
- **返回值类型**：`Vector2`（f32）vs `(i32, i32)`——精度差异，原版 Celeste 的字符度量是整数像素，影响不大。

### 3.4 WidthToNextLine

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `WidthToNextLine(text, start)` | 从 `start` 位置到下一个 `\n` 之间的文本宽度 | `PixelFontSize::width_to_next_line(text: &str, start: usize) -> i32` | ✅ 逻辑对齐 |

**差异**：
- C# 返回 `float`，Rust 返回 `i32`。
- C# 的 `start` 是字符索引（`string` 按 UTF-16 处理），Rust 的 `start` 是 `chars().collect()` 后的索引（按 Unicode 标量值）。对于 BMP 字符一致，对 emoji 等有潜在差异。

### 3.5 HeightOf

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `HeightOf(text)` | 计算文本行数 × lineHeight | `PixelFontSize::height_of(text: &str) -> i32` | ✅ 逻辑对齐 |

**差异**：
- C# 使用 `IndexOf('\n')` 快速跳过无换行文本。Rust 始终遍历。性能差异微小。

### 3.6 Draw 方法族（PixelFontSize 级别）

| 方法 | C# | Rust | 状态 |
|---|---|---|---|
| `Draw(char, position, justify, scale, color)` | 渲染单字符 | 无 | 🔴 **缺失** |
| `Draw(string, position, justify, scale, color, edgeDepth, edgeColor, stroke, strokeColor)` | 全参数文本渲染（含 stroke/edge） | 无 | 🔴 **缺失** |
| `Draw(string, position, color)` | 简化文本渲染 | 无 | 🔴 **缺失** |
| `Draw(string, position, justify, scale, color)` | 带 justify/scale 渲染 | 无 | 🔴 **缺失** |
| `DrawOutline(string, ...)` | 带描边渲染 | 无 | 🔴 **缺失** |
| `DrawEdgeOutline(string, ...)` | 带边缘+描边渲染 | 无 | 🔴 **缺失** |

**说明**：这些方法是字体渲染的核心。C# 实现包含：
- **Kerning 应用**：每字符后查询下一个字符的 kerning 偏移
- **Stroke 描边**：在 8 个方向（无 edge 时）或网格方向（有 edge 时）重复绘制
- **Edge 深度**：将字符向下偏移 `edgeDepth` 像素再绘制边缘色
- **Justify 对齐**：基于每行宽度计算偏移

Rust 中这些渲染方法全部缺失。渲染逻辑位于 `src/interface/renderer` 中，通过 `FontPage` + `PixelFontCharacter.region` 进行绘制。

---

## 4. 额外差异：Rust 独有

| 功能 | Rust | C# | 说明 |
|---|---|---|---|
| `SpriteFont` 类型 | ✅ 完整实现 | 无 | XNA `.xnb` / `.spritefont` 占位实现，用于兼容 |
| `load_page_png()` | ✅ PNG 解码 | 通过 atlas/VirtualTexture | Rust 自行解码 PNG 为 FontPage |
| `FontPage` 结构 | ✅ `rgba: Vec<u8>` | 无（使用 MTexture） | 直接提供像素数据 |
| 单元测试 | ✅ 9 个测试 | 无 | Rust 有完整的解析和度量测试 |

---

## 5. 总结

### 对齐统计

| 状态 | 数量 | 详情 |
|---|---|---|
| ✅ 完全对齐 | **10** | 构造、Get、Measure(string)、WidthToNextLine、HeightOf、各结构体主要字段 |
| 🟠 部分实现 | **4** | AddFontSize（无多字号/atlas）、Character 字段类型差异、Get(int id) 封装差异 |
| 🟡 近似 | **5** | Measure(char) 无特化、page_textures 存名称非纹理、outline 硬编码 false、无 AutoNewline、int vs char 键差异 |
| 🔴 缺失 | **11** | Has()、AutoNewline()、PixelFont.Draw 族（6个）、PixelFontSize.Draw 族（6个中的 6个——实际全部缺失） |

### 核心差距

1. **渲染层缺失**：C# 版本的 `Draw` 方法族（共 ~12 个重载）全部缺失。这是最大差距。Rust 将解析和渲染分离，渲染由 `Renderer` 处理，但 `PixelFont` / `PixelFontSize` 不具备独立渲染能力。
2. **AutoNewline 缺失**：自动换行是 UI 文本显示的关键功能，未实现。
3. **Has() 缺失**：简单的存在性检查，低优先级。
4. **多字号加载**：`parse()` 只创建单个 size，而 `AddFontSize` 在原版支持多字号。需要重构 `load` 以支持追加或合并。

### 设计决策

Rust 版本采用了**解析/渲染分离**架构：
- `font.rs` 负责 BMFont XML 解析和度量计算
- `FontPage` + `GlyphRegion` 为渲染层提供数据
- 渲染由 `src/interface/renderer` 统一处理

这比 C# 的 `PixelFont.Draw` 自绘模式更清晰，但意味着需要在渲染层补充等效的 stroke/edge/kerning 逻辑。
