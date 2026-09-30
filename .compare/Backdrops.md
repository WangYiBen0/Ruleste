# Backdrops & Parallax 对比分析：Celeste (C#) vs Ruleste (Rust)

> 源码对照
> - C# 原版：`references/source/Celeste/Celeste/MapData.cs`（`CreateBackdrops`/`ParseBackdrop`，761 行）＋ `Parallax.cs`（115 行）＋ `Backdrop.cs`（168 行）
> - Rust 实现：`crates/ruleste-core/src/engine/backdrops.rs`（158 行）＋ `src/interface/renderer.rs::draw_backdrops`

## 架构差异

原版有完整的 `Backdrop` 类层次（抽象基类 + `Parallax` + 各种程序化/着色器背景：`Snow`、`StarsBG`、`WindSnowFG`、`MirrorFG`、`ReflectionFG`、`BlackholeBG`、`RainFG`、`Planets`、`Starfield`、`Petals`、`HeatWave`、`CoreStarsFG`、`StardustFG`、`Tentacles`、`NorthernLights`、`Godrays`、`FinalBossStarfield`……）。
Ruleste 只实现 `parallax`（贴图视差层），其余类型一律**跳过并告警**。渲染由宿主 `draw_backdrops` 承担（插件并非实体），这和原版把 Backdrop 挂进 `RendererList` 的架构对应。

## 一、字段/属性逐项对比

| C# `Backdrop` | Rust `Backdrop` | 状态 |
|--------------|----------------|------|
| `Position` | `position: Vec2` | ✅ |
| `Scroll`（默认 1,1） | `scroll: Vec2`（默认 1,1） | ✅ |
| `Speed` | `speed: Vec2`（按秒漂移） | ✅ |
| `Color` | `color: (u8,u8,u8)`（与 alpha 分离） | 🟠 |
| `LoopX` / `LoopY` | `loop_x` / `loop_y` | ✅ |
| `FlipX` / `FlipY` | `flip_x` / `flip_y` | ✅ |
| `Texture`（MTexture） | `texture: String`（图集帧 id） | 🟠 |
| `BlendState`（AlphaBlend/Add） | `additive: bool` | 🟡 |
| `FadeX` / `FadeY` | 缺失 | 🔴 |
| `FadeAlphaMultiplier` | 缺失 | 🔴 |
| `WindMultiplier` | 缺失 | 🔴 |
| `ExcludeFrom` / `OnlyIn` | 缺失 | 🔴 |
| `OnlyIfFlag` / `OnlyIfNotFlag` / `AlsoIfFlag` | 缺失 | 🔴 |
| `Dreaming` | 缺失 | 🔴 |
| `Visible` / `ForceVisible` | 缺失 | 🔴 |
| `InstantIn` / `InstantOut` | 缺失 | 🔴 |
| `Tags` / `Renderer` / `Name` | 缺失 | 🔴 |

## 二、`MapData.CreateBackdrops` / `ParseBackdrop`（构造）

Rust `backdrops::parse` 只处理 `Style > Backgrounds` 和 `Style > Foregrounds`，然后对每个子元素调用 `parse_backdrop`。
对照原版：

| 原版行为 | Rust 行为 | 状态 |
|---------|----------|------|
| `apply` 分组继承 | `parse_group` 对 `apply` 子元素统一以父作 `above` 处理 | ✅ |
| 属性继承（子优先，其次 `above`） | `get`/`get_color` 闭包：子有属性则用子，否则查 `above` | ✅ |
| `atlas="game"/"gui"/其他` 选择 | 忽略；统一从活动图集取 `texture` | 🟠 |
| `blendmode="additive"` | `additive`（子/`above` 优先） | ✅ |
| `fadeIn` | 缺失（DoFadeIn） | 🔴 |
| `x/y/scrollx/scrolly/speedx/speedy` | ✅ 一致 | ✅ |
| `color`（HexToColor） | `hex_color` 解析 RRGGBB | ✅ |
| `alpha`（乘到 Color 上） | 独立 `alpha: f32` 字段 | 🟠 |
| `flipx/flipy/loopx/loopy` | ✅ | ✅ |
| `wind` | 缺失 | 🔴 |
| `exclude` / `only` | 缺失 | 🔴 |
| `flag` / `notflag` / `always` | 缺失 | 🔴 |
| `dreaming` | 缺失 | 🔴 |
| `instantIn` / `instantOut` | 缺失 | 🔴 |
| `fadex` / `fadey`（Fader 段） | 缺失 | 🔴 |
| 未知背景类型 | 原版 `throw new Exception`；Rust `eprintln!` 跳过 | 🟡 |

