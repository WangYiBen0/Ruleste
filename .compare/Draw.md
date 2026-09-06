# Draw 类对比分析：C# Monocle → Rust Ruleste

> **源文件**
> - C#: `references/source/Celeste/Monocle/Draw.cs` (351 行)
> - Rust: `src/engine/draw.rs` (97 行)

## 架构差异概述

| 维度 | C# (Monocle) | Rust (Ruleste) |
|------|-------------|----------------|
| **设计模式** | 静态工具类，即时模式（Immediate Mode）API | 数据结构（Command Objects），延迟提交（Deferred） |
| **渲染方式** | 直接调用 `SpriteBatch.Draw()` | 定义绘制命令结构体，由宿主收集并在 `entity_draw` 阶段统一渲染 |
| **调用者** | 任何游戏代码直接调用 `Draw.Xxx()` | Wasm 插件通过 FFI 提交结构体数据 |
| **字体支持** | `SpriteFont`（XNA 内置） | 自定义字体系统（未在本文件中） |
| **纹理** | 直接引用 `MTexture` | 通过 atlas frame id 字符串引用 |

> **核心结论**：Rust 版本并非逐方法移植 C# 的 `Draw` 类，而是采用了一种**完全不同的架构**——将绘制需求抽象为可序列化的数据结构，便于跨 Wasm 边界传递。以下逐项对比原始 API 在新架构中的对应情况。

---

## 1. 静态字段 / 属性

### `Draw.Particle` / `Draw.Pixel`

| | C# | Rust |
|--|----|------|
| **用途** | 1×1 白色像素纹理，用于所有程序化绘制 | 无对应（由渲染器内部处理） |
| **状态** | 🔴 缺失（不需要） | Rust 不暴露像素纹理；渲染器在宿主侧直接使用内部 pixel texture |

### `Draw.Renderer`

| | C# | Rust |
|--|----|------|
| **用途** | 引用当前活动 Renderer | 无对应 |
| **状态** | 🔴 缺失（不需要） | 架构不同，渲染器由宿主管理，插件不直接访问 |

### `Draw.SpriteBatch`

| | C# | Rust |
|--|----|------|
| **用途** | XNA SpriteBatch 引用 | 无对应 |
| **状态** | 🔴 缺失（不需要） | SDL3 渲染器由宿主控制，插件不持有渲染上下文 |

### `Draw.DefaultFont`

| | C# | Rust |
|--|----|------|
| **用途** | 默认 SpriteFont 引用 | 无对应 |
| **状态** | 🔴 缺失 | 字体系统在宿主侧管理，插件不直接引用字体对象 |

### `Draw.rect` (私有 Rectangle)

| | C# | Rust |
|--|----|------|
| **用途** | 内部复用的 Rectangle 字段，避免分配 | 无对应 |
| **状态** | 🔴 缺失（不需要） | Rust 版本每次创建独立结构体，无此优化需求 |

---

## 2. 初始化方法

### `Draw.Initialize(GraphicsDevice)`

| | C# | Rust |
|--|----|------|
| **签名** | `internal static void Initialize(GraphicsDevice)` | 无对应 |
| **行为** | 创建 SpriteBatch、加载默认字体、调用 `UseDebugPixelTexture()` | — |
| **状态** | 🔴 缺失（不需要） | 渲染器初始化由宿主（`src/interface/` 或 `src/engine/renderer.rs`）处理 |

### `Draw.UseDebugPixelTexture()`

| | C# | Rust |
|--|----|------|
| **签名** | `public static void UseDebugPixelTexture()` | 无对应 |
| **行为** | 创建 3×3 白色纹理，裁切为 1×1 的 `Pixel` 和 `Particle` | — |
| **状态** | 🔴 缺失（不需要） | 同上 |

---

## 3. 点绘制

### `Draw.Point(Vector2, Color)`

| | C# | Rust |
|--|----|------|
| **签名** | `public static void Point(Vector2 at, Color color)` | 无对应 |
| **行为** | 在指定位置绘制一个像素 | — |
| **状态** | 🔴 缺失 | 原版中极少使用，可通过 `Rect` 代替 |

---

## 4. 线绘制

