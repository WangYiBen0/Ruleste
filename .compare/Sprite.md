# Sprite 对比分析：C# Monocle vs Rust Ruleste

> 对比文件：`references/source/Celeste/Monocle/Sprite.cs` vs `src/engine/sprites.rs`
> 
> ⚠️ **架构差异**：C# 原版将所有状态和逻辑集中在 `Sprite` 类中（继承 `Image`）。Rust 版本采用 ECS 架构，将职责拆分为：
> - **`SpriteState`**（`src/engine/ecs.rs`）— 精灵状态组件
> - **`SpriteAnimator`**（`src/engine/sprites.rs`）— 动画推进逻辑
> - **`SpriteData` / `Animation` / `SpriteBank`**（`src/data/spritebank.rs`）— 数据定义（从 XML 解析）
> - **Plugin API `Sprite`**（`crates/ruleste-plugins-api/src/host.rs`）— 插件侧 FFI 接口
> - **`wasm_host`**（`src/hotload/wasm_host.rs`）— 宿主侧 FFI 实现

---

## 1. 字段 / 属性

| # | C# 成员 | 类型 | Rust 对应 | 状态 | 说明 |
|---|---------|------|-----------|------|------|
| 1 | `Rate` | `float` | `SpriteState.rate` / `SpriteAnimator::update()` 使用 | ✅ 完全对齐 | C# 默认 `1.0`；Rust `SpriteState::default()` 也是 `1.0`。逻辑相同：`frame += rate * dt / delay` |
| 2 | `UseRawDeltaTime` | `bool` | — | 🔴 缺失 | Rust 版始终使用传入的 `dt`，没有区分 `RawDeltaTime` vs `DeltaTime`。原版用于跳过暂停帧的动画，可能需要在调用侧处理。 |
| 3 | `Justify` | `Vector2?` | `SpriteData.justify: Option<(f32, f32)>` | ✅ 完全对齐 | 数据模型一致；但原版在 `SetFrame` 中动态计算 `Origin`，Rust 版由渲染器读取 `justify` 字段处理。 |
| 4 | `OnFinish` | `Action<string>` | — | 🔴 缺失 | Rust 版没有回调机制。动画结束时无通知。 |
| 5 | `OnLoop` | `Action<string>` | — | 🔴 缺失 | 同上，循环动画切换时无回调。 |
| 6 | `OnFrameChange` | `Action<string>` | — | 🔴 缺失 | 无帧变化回调。插件通过每帧轮询 `animation()` 检测变化。 |
| 7 | `OnLastFrame` | `Action<string>` | — | 🔴 缺失 | 无回调。 |
| 8 | `OnChange` | `Action<string, string>` | — | 🔴 缺失 | 无动画切换回调。 |
| 9 | `atlas` | `Atlas` | `SpriteAnimator.atlas: &Atlas` | ✅ 完全对齐 | 引用方式不同（C# 是字段，Rust 是生命周期引用）。 |
| 10 | `Path` | `string` | `SpriteData.path: String` | ✅ 完全对齐 | 语义一致，均作为图集子纹理路径前缀。 |
| 11 | `animations` | `Dictionary<string, Animation>` | `SpriteData.animations: HashMap<String, Animation>` | ✅ 完全对齐 | 结构一致。Rust 版 `Animation` 结构更丰富（含 `is_loop`、`path` 等）。 |
| 12 | `currentAnimation` | `Animation` (private) | 通过 `SpriteState.animation: String` + `SpriteBank` 间接查找 | 🟡 近似 | Rust 不缓存当前 Animation 引用，每次通过 id 查 `SpriteBank`。语义等价但性能特征不同。 |
| 13 | `animationTimer` | `float` (private) | 隐含在 `SpriteState.frame` 的小数部分 | 🟡 近似 | C# 用 `animationTimer` 累加后减去 delay；Rust 直接用浮点帧索引 `frame += rate * dt / delay`。数学等价但精度模型不同。 |
| 14 | `width` | `int` (private) | — | 🔴 缺失 | Rust 版不维护 sprite 宽高，渲染时从纹理直接获取。 |
| 15 | `height` | `int` (private) | — | 🔴 缺失 | 同上。 |
| 16 | `Center` | `Vector2` (computed) | — | 🔴 缺失 | Rust 版不提供此计算属性；渲染器自行处理。 |
| 17 | `Animating` | `bool` | `SpriteState.finished`（取反语义） | ✅ 对齐 | C# 用 `Animating`；Rust 用 `finished` 标志表达同一状态。播完后两者都停止推进，**且都保留 animation id 与末帧**（C# 保留 `Texture`，早期 Rust 清空 id 会让实体消失，已修复）。 |
| 18 | `CurrentAnimationID` | `string` | `SpriteState.animation: String` | ✅ 完全对齐 | 语义一致。 |
| 19 | `LastAnimationID` | `string` | — | 🔴 缺失 | Rust 版不追踪上一次动画 id。 |
| 20 | `CurrentAnimationFrame` | `int` | `SpriteState.frame: f32` 的整数部分 | 🟡 近似 | C# 用整数索引；Rust 用浮点帧索引。取整方式：C# 直接整数；Rust 用 `floor() as usize`。 |
| 21 | `CurrentAnimationTotalFrames` | `int` (computed) | `SpriteAnimator::frames().len()` | ✅ 完全对齐 | 可通过 `frames()` 获取。 |

