# MTexture (C# Monocle) vs Rust Atlas 对比分析

> 原版 C# 源码：`references/source/Celeste/Monocle/MTexture.cs`
> Rust 实现：`src/data/atlas.rs` + `src/interface/renderer.rs`

---

## 架构差异概述

C# 版将 **纹理数据 + 子纹理视图 + 绘制方法** 全部封装在一个 `MTexture` 类中。
Rust 版采用了**分离架构**：
- `atlas.rs` 负责图集解析和像素数据（对应 `VirtualTexture` + `AtlasMeta` 的角色）
- `renderer.rs` 负责纹理上传和绘制（对应 `MTexture.Draw*` 方法）
- 没有独立的 "子纹理视图" 类型，子纹理信息直接存在 `Frame.clip` + `Frame.offset` 中

---

## 逐项对比

### 1. 字段 / 属性

| C# 属性 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `AtlasPath` (string) | 🔴 缺失 | — | Rust 没有存储 atlas 路径，使用 `Frame.id`（如 `"characters/player/sprite00"`）代替 |
| `Texture` (VirtualTexture) | ✅ 对应 | `AtlasPage.rgba` (Vec\<u8\>) | C# 引用一个 VirtualTexture 对象；Rust 直接在 `AtlasPage` 中存储 RGBA8 像素数据 |
| `ClipRect` (Rectangle) | ✅ 对应 | `Frame.clip` (FrameRect) | C# 是 `Rectangle(x,y,w,h)`；Rust 是 `FrameRect { x, y, w, h }`，语义一致 |
| `DrawOffset` (Vector2) | ✅ 对应 | `Frame.offset` (FrameRect) | C# 是 `(float, float)`；Rust 是 `FrameRect { x, y, w, h }`，额外存储了 w/h（未裁剪尺寸） |
| `Width` (int) | ✅ 对应 | `Frame.clip.w` / `AtlasPage.width` | C# 存储裁剪后宽度；Rust 通过 `frame.clip.w` 获取 |
| `Height` (int) | ✅ 对应 | `Frame.clip.h` / `AtlasPage.height` | 同上 |
| `Center` (Vector2) | 🟡 近似 | 计算时使用 `frame.clip.w * 0.5` | C# 预计算 `Center = (Width, Height) * 0.5` 并缓存；Rust 在 `renderer.rs` 绘制时即时计算，未单独存储 |
| `LeftUV / RightUV / TopUV / BottomUV` (float) | 🔴 缺失 | — | C# 预计算 UV 坐标用于 SpriteBatch.Draw；Rust 使用 SDL3 的 `FRect` 作为 source rect，不需要 UV |
| `TotalPixels` (int) | 🔴 缺失 | — | C# 的 `Width * Height` 派生属性；Rust 无等价（有 `total_pixels` 但仅在 decode 中使用） |

### 2. 构造函数

| C# 构造函数 | 状态 | Rust 对应 | 差异说明 |
|------------|------|----------|---------|
| `MTexture()` (默认) | 🔴 缺失 | — | 空构造函数；Rust 的 `Atlas::default()` 只用于初始化空图集 |
| `MTexture(VirtualTexture)` | ✅ 对应 | `AtlasPage::decode()` | C# 包装完整纹理；Rust 从 `.data` 文件解码完整页面 |
| `MTexture(parent, x, y, w, h)` | 🟡 近似 | `Frame { clip, offset }` | C# 创建子纹理视图；Rust 在 `AtlasMeta::from_bytes()` 中解析 clip/offset，作为 Frame 存储在 Page 中 |
| `MTexture(parent, clipRect)` | ✅ 对应 | 同上（委托给 x,y,w,h 版本） | C# 委托调用；Rust 中 clip 直接从 meta 读取 |
| `MTexture(parent, atlasPath, clipRect, drawOffset, w, h)` | 🟠 部分 | `Frame { id, clip, offset }` | C# 保留 atlasPath；Rust 使用 Frame.id 作为标识（语义等价但无单独 AtlasPath） |
| `MTexture(parent, atlasPath, clipRect)` | 🟠 部分 | 同上 | C# 设置 AtlasPath 但使用 clip；Rust Frame.id 同时承担路径角色 |
| `MTexture(texture, drawOffset, frameW, frameH)` | 🔴 缺失 | — | C# 用于有自定义帧尺寸的纹理；Rust 无等价构造 |

### 3. 核心方法

| C# 方法 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `SetUtil()` | 🟡 近似 | 计算分散在各处 | C# 计算 Center + UV；Rust 只在需要时计算中心点，UV 由 SDL 替代 |
| `Unload()` | 🔴 缺失 | — | C# 释放 VirtualTexture；Rust 依赖所有权/RAII，无需显式 unload |
| `GetSubtexture(x, y, w, h, applyTo)` | 🟡 近似 | `frame_rgba_into()` | C# 创建或复用子纹理视图；Rust 提取裁剪后的像素数据到新 Vec |
| `GetSubtexture(Rectangle)` | 🟡 近似 | 同上 | C# 委托调用；Rust 无 Rectangle 参数版本 |
| `ToString()` | 🔴 缺失 | — | C# 返回 AtlasPath 或尺寸；Rust 无等价（Debug trait 可部分替代） |
| `GetRelativeRect(Rectangle)` | 🔴 缺失 | — | C# 将矩形变换到当前纹理的相对坐标；Rust 无等价（绘制时直接使用 frame.clip） |
| `GetRelativeRect(x, y, w, h)` | 🔴 缺失 | — | 同上（带 clamp 逻辑） |