### `Draw.Line(...)` (4 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `Line(Vector2, Vector2, Color)` | `Line` 结构体 | ✅ 完全对齐 |
| 2 | `Line(Vector2, Vector2, Color, float thickness)` | `Line` 结构体（无 thickness 字段） | 🟠 部分实现 |
| 3 | `Line(float, float, float, float, Color)` | `Line` 结构体（字段为 f32） | ✅ 完全对齐 |
| 4 | `Line(float, float, float, float, Color, float thickness)` | `Line` 结构体（无 thickness 字段） | 🟠 部分实现 |

**逐行差异分析（以 C# `Line(Vector2, Vector2, Color)` 为例）：**

```csharp
// C# - 即时绘制
public static void Line(Vector2 start, Vector2 end, Color color)
{
    LineAngle(start, Calc.Angle(start, end), Vector2.Distance(start, end), color);
}
```

```rust
// Rust - 数据结构
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub color: Color,
}
```

**差异：**
- C# 直接绘制（调用 `LineAngle` → `SpriteBatch.Draw`）
- Rust 仅定义数据结构，实际绘制由宿主渲染器完成
- Rust 使用 4 个 `f32` 字段代替 `Vector2`（无依赖）
- 缺少 `thickness` 参数：C# 的 thickness 重载通过不同 `LineAngle` 实现，Rust `Line` 结构体无此字段
- 缺少 `LineAngle` 系列方法：`LineAngle(start, angle, length, color)` 等 3 个重载在 Rust 中无对应

### `Draw.LineAngle(...)` (3 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `LineAngle(Vector2, float, float, Color)` | 无 | 🔴 缺失 |
| 2 | `LineAngle(Vector2, float, float, Color, float thickness)` | 无 | 🔴 缺失 |
| 3 | `LineAngle(float, float, float, float, Color)` | 无 | 🔴 缺失 |

> **注**：`LineAngle` 通常被 `Line` 内部使用，插件可能不需要直接调用。Rust 的 `Line` 结构体可通过宿主侧计算角度和长度来等效实现。

---

## 5. 圆绘制

### `Draw.Circle(...)` (4 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `Circle(Vector2, float, Color, int resolution)` | `Circle` 结构体 | 🟡 近似 |
| 2 | `Circle(float, float, float, Color, int resolution)` | `Circle` 结构体 | 🟡 近似 |
| 3 | `Circle(Vector2, float, Color, float thickness, int resolution)` | `Circle` 结构体（无 thickness/resolution） | 🟠 部分实现 |
| 4 | `Circle(float, float, float, Color, float thickness, int resolution)` | `Circle` 结构体（无 thickness/resolution） | 🟠 部分实现 |

**逐行差异分析（以 C# `Circle(Vector2, float, Color, int)` 为例）：**

```csharp
// C# - 即时绘制，4 象限对称线段
public static void Circle(Vector2 position, float radius, Color color, int resolution)
{
    Vector2 vector = Vector2.UnitX * radius;
    Vector2 vector2 = vector.Perpendicular();
    for (int i = 1; i <= resolution; i++)
    {
        Vector2 vector3 = Calc.AngleToVector((float)i * ((float)Math.PI / 2f) / (float)resolution, radius);
        Vector2 vector4 = vector3.Perpendicular();
        Line(position + vector, position + vector3, color);
        Line(position - vector, position - vector3, color);
        Line(position + vector2, position + vector4, color);
        Line(position - vector2, position + vector4, color);
        vector = vector3;
        vector2 = vector4;
    }
}
```

```rust
// Rust - 数据结构
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub color: Color,
}
```

**差异：**
- C# 接受 `resolution`（每象限线段数）和 `thickness` 参数
- Rust 仅有 `cx`, `cy`, `r`, `color`，无 `resolution` 和 `thickness`
- `resolution` 和 `thickness` 可能由宿主渲染器使用默认值或通过全局配置提供
- 圆的绘制算法（4 象限对称）需在宿主侧实现

---

## 6. 矩形绘制

### `Draw.Rect(...)` (4 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `Rect(float, float, float, float, Color)` | `Rect` 结构体 | ✅ 完全对齐 |
| 2 | `Rect(Vector2, float, float, Color)` | `Rect` 结构体 | ✅ 完全对齐 |
| 3 | `Rect(Rectangle, Color)` | `Rect` 结构体 | ✅ 完全对齐 |
| 4 | `Rect(Collider, Color)` | 无 | 🔴 缺失 |

**逐行差异分析（以 C# `Rect(float, float, float, float, Color)` 为例）：**