---

## 2. 构造函数

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Sprite(Atlas atlas, string path)` | `SpriteAnimator::new(atlas, bank)` + `SpriteState::default()` | 🟡 近似 | C# 构造时绑定图集和路径，初始化 animations 字典。Rust 拆分为：`SpriteAnimator` 持有图集/bank 引用；`SpriteState` 由 ECS 默认初始化。逻辑等价。 |
| 2 | `Sprite()` (internal) | `SpriteState::default()` | ✅ 完全对齐 | 无参构造，Rust 通过 `Default` trait 实现。 |

---

## 3. Reset

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Reset(Atlas, string)` | — | 🔴 缺失 | Rust 没有重置方法。每个实体创建时 `SpriteState::default()` 即为初始状态；不支持运行时重绑图集。在当前架构中可能不需要（sprite bank 全局共享）。 |

---

## 4. 动画注册方法

### 4.1 `AddLoop` 系列

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `AddLoop(string id, string path, float delay)` | `SpriteBank::from_xml()` 中解析 `<Loop>` 节点 | ✅ 完全对齐 | Rust 版从 XML 声明式定义，等价于 C# 的命令式注册。`Animation.is_loop = true` + `goto = Some(id)` 实现自循环。 |
| 2 | `AddLoop(string id, string path, float delay, int[] frames)` | `SpriteBank::from_xml()` 中 `frames` 属性解析 | ✅ 完全对齐 | `parse_frames()` 支持 `"0,1,2"` 和 `"2-7"` 语法。 |
| 3 | `AddLoop(string id, float delay, MTexture[] frames)` | — | 🔴 缺失 | Rust 版不支持直接传入纹理数组作为动画帧（仅支持从图集按前缀/索引解析）。但实际 Celeste 没有使用此重载。 |