### 4. 绘制方法 — Draw (基础绘制)

| C# 方法 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `Draw(position)` | ✅ 对应 | `draw_entities()` + `draw_images()` | C# 直接调用 SpriteBatch.Draw；Rust 通过 SDL3 copy_ex |
| `Draw(position, origin)` | 🟡 近似 | `draw_images()` (Image struct) | C# 重载支持任意 origin；Rust 通过 Image.x/y 传入中心位置 |
| `Draw(position, origin, color)` | 🟡 近似 | `draw_images()` + color mod | C# 支持颜色参数；Rust 通过 SDL color mod |
| `Draw(position, origin, color, scale)` | 🟡 近似 | `draw_images(scale_x, scale_y)` | C# float scale 统一缩放；Rust 支持独立 scale_x/scale_y |
| `Draw(position, origin, color, scale, rotation)` | 🟡 近似 | `draw_images(rotation)` | C# rotation 弧度；Rust 旋转参数为度数（convert 在 draw_images 中） |
| `Draw(position, origin, color, scale, rotation, flip)` | 🟡 近似 | `draw_images(flip_x, flip_y)` | C# 使用 SpriteEffects 枚举；Rust 使用 bool flip_x/flip_y |
| `Draw(position, origin, color, Vector2 scale)` | 🟡 近似 | `draw_images(scale_x, scale_y)` | C# Vector2 scale；Rust 独立 scale_x/scale_y 参数 |
| `Draw(position, origin, color, Vector2 scale, rotation)` | 🟡 近似 | 同上 | — |
| `Draw(position, origin, color, Vector2 scale, rotation, flip)` | 🟡 近似 | 同上 | — |
| `Draw(position, origin, color, Vector2 scale, rotation, clip)` | 🔴 缺失 | — | C# 支持额外 clip rect 裁剪；Rust 无等价 |

**绘制差异详解**：
- C# 使用 `origin - DrawOffset` 作为 SpriteBatch 的原点偏移
- Rust 在 `draw_entities()` 中计算 `ox = frame.offset.x + (justify/center offset)`，然后 `dst_x = anchor_x - ox - camera.x`
- C# DrawOffset 的含义：子纹理相对于父纹理的裁剪偏移（负值方向）
- Rust Frame.offset 的含义：未裁剪框相对于裁剪框的偏移（正值方向）
- **两者方向相反但语义等价**：C# 从 position 减去 offset 来对齐裁剪区域；Rust 从 position 减去 offset 来定位未裁剪框的起点

### 5. 绘制方法 — DrawCentered

| C# 方法 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `DrawCentered(position)` | ✅ 对应 | `draw_entities()` (center 模式) | C# 使用 `Center - DrawOffset`；Rust 在 Sprite.center=true 时计算 `(clip.w*0.5, clip.h*0.5)` |
| `DrawCentered(position, color)` | 🟡 近似 | 同上 + color mod | — |
| `DrawCentered(position, color, scale)` | 🟡 近似 | 同上 + scale 参数 | — |
| `DrawCentered(position, color, scale, rotation)` | 🟡 近似 | 同上 + rotation | — |
| `DrawCentered(position, color, scale, rotation, flip)` | 🟡 近似 | 同上 + flip | — |
| `DrawCentered(position, color, Vector2 scale)` | 🟡 近似 | 同上 | — |
| `DrawCentered(position, color, Vector2 scale, rotation)` | 🟡 近似 | 同上 | — |
| `DrawCentered(position, color, Vector2 scale, rotation, flip)` | 🟡 近似 | 同上 | — |

### 6. 绘制方法 — DrawJustified

| C# 方法 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `DrawJustified(position, justify)` | ✅ 对应 | `draw_entities()` (justify 模式) | C# 计算 `new Vector2(Width*justify.X, Height*justify.Y) - DrawOffset`；Rust 在 Sprite.justify 中 `(jx * clip.w, jy * clip.h)` |
| `DrawJustified(position, justify, color)` | 🟡 近似 | 同上 + color | — |
| `DrawJustified(position, justify, color, scale)` | 🟡 近似 | 同上 + scale | — |
| `DrawJustified(position, justify, color, scale, rotation)` | 🟡 近似 | 同上 + rotation | — |
| `DrawJustified(position, justify, color, scale, rotation, flip)` | 🟡 近似 | 同上 + flip | — |
| `DrawJustified(position, justify, color, Vector2 scale)` | 🟡 近似 | 同上 | — |
| `DrawJustified(position, justify, color, Vector2 scale, rotation)` | 🟡 近似 | 同上 | — |
| `DrawJustified(position, justify, color, Vector2 scale, rotation, flip)` | 🟡 近似 | 同上 | — |