```csharp
// C# - 即时绘制
public static void Rect(float x, float y, float width, float height, Color color)
{
    rect.X = (int)x;
    rect.Y = (int)y;
    rect.Width = (int)width;
    rect.Height = (int)height;
    SpriteBatch.Draw(Pixel.Texture.Texture, rect, Pixel.ClipRect, color);
}
```

```rust
// Rust - 数据结构
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}
```

**差异：**
- C# 使用复用的 `rect` 字段（int 坐标），避免 GC
- Rust 每次创建独立结构体（f32 坐标），无 int 转换
- Rust 字段名为 `w`/`h`，C# 为 `Width`/`Height`
- `Rect(Collider)` 重载：Rust 无 Collider 类型引用（插件不直接访问碰撞体矩形）

---

## 7. 空心矩形绘制

### `Draw.HollowRect(...)` (4 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `HollowRect(float, float, float, float, Color)` | `HollowRect` 结构体 | ✅ 完全对齐 |
| 2 | `HollowRect(Vector2, float, float, Color)` | `HollowRect` 结构体 | ✅ 完全对齐 |
| 3 | `HollowRect(Rectangle, Color)` | `HollowRect` 结构体 | ✅ 完全对齐 |
| 4 | `HollowRect(Collider, Color)` | 无 | 🔴 缺失 |

**逐行差异分析（以 C# `HollowRect(float, float, float, float, Color)` 为例）：**

```csharp
// C# - 绘制 4 条边（top, bottom, left, right）
public static void HollowRect(float x, float y, float width, float height, Color color)
{
    // top edge
    rect.X = (int)x; rect.Y = (int)y;
    rect.Width = (int)width; rect.Height = 1;
    SpriteBatch.Draw(Pixel.Texture.Texture, rect, Pixel.ClipRect, color);
    // bottom edge
    rect.Y += (int)height - 1;
    SpriteBatch.Draw(Pixel.Texture.Texture, rect, Pixel.ClipRect, color);
    // left edge
    rect.Y -= (int)height - 1;
    rect.Width = 1; rect.Height = (int)height;
    SpriteBatch.Draw(Pixel.Texture.Texture, rect, Pixel.ClipRect, color);
    // right edge
    rect.X += (int)width - 1;
    SpriteBatch.Draw(Pixel.Texture.Texture, rect, Pixel.ClipRect, color);
}
```

```rust
// Rust - 数据结构
pub struct HollowRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}
```

**差异：**
- C# 通过 4 次 `SpriteBatch.Draw` 绘制 4 条 1px 边（top, bottom, left, right）
- Rust 定义结构体，由宿主渲染器实现相同的 4 边绘制逻辑
- 实现注释中说明 "rendered as pixel-perfect line segments"，与原版行为一致

---

## 8. 文本绘制

### `Draw.Text(...)` (2 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `Text(SpriteFont, string, Vector2, Color)` | `Text` 结构体 | 🟡 近似 |
| 2 | `Text(SpriteFont, string, Vector2, Color, Vector2 origin, Vector2 scale, float rotation)` | `Text` 结构体（无 origin/scale/rotation） | 🟠 部分实现 |

**逐行差异分析（以 C# `Text(SpriteFont, string, Vector2, Color)` 为例）：**

```csharp
// C# - 即时绘制
public static void Text(SpriteFont font, string text, Vector2 position, Color color)
{
    SpriteBatch.DrawString(font, text, position.Floor(), color);
}
```

```rust
// Rust - 数据结构
pub struct Text {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: Color,
    pub justify: Justify,
    pub outline: Option<Color>,
}
```

**差异：**
- C# 接受 `SpriteFont` 对象，Rust 由宿主管理字体（插件不指定字体）
- C# 第二个重载支持 `origin`, `scale`, `rotation`，Rust `Text` 结构体无这些字段
- Rust 增加了 `justify: Justify` 字段（对齐方式）和 `outline: Option<Color>`（描边颜色），这些在 C# 中是独立方法
- Rust 的 `Text` 结构体融合了 C# 的 `Text`, `TextJustified`, `TextCentered` 的功能（通过 `justify` 字段）

### `Draw.TextJustified(...)` (2 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `TextJustified(SpriteFont, string, Vector2, Color, Vector2 justify)` | `Text` 结构体 (`justify` 字段) | ✅ 完全对齐 |
| 2 | `TextJustified(SpriteFont, string, Vector2, Color, float scale, Vector2 justify)` | `Text` 结构体（无 scale） | 🟠 部分实现 |