### 4.2 `Add` 系列

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Add(string id, string path)` | `SpriteBank::from_xml()` 中解析 `<Anim>` 节点（delay 默认 0.1） | 🟡 近似 | C# delay=0 表示单帧；Rust XML 解析默认 delay=0.1。实际使用中 Anim 通常显式指定 delay。 |
| 2 | `Add(string id, string path, float delay)` | `SpriteBank::from_xml()` 中 `<Anim delay="...">` | ✅ 完全对齐 | |
| 3 | `Add(string id, string path, float delay, int[] frames)` | `SpriteBank::from_xml()` 中 `frames` 属性 | ✅ 完全对等 | |
| 4 | `Add(string id, string path, float delay, string into)` | `<Anim goto="...">` | ✅ 完全对齐 | `goto` 字段对应 `Chooser.FromString` 解析后的目标。 |
| 5 | `Add(string id, string path, float delay, Chooser<string> into)` | `<Anim goto="...">` | 🟡 近似 | Rust 版的 `goto` 是 `Option<String>`（单目标），不支持 `Chooser<string>` 的多概率分支。原版 `Chooser` 允许加权随机选择。 |
| 6 | `Add(string id, string path, float delay, string into, int[] frames)` | 组合 | ✅ 完全对齐 | |
| 7 | `Add(string id, float delay, string into, MTexture[] frames)` | — | 🔴 缺失 | 同 AddLoop(3)，不支持直接纹理数组。 |
| 8 | `Add(string id, string path, float delay, Chooser<string> into, int[] frames)` | — | 🔴 缺失 | 不支持 Chooser 多目标 + 帧选择组合。 |

---

## 5. Play / 播放控制

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Play(string id, bool restart, bool randomizeFrame)` | `Sprite::play(name)` + `Sprite::play_with(name, restart, randomize_frame)` | ✅ 对齐 | `host_sprite_play_flags` 已按 C# 三参数实现：`restart` 重置帧计时，`randomizeFrame` 取随机起始帧（`frame` 设为大随机数，下一 tick 由 `% len` 收敛成 0..len 相位）。 |
| 2 | `PlayOffset(string id, float offset, bool restart)` | `Sprite::play_offset(name, offset, restart)` + `host_sprite_play_offset` | ✅ 对齐 | 按 0..1 相位换算成浮点帧索引（宿主把 C# 的 `animationTimer` 折进了帧小数位）。`delay==0` 的动画按原版忽略 offset 并钉在帧 0；`restart=false` 且动画已在播时为 no-op。 |
| 3 | `Reverse(string id, bool restart)` | `Sprite::reverse(name, restart)` | ✅ 对齐 | 先 `Play` 再在 `Rate > 0` 时取反；配合下方 `update` 的负向边界处理即可倒放。 |
| 4 | `Stop()` | `Sprite::stop()` | ✅ 对齐 | `play("")` 清空 animation id，实体不再绘制该动画。 |

---

## 6. 查询方法

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Has(string id)` | `SpriteData.animation(id)` 返回 `Option` | ✅ 完全对齐 | Rust 用 `Option` 模式替代 bool 检查，更惯用。 |
| 2 | `GetFrame(string animation, int frame)` | `SpriteAnimator::current_frame(sprite, anim, frame)` | 🟡 近似 | C# 返回 `MTexture`；Rust 返回 `Option<String>`（frame id 字符串，由渲染器查找纹理）。 |

---

## 7. 停止 / 清除

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Stop()` | `Sprite::stop()` | ✅ 对齐 | `play("")` 清空 animation id。 |
| 2 | `ClearAnimations()` | — | 🔴 缺失 | Rust 版动画定义在 `SpriteBank` 中（全局共享），不支持运行时清除。设计上不需要。 |

---

## 8. 克隆

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `CreateClone()` / `CloneInto(Sprite)` | — | 🔴 缺失 | Rust ECS 中每个实体有独立 `SpriteState`，复制时只需 clone 组件即可。但没有显式的深拷贝动画字典逻辑，因为动画数据在 `SpriteBank` 中共享。 |

---

