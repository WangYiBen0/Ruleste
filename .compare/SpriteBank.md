# SpriteBank 对比分析

> C# 原版: `references/source/Celeste/Monocle/SpriteBank.cs`
> Rust 实现: `crates/ruleste-core/src/data/spritebank.rs`（307 行）

## 类型/结构体对应

| C# | Rust | 状态 |
|----|------|------|
| `SpriteData` | `SpriteData` | ✅ |
| `Animation` | `Animation` | ✅ |
| `Chooser<string>` | `Chooser`（加权 + PRNG 选择） | ✅ |
| `MTexture[] Frames` | `Animation.frames: Vec<u32>` + 运行时 `SpriteAnimator::frames` 解析 | 🟠 |
| `Dictionary<string, AnimationData>`（按 animation id） | `animations: HashMap<String, Animation>` | ✅ |
| `Atlas`（纹理池） | 不持有；`path` 前缀按需从 `Atlas` 解析 | 🟠 |

## 一、数据字段逐项

### `SpriteData`

| C# | Rust | 状态 |
|----|------|------|
| `name` | `name` | ✅ |
| `Path` | `path` | ✅ |
| `Start` | `start` | ✅ |
| `Origin` | `origin: (i32,i32)` | ✅ |
| `Center`/`Justify`（`<Center/>`/`<Justify/>`） | `center: bool` / `justify: Option<(f32,f32)>` | ✅ |
| `useRawDeltaTime` | 缺失（统一 dt） | 🔴 |
| `Goto`（Sprite 级默认） | 缺失（动画级 goto 有） | 🟠 |

### `AnimationData` / `Animation`

| C# | Rust | 状态 |
|----|------|------|
| `FrameRate`（延迟属性） | `delay`（秒/帧；C# `FrameRate` 是帧率） | 🟠 语义换算一致 |
| `Frames`（`MTexture[]`） | `frames: Vec<u32>` + atlas 前缀解析 | ✅ |
| `Chooser<string> Goto` | `goto: Option<Chooser>` | ✅ |
| `Loop` | `is_loop: bool`（`Loop` 元素） | ✅ |
| `Add`（单次播放 Anim） | `Anim` 元素解析 | ✅ |
| `Delay` 随机/偏移 | 缺失 | 🟠 |

## 二、`copy` 继承

Rust `parse_sprite` **已实现** `copy="src"`：从已解析 `existing` 克隆源 `SpriteData`，当前元素覆盖 `path/start/origin/center/justify` 并按动画 id 合并（当前覆盖源的同名 id）。对照 C# 的 `SpriteBank.cs`（`CopyData`）逐项：

| 行为 | C# | Rust | 状态 |
|------|-----|------|------|
| copy 复制源动画/原点/中心/justify | ✅ | ✅ | ✅ |
| 当前元素覆盖 path/start/origin | ✅ | ✅（start 覆盖规则见下） | ✅ |
| 动画合并（当前覆盖同名） | ✅ | ✅ | ✅ |
| `start` 覆盖语义（含“仅当显式指定时覆盖复制源”） | ✅ | ✅（`el.attribute("start").is_some() || (源非空 && 请求非 idle)`） | ✅ |

**注意**：`copy` 引用的源必须在**之前**出现过（Rust 单遍 `existing`），与原版依赖 XML 中复制目标须先定义一致。

## 三、`Chooser`

C# `Chooser.FromString("idle:10,flash:2,blink")` 权重选择；Rust `parse_goto` 同格式，`Chooser::choose` 用宿主 xorshift64 做加权随机。✅ 对齐。

## 四、解析与运行时拆分

C# `SpriteBank.CreateOn(entity)` / `Create()` 在统一 Sprite 类上实例化动画。Rust 把**解析（数据）与播放（engine/sprites.rs）分离**：`SpriteBank` 只做静态定义，`SpriteAnimator` 负责帧序号展开（`atlas_subtexture_count`/`subtexture_key`）、delay、loop/goto、随机起始帧。这是架构差异，不是缺陷。

## 五、差异总结

| 维度 | 状态 | 说明 |
|------|------|------|
| copy 继承 | ✅ | 当前实现含在内 |
| 重复名称检测 | 🟠 | C# 抛异常，Rust 静默覆盖 |
| 大小写不敏感查找 | 🔴 | C# `OrdinalIgnoreCase`，Rust 精确匹配（原版 XML 实际大小写一致，影响小） |
| `useRawDeltaTime` | 🔴 | 暂停/慢动作粒子差异 |
| `reverse`/`PlayOffset` 等运行时 | 在 `Sprite.md` 讨论 | 播放层 |
| Create/CreateOn | 架构分离合理 | 不在 SpriteBank 层 |

## 六、已对齐/待补

- ✅ **全部 XML 属性**：`path/start/copy/center/justify/origin`、`Anim/Loop` 的 `id/path/delay/frames/goto`
- ✅ 加权 goto 运行时选择
- 🟠 重复名称、大小写、randomize 属 SPIKE 层