**差异：**
- C# 通过 `font.MeasureString(text) * justify` 计算 origin
- Rust 将 `justify` 作为字段存储，由宿主渲染器执行相同的 origin 计算
- 缺少 `scale` 参数

### `Draw.TextCentered(...)` (4 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `TextCentered(SpriteFont, string, Vector2)` | `Text` 结构体 (`justify: Justify { x: 0.5, y: 0.5 }`) | ✅ 完全对齐 |
| 2 | `TextCentered(SpriteFont, string, Vector2, Color)` | `Text` 结构体 | ✅ 完全对齐 |
| 3 | `TextCentered(SpriteFont, string, Vector2, Color, float scale)` | `Text` 结构体（无 scale） | 🟠 部分实现 |
| 4 | `TextCentered(SpriteFont, string, Vector2, Color, float scale, float rotation)` | `Text` 结构体（无 scale/rotation） | 🟠 部分实现 |

**差异：**
- C# 的 4 个重载从简单（无 scale）到复杂（scale + rotation）
- Rust 的 `Text` 结构体统一处理居中（通过 justify），但缺少 `scale` 和 `rotation`

### `Draw.OutlineTextCentered(...)` (3 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `OutlineTextCentered(SpriteFont, string, Vector2, Color, float scale)` | `Text` 结构体 (`outline: Some(Color::BLACK)`) | 🟡 近似 |
| 2 | `OutlineTextCentered(SpriteFont, string, Vector2, Color, Color outlineColor)` | `Text` 结构体 (`outline: Some(outline_color)`) | 🟡 近似 |
| 3 | `OutlineTextCentered(SpriteFont, string, Vector2, Color, Color outlineColor, float scale)` | `Text` 结构体（无 scale） | 🟠 部分实现 |

**差异：**
- C# 通过 8 方向偏移（±1, ±1）绘制黑色/自定义描边，然后绘制主体
- Rust 的 `Text` 结构体有 `outline: Option<Color>` 字段，宿主渲染器需实现相同的 8 方向描边算法
- 缺少 `scale` 参数

### `Draw.OutlineTextJustify(...)` (2 个重载)

| 重载 | C# 签名 | Rust 对应 | 状态 |
|------|---------|-----------|------|
| 1 | `OutlineTextJustify(SpriteFont, string, Vector2, Color, Color, Vector2 justify)` | `Text` 结构体 (`outline + justify`) | ✅ 完全对齐 |
| 2 | `OutlineTextJustify(SpriteFont, string, Vector2, Color, Color, Vector2 justify, float scale)` | `Text` 结构体（无 scale） | 🟠 部分实现 |

---

## 9. 特殊纹理绘制

### `Draw.SineTextureH(...)`

| | C# | Rust |
|--|----|------|
| **签名** | `SineTextureH(MTexture, Vector2 pos, Vector2 origin, Vector2 scale, float rotation, Color, SpriteEffects, float sineCounter, float amplitude=2f, int sliceSize=2, float sliceAdd=π/4)` | 无对应 |
| **行为** | 将纹理水平切成 slice，每片按正弦偏移 Y，实现波浪效果 |
| **状态** | 🔴 缺失 | 未在 Rust draw.rs 中定义对应结构体 |

### `Draw.SineTextureV(...)`

| | C# | Rust |
|--|----|------|
| **签名** | `SineTextureV(MTexture, Vector2 pos, Vector2 origin, Vector2 scale, float rotation, Color, SpriteEffects, float sineCounter, float amplitude=2f, int sliceSize=2, float sliceAdd=π/4)` | 无对应 |
| **行为** | 将纹理垂直切成 slice，每片按正弦偏移 X |
| **状态** | 🔴 缺失 | 同上 |

### `Draw.TextureBannerV(...)`

| | C# | Rust |
|--|----|------|
| **签名** | `TextureBannerV(MTexture, Vector2 pos, Vector2 origin, Vector2 scale, float rotation, Color, SpriteEffects, float sineCounter, float amplitude=2f, int sliceSize=2, float sliceAdd=π/4)` | 无对应 |
| **行为** | 垂直切片 + 正弦偏移 + 高度渐变，实现旗帜飘动效果 |
| **状态** | 🔴 缺失 | 同上 |

> **注**：这三个特殊纹理效果在原版 Celeste 中用于特定实体（如旗帜、布条）。Rust 版本可能通过 `Image` 结构体在宿主侧实现类似效果，或由插件自行处理。

