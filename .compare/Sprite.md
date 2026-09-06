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
| 17 | `Animating` | `bool` | 隐含于 `SpriteState.animation` 非空 | 🟡 近似 | C# 用显式 bool；Rust 以 `animation.is_empty()` 判断。当动画结束后 C# 设 `Animating = false` + `AnimationID = ""`；Rust 的 `update()` 在 goto=None 时把 frame 钉在最后一帧但不清空 animation id。 |
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
| 1 | `Play(string id, bool restart, bool randomizeFrame)` | Plugin API: `Sprite::play(name)` + `host_sprite_play` | 🟡 近似 | Rust `play` 只接受动画名，无 `restart` 参数（总是重置 frame=0）。无 `randomizeFrame` 支持。`host_sprite_play` 直接设 `animation = s; frame = 0.0`。 |
| 2 | `PlayOffset(string id, float offset, bool restart)` | — | 🔴 缺失 | 没有带偏移量的播放方法。需要通过 `set_frame()` 手动实现。 |
| 3 | `Reverse(string id, bool restart)` | `Sprite::set_rate()` 设负值 | 🟡 近似 | C# 先 Play 再将 Rate 取反。Rust 需要手动：先 `play()` 再 `set_rate(-1.0)`。无封装方法。 |

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
| 1 | `Stop()` | `Sprite::play("")` 或 `set_frame(0)` + 清空 | 🔴 缺失 | 没有显式 Stop 方法。插件可以设 `animation = ""` 但无专门 API。 |
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
| 非动画时提前返回 | `if (!Animating) return;` | `filter(!sprite.animation.is_empty())` | ✅ 等价 |
| DeltaTime 选择 | `UseRawDeltaTime ? RawDeltaTime : DeltaTime` | 直接使用传入 `dt` | 🔴 缺失 `UseRawDeltaTime` |
| 累加计时器 | `animationTimer += dt * Rate` | `frame += rate * dt / anim.delay` | 🟡 数学等价（将 delay 除法内联） |
| 到帧判定 | `Math.Abs(animationTimer) >= currentAnimation.Delay` | `frame >= len`（整数部分越界） | 🟡 等价（Rust 用帧数而非时间比较） |
| 帧推进 | `CurrentAnimationFrame += Sign(animationTimer)` | `frame` 浮点自增，整数部分隐式推进 | 🟡 等价 |
| 计时器归还 | `animationTimer -= Sign * Delay` | 余数保留在 `frame` 小数部分 | ✅ 等价 |
| 越界处理：回调 | 触发 `OnLastFrame` 检查 | 无回调 | 🔴 缺失 |
| 越界处理：Goto 分支 | 检查 `currentAnimation.Goto`，切换动画，触发 `OnChange`/`OnLoop` | 检查 `anim.goto`，切换动画，frame=0 | 🟡 核心逻辑对齐，缺回调 |
| 越界处理：无 Goto | 钉帧到最后一帧，`Animating=false`，清空状态，触发 `OnFinish` | 钉帧 `frame = len - 1.0`，不清空 animation | 🟠 部分实现 |
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
2. **`Chooser<string>` 多目标 goto** — 影响动画状态机分支
3. **`OnFinish` / `OnLoop` / `OnChange` 回调** — 部分实体依赖回调触发行为（如粒子生成、状态切换）
4. **`PlayOffset`** — 玩家/助推器等实体可能需要偏移播放
5. **`Stop()` 方法** — 插件需要显式停止动画

### 中优先级（影响完整度）
6. **`LastAnimationID`** — 部分实体需要知道上一次动画
7. **`CloneInto`** — 复制实体时需要
8. **`DrawSubrect`** — 部分效果可能用到
9. **`randomizeFrame`** — 部分环境动画需要随机初始帧

### 低优先级（调试/边缘用例）
10. **`LogAnimations`** — 调试用
11. **`PlayRoutine` / `ReverseRoutine`** — 协程在 ECS 架构下需要替代方案
12. **`ClearAnimations`** — 当前架构下不需要
13. **直接 `MTexture[]` 的 Add/AddLoop 重载** — Celeste 原版极少使用