### 7. 绘制方法 — DrawOutline

| C# 方法 | 状态 | Rust 对应 | 差异说明 |
|---------|------|----------|---------|
| `DrawOutline(position)` (8 个重载) | 🔴 缺失 | — | C# 在 8 个方向绘制黑色偏移副本实现描边；Rust 无等价绘制方法 |
| `DrawOutlineCentered(position)` (8 个重载) | 🔴 缺失 | — | 同上，中心对齐版本 |
| `DrawOutlineJustified(position, justify)` (8 个重载) | 🔴 缺失 | — | 同上，Justify 对齐版本 |

> **注意**：Rust 的 Text 绘制支持 outline（`Text.outline` 字段），但纹理精灵的描边功能完全缺失。

### 8. Atlas 加载方法（Rust 独有）

| Rust 方法 | 状态 | C# 对应 | 差异说明 |
|----------|------|--------|---------|
| `AtlasMeta::from_bytes()` | 🟠 部分 | `Atlas.ReadAtlas()` | C# 在 `Atlas` 类中解析 meta；Rust 分离到 `AtlasMeta` |
| `AtlasMeta::from_file()` | 🟠 部分 | `Atlas.ReadAtlas(path)` | 同上 |
| `AtlasMeta::frame_ids()` | 🔴 缺失 | `Atlas.GetAtlasSubtextures()` | C# 有获取所有子纹理 ID 的方法；Rust 只有 `frame_ids()` 迭代器 |
| `AtlasPage::decode()` | ✅ 对应 | `VirtualTexture.LoadData()` | C# 在 VirtualTexture 中解码 RLE；Rust 在 AtlasPage 中完成 |
| `Atlas::load()` | ✅ 对应 | `Atlas.FromAtlas(path)` | C# 加载打包图集；Rust 等价 |
| `Atlas::load_no_pack()` | ✅ 对应 | `Atlas.FromAtlasNoPacker(path)` | C# 加载非打包图集；Rust 等价 |
| `Atlas::merge()` | 🔴 缺失 | `Atlas.Merge()` | C# 在 Atlas 中有 Merge 方法；Rust 同样有 |
| `Atlas::frame_rgba_into()` | 🔴 缺失 | — | Rust 独有：提取单帧像素数据（C# 不需要，因为直接从 GPU 纹理裁剪） |
| `Atlas::frame_clip()` | 🔴 缺失 | — | Rust 独有：查询帧的裁剪矩形 |
| `load_atlas_dir()` | 🔴 缺失 | — | Rust 独有：加载整个目录的所有图集并合并 |

---

## 总结统计

| 状态 | 数量 | 说明 |
|------|------|------|
| ✅ 完全对齐 | 8 | 核心数据结构和基本绘制概念一致 |
| 🟡 近似 | 38 | 绘制重载（功能等价但 API 签名不同）、子纹理视图 |
| 🟠 部分实现 | 4 | 构造函数和加载方法（功能覆盖但结构不同） |
| 🔴 缺失 | 14 | AtlasPath、UV 坐标、DrawOutline 系列、Unload、GetRelativeRect |

---

## 关键差异分析

### 1. 设计哲学差异
- **C#**：MTexture 是一个"智能纹理引用"，封装了裁剪、偏移、UV、绘制等全部功能
- **Rust**：采用数据驱动的分离架构，atlas.rs 管数据，renderer.rs 管绘制，通过 Frame 结构体关联

### 2. 绘制 API 差异
- **C#**：10+8+8+8+8 = **42 个 Draw 重载方法**（通过参数默认值和重载实现灵活性）
- **Rust**：统一的 `draw_entities()` + `draw_images()` 方法，通过 Entity/Image 结构体字段控制行为
- **实际影响**：Rust 的 API 更简洁但灵活性受限；插件通过 `host::draw_image()` 调用，参数较少

### 3. 内存管理差异
- **C#**：MTexture.Unload() 显式释放；VirtualTexture 有引用计数
- **Rust**：所有权系统自动管理；Atlas/Page/Frame 都是 owned 类型，Drop 自动清理

### 4. 坐标系差异
- **C# DrawOffset**：负方向偏移（子纹理左上角相对于父纹理的偏移，取负后用于绘制原点）
- **Rust Frame.offset**：正方向偏移（未裁剪框相对于裁剪框的偏移）
- **语义等价但方向相反**，绘制时需注意符号转换

---

## 建议：缺失功能优先级

### 高优先级（影响游戏功能）
1. **DrawOutline 系列**：原版游戏中文字和精灵经常使用描边效果，缺失会影响视觉保真度
2. **GetRelativeRect**：某些实体可能依赖子纹理的相对矩形计算

### 中优先级（API 完整性）
3. **AtlasPath 存储**：调试和序列化时有用
4. **Unload 方法**：热重载场景下可能需要手动释放纹理

### 低优先级（可选）
5. **UV 坐标预计算**：SDL3 使用 FRect 替代，不需要
6. **TotalPixels 属性**：仅在 decode 内部使用
7. **ToString 覆盖**：Debug trait 已覆盖部分需求