---

## 10. Rust 独有结构体（C# 无对应）

### `Draw.Image`

| | Rust | C# 对应 |
|--|------|---------|
| **结构体** | `Image { frame_id, x, y, rotation, scale_x, scale_y, flip_x, flip_y, color }` | 无直接对应 |
| **用途** | 绘制单个 atlas 帧，支持旋转、缩放、翻转 | C# 中通过 `SpriteBatch.Draw` 直接绘制 |
| **状态** | 🟢 Rust 新增 | 原版中类似功能通过 `SpriteBatch.Draw(MTexture, ...)` 实现 |

### `Draw.TileBox`

| | Rust | C# 对应 |
|--|------|---------|
| **结构体** | `TileBox { frame_id, x, y, width, height, col, row }` | 无对应 |
| **用途** | 自动拼接的 8×8 瓦片盒（如 introCrusher） | C# 中由 `Autotiler.GenerateBox` 生成 |
| **状态** | 🟢 Rust 新增 | 原版中类似功能由 `Autotiler` 类处理 |

---

## 11. C# 中存在但 Rust 完全缺失的方法

| C# 方法 | 用途 | 缺失原因 |
|---------|------|---------|
| `Point(Vector2, Color)` | 绘制单个像素 | 极少使用，可通过 `Rect` 代替 |
| `LineAngle(Vector2, float, float, Color)` | 按角度绘制线段 | 内部工具方法，插件不需要 |
| `LineAngle(Vector2, float, float, Color, float thickness)` | 按角度绘制带粗细线段 | 同上 |
| `LineAngle(float, float, float, float, Color)` | 按角度绘制线段（坐标版本） | 同上 |
| `Line(Vector2, Vector2, Color, float thickness)` | 带粗细的线段 | `Line` 结构体无 thickness 字段 |
| `Line(float, float, float, float, Color, float thickness)` | 带粗细的线段（坐标版本） | 同上 |
| `Rect(Collider, Color)` | 绘制碰撞体矩形 | 插件不直接访问碰撞体 |
| `HollowRect(Collider, Color)` | 绘制碰撞体空心矩形 | 同上 |
| `SineTextureH(...)` | 水平正弦波纹理 | 特殊效果，可能由插件自行实现 |
| `SineTextureV(...)` | 垂直正弦波纹理 | 同上 |
| `TextureBannerV(...)` | 垂直旗帜纹理 | 同上 |

---

## 12. 总结统计

| 状态 | 数量 | 说明 |
|------|------|------|
| ✅ 完全对齐 | **8** | `Line`(2 重载), `Rect`(3 重载), `HollowRect`(2 重载), `TextJustified`(1 重载) |
| 🟠 部分实现 | **11** | 缺少 `thickness`, `scale`, `rotation` 等参数 |
| 🟡 近似 | **5** | `Circle`(2 重载), `Text`(1 重载), `TextCentered`(2 重载), `OutlineTextCentered`(2 重载), `OutlineTextJustify`(1 重载) |
| 🔴 缺失 | **15** | `Point`, `LineAngle`(3), `Rect(Collider)`, `HollowRect(Collider)`, `SineTextureH/V`, `TextureBannerV`, 初始化方法, 静态字段 |
| 🟢 Rust 新增 | **2** | `Image`, `TileBox` |

---

## 13. 架构评价

### 优势
1. **跨 Wasm 边界安全**：数据结构可安全序列化传递，无指针/引用问题
2. **职责分离清晰**：插件只声明"要画什么"，宿主决定"怎么画"
3. **扩展性好**：新增 `Image` 和 `TileBox` 结构体支持原版中需要复杂计算的场景
4. **`Text` 结构体融合**：通过 `justify` 和 `outline` 字段统一了 C# 的 `Text`, `TextJustified`, `TextCentered`, `OutlineText*` 系列

### 不足 / 待补充
1. **`Line` 缺少 thickness**：无法绘制粗线（原版 `wire` 实体可能需要）
2. **`Text` 缺少 scale/rotation**：部分 `TextCentered` 和 `OutlineTextCentered` 重载无法表达
3. **`Circle` 缺少 resolution/thickness**：控制精度和粗细的能力缺失
4. **特殊纹理效果缺失**：`SineTextureH/V` 和 `TextureBannerV` 无对应，但这些在原版中使用场景有限
5. **无 `Point` 方法**：虽然极少使用，但某些调试场景可能需要