## 三、`Parallax.Update`

| C# | Rust | 状态 |
|-----|------|------|
| `Position += Speed * dt` | `Backdrop::update`：`position += speed*dt` | ✅ |
| `Position += WindMultiplier * Wind * dt` | 缺失（无风系统） | 🔴 |
| `DoFadeIn` 渐显 | 缺失 | 🔴 |

## 四、`Parallax.Render`

| C# | Rust `draw_backdrops` | 状态 |
|-----|-------------------|------|
| `camera.Position + CameraOffset`（floor） | `self.camera`（宿主已含 shake；`CameraOffset` 恒 0） | 🟡 |
| `(Position - vector*Scroll).floor()` | `(b.position - cam*b.scroll).floor()` | ✅ |
| `fadeIn * Alpha * FadeAlphaMultiplier` 且 `color *= num` | `b.alpha` 乘到 RGB/Alpha | 🟠 |
| `LoopX/LoopY` 归一到图框 | `while sx<0 / >0 +=/-= tw` | ✅ |
| 双层循环按 `Texture.Width/Height` 覆盖 320×180 | 按 untrim box `offset.w/h` 步进 | ✅ |
| 翻转组合 | `copy_ex(flip_x, flip_y)` | ✅ |
| 帧绘制含 `offset`（子图未裁剪区域） | `x-ox, y-oy` | ✅ |

Rust 版用**裁剪过的独立纹理**（`upload_backdrop` 抠出 clip），而原版直接 `MTexture.Draw`；视觉上等价，但 Rust 的 `tw/th` 用未裁剪框步进、绘制用裁剪后尺寸，行为与原版一致。

## 五、缺失功能及影响

1. **程序化背景**（Snow / Stars / Tentacles / Planets / Blackhole / HeatWave / Rain / Petals / Godrays / MirrorFG / ReflectionFG / NorthernLights / CoreStarsFG / StardustFG / FinalBossStarfield / WindSnowFG / DreamStars）—— 全部未实现。`eprintln!` 跳过。
2. **可见性 / 房间过滤**（OnlyIn / ExcludeFrom / flag / notflag / dreaming / ForceVisible）—— 缺失会让一些按章节、梦境、旗标显隐的贴图永远显示。
3. **FadeX/FadeY/FadeAlphaMultiplier** —— 缺失会导致横向/纵向渐隐背景（如远景边缘）不淡出。
4. **风偏移** —— 无 `Wind` 系统。
5. **CameraOffset** —— Rust 的背景直接跟随宿主 `camera.position`（已含 shake），与背景独立偏移略有差异。

## 六、建议优先级

| 优先级 | 项目 | 原因 |
|---|------|------|
| P0 | 房间/旗标可见性（OnlyIn/exclude/flag/dreaming/ForceVisible） | 原图很多 `parallax` 依赖它；不做会整章错误显隐 |
| P1 | FadeX/FadeY/Fader | 远景边缘渐隐、深色章节 |
| P1 | 程序化背景 | 山顶/镜之寺/核心/暴风关卡核心视觉 |
| P2 | Wind | 需要引擎级风系统 |
| P3 | blendmode/alpha 细化 | 现有近似够用 |

> 结论：Ruleste 的 `parallax` 静态贴图视差已对齐核心算法；差距集中在**非贴图背景**与**可见性/旗标/渐隐**两类。