## 9. 核心 Update 逻辑

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Update()` | `SpriteAnimator::update(world, dt)` | 🟡 近似 | 见下方逐行对比 |

### `Update()` vs `SpriteAnimator::update()` 逐行差异

| 行为 | C# 原版 | Rust 实现 | 差异 |
|------|---------|-----------|------|
| 非动画时提前返回 | `if (!Animating) return;` | `filter(!animation.is_empty() && !finished)` | ✅ 等价 |
| DeltaTime 选择 | `UseRawDeltaTime ? RawDeltaTime : DeltaTime` | 直接使用传入 `dt` | 🔴 缺失 `UseRawDeltaTime` |
| 累加计时器 | `animationTimer += dt * Rate` | `frame += rate * dt / anim.delay` | 🟡 数学等价（将 delay 除法内联） |
| 到帧判定 | `Math.Abs(animationTimer) >= currentAnimation.Delay` | `frame >= len`（整数部分越界） | 🟡 等价（Rust 用帧数而非时间比较） |
| 帧推进 | `CurrentAnimationFrame += Sign(animationTimer)` | `frame` 浮点自增，整数部分隐式推进 | 🟡 等价 |
| 计时器归还 | `animationTimer -= Sign * Delay` | 余数保留在 `frame` 小数部分 | ✅ 等价 |
| **上下边界** | `if (Frame < 0 \|\| Frame >= Frames.Length)` | 同样检查两端，负向越界回绕到 `len-1` | ✅ 对齐（本次修复） |
| 越界处理：回调 | 触发 `OnLastFrame` 检查 | 无回调 | 🔴 缺失 |
| 越界处理：Goto 分支 | 检查 `currentAnimation.Goto`，切换动画，触发 `OnChange`/`OnLoop` | 检查 `anim.goto`，切换动画 | 🟡 核心逻辑对齐，缺回调 |
| 越界处理：无 Goto | 钉帧（`Frame < 0` 钉 0，否则钉 `len-1`），`Animating=false`，触发 `OnFinish` | 同样按越界方向钉帧，`finished=true`，**保留 animation id** | ✅ 对齐（本次修复） |
| 正常帧更新 | `SetFrame(frames[CurrentAnimationFrame])` | 由 `current_frame()` 返回 frame id，渲染器处理 | ✅ 等价（职责在渲染器） |

---

## 10. SetFrame

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `SetFrame(MTexture)` | `current_frame()` + 渲染器 | 🟡 近似 | C# 更新 `Texture`/`Origin`/`width`/`height` 并触发 `OnFrameChange`。Rust 仅返回 frame id 字符串，渲染器负责纹理查找和绘制。Origin/Justify 由渲染器读取 `SpriteData`。无回调。 |

---

## 11. SetAnimationFrame

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `SetAnimationFrame(int frame)` | Plugin API: `Sprite::set_frame(f32)` | 🟡 近似 | C# 取模并立即 `SetFrame`。Rust 只设 `frame` 值，下次 `update()` 时使用。C# 是整数，Rust 是浮点。 |

---

## 12. DrawSubrect

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `DrawSubrect(Vector2 offset, Rectangle rect)` | — | 🔴 缺失 | 子区域绘制方法。Rust 渲染器未实现。实际游戏中使用较少。 |

---

## 13. LogAnimations

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `LogAnimations()` | — | 🔴 缺失 | 调试辅助方法。不影响功能。 |

---

## 14. 协程相关

| # | C# 方法 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `PlayRoutine(string, bool)` | — | 🔴 缺失 | C# 协程，等待动画播完。Rust 无协程机制。 |
| 2 | `ReverseRoutine(string, bool)` | — | 🔴 缺失 | 同上。 |
| 3 | `PlayUtil()` (private) | — | 🔴 缺失 | 协程工具方法。 |

---

## 15. Chooser 支持

| # | C# 机制 | Rust 对应 | 状态 | 说明 |
|---|---------|-----------|------|------|
| 1 | `Chooser<string>` — 加权随机动画选择 | `Animation.goto: Option<String>` — 单目标 | 🔴 缺失 | 原版 `Chooser` 支持多目标加权随机（如 `goto="a:0.5,b:0.5"`）。Rust 版 `goto` 仅支持单目标字符串。如果游戏逻辑中存在加权分支（如粒子特效的随机动画），将无法正确复现。 |

---

## 汇总统计

> 下表为本次修复前的基线；`Play` / `PlayOffset` / `Reverse` / `Stop` /
> 负向倒放边界 / 播完保留末帧 均已在本轮补齐，见文末补记。

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 13 | 32% |
| 🟠 部分实现 | 1 | 2% |
| 🟡 近似 | 12 | 30% |
| 🔴 缺失 | 15 | 37% |

---

## 优先级建议

### 高优先级（影响游戏逻辑正确性）
1. **`UseRawDeltaTime`** — 影响暂停场景下的动画行为
2. **`Chooser<string>` 多目标 goto** — 影响动画状态机分支（`SpriteBank.md` 已实现加权选择，播放层已消费 `Option<Chooser>`）
3. **`OnFinish` / `OnLoop` / `OnChange` 回调** — 部分实体依赖回调触发行为（如粒子生成、状态切换）
4. ~~**`PlayOffset`**~~ — ✅ 本轮已实现（`Sprite::play_offset`）
5. ~~**`Stop()` 方法**~~ — ✅ 本轮已实现（`Sprite::stop`）

### 中优先级（影响完整度）
6. **`LastAnimationID`** — 部分实体需要知道上一次动画
7. **`CloneInto`** — 复制实体时需要
8. **`DrawSubrect`** — 部分效果可能用到
9. ~~**`randomizeFrame`**~~ — ✅ 已实现（`Sprite::play_with` 第三参数）

### 低优先级（调试/边缘用例）
10. **`LogAnimations`** — 调试用
11. **`PlayRoutine` / `ReverseRoutine`** — 协程在 ECS 架构下需要替代方案
12. **`ClearAnimations`** — 当前架构下不需要
13. **直接 `MTexture[]` 的 Add/AddLoop 重载** — Celeste 原版极少使用
14. ~~**`Reverse`**~~ — ✅ 本轮已实现（`Sprite::reverse`）

---

## 9. 当前时间点补记：`copy` 与重叠框架

- **`copy` 继承已实现**（见 `SpriteBank.md`）：`plugins` 使用 `<SpriteName copy="src"/>` 时，Rust 会克隆源 `SpriteData` 并合并动画，起点/原点/中心/justify 覆盖规则与 C# `SpriteBank.cs` 对齐。这消除了早期"copy 未实现"的遗留差异。
- **重叠帧语义**：Rust `SpriteAnimator::update` 的 `delay="0"` 分支（frame 每 tick +1）与 C# `|animationTimer| >= Delay` 即时判定一致，修复过 `delay=0` 产生 NaN 的问题（`zero_delay_loop_does_not_produce_nan` 单测）。
- **负向倒放语义（本次修复）**：C# 的越界判定是 `CurrentAnimationFrame < 0 || CurrentAnimationFrame >= Frames.Length` **双边**判断，而 Rust 之前只判上界。`Sprite.Reverse` / `sprite.Rate = -1f`（`Trapdoor.cs:48`、`Player.cs:5757,5785` 在原版中真实使用）会让帧索引走到 0 以下，旧实现直接越界成负数（单测实测 `frame = -5`），且 `current_frame()` 的 `frame.floor() as usize` 会把负数饱和成 0 恒定绘制首帧。现已双边判定 + `wrap_index` 回绕，并按越界方向决定钉帧端点（正向钉末帧、倒放钉首帧）。
- **非循环动画播完不清空 animation id**：C# 播完设 `Animating=false` 且 `CurrentAnimationID=""`，但同时把 `Texture` 留在最后一帧上，所以实体仍然可见。Rust 之前清空 animation id 会让实体整个消失；现改为 `finished` 标志位（见 `SpriteState::finished`），保留末帧继续绘制，直到下次 `play`/`set_frame` 复位。

## 10. 播放层（`engine/sprites.rs`）对应

| C# `Sprite` | Rust `SpriteAnimator` | 状态 |
|------------|----------------------|------|
| `Play(id, restart, randomizeFrame)` | `Sprite::play_with` | ✅ 对齐 |
| `PlayOffset(id, offset, restart)` | `Sprite::play_offset` | ✅ 对齐（本次新增） |
| `Reverse(id, restart)` | `Sprite::reverse` | ✅ 对齐（本次新增） |
| `Stop()` | `Sprite::stop` | ✅ 对齐（本次新增） |
| `Update()`（delay/goto/loop/reverse） | `update(world, dt)` | ✅ 对齐（双边边界本次修复） |
| `SetAnimationFrame` | 直接写 `frame` | ✅ |
| `UseRawDeltaTime` | 缺失 | 🔴 |
| `OnFinish/OnLoop/OnChange/OnLastFrame` 回调 | 缺失（host 无事件回调） | 🔴 |
