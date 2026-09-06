# Scene.cs → Rust 对比分析

> 对比 `references/source/Celeste/Monocle/Scene.cs`（C# 原版 Monocle 引擎 Scene 类）
> 与 Ruleste 的 `src/engine/level.rs`、`src/engine/ecs.rs`、`src/engine/physics.rs`、`src/main.rs` 等模块。

## 架构说明

C# 的 `Scene` 是 Monocle 引擎的核心抽象：管理实体列表、渲染器列表、标签列表、碰撞检测、时间追踪和生命周期钩子。在 Ruleste 中，**不存在单一的 `Scene` 等价物**——其职责被分散到多个模块：

| C# Scene 职责 | Rust 等价位置 |
|---|---|
| 实体注册/生命周期 | `src/engine/ecs.rs` — `World` |
| 关卡/地图/碰撞网格 | `src/engine/level.rs` — `Level`、`Room` |
| 碰撞检测（网格/实体） | `src/engine/physics.rs` — `SolidGrid` |
| 生命周期（update/render） | `src/main.rs` 主循环 |
| 相机 | `src/engine/camera.rs` — `Camera` |
| 渲染 | `src/interface/renderer.rs` — `Renderer` |
| 实体行为 | Wasm 插件（`plugins/`） |

---

## 1. 属性 / 字段

### `Paused` (bool)
| | |
|---|---|
| **C#** | 控制 `TimeActive` 是否累加、`Entities.Update()` / `RendererList.Update()` 是否执行 |
| **Rust** | 🔴 **缺失** — 无全局暂停标志；主循环始终调用 `wasm_host.update(dt)`；dt 始终传入 |
| **差异** | 原版 `Engine.Scene.Paused = true` 可冻结整个场景（包括渲染器更新）；Ruleste 无此机制 |

### `TimeActive` (float)
| | |
|---|---|
| **C#** | 仅在 `!Paused` 时累加 `Engine.DeltaTime` |
| **Rust** | 🔴 **缺失** — 主循环有 `dt` 计算（`frame_start.elapsed()`），但无场景级累计时间 |
| **差异** | Wasm 插件可在自己的状态中追踪时间，但没有场景级等价物 |

### `RawTimeActive` (float)
| | |
|---|---|
| **C#** | 始终累加 `Engine.RawDeltaTime`（不受 Paused 影响） |
| **Rust** | 🔴 **缺失** — 无 raw/delta 时间区分 |
| **差异** | 原版用于粒子/视觉效果在暂停时继续运行 |

### `Focused` (bool)
| | |
|---|---|
| **C#** | `Begin()` 时设为 true，`End()` 时设为 false |
| **Rust** | 🔴 **缺失** — 无焦点跟踪 |
| **差异** | Ruleste 无多场景切换（仅主循环一个场景） |

### `Entities` (EntityList)
| | |
|---|---|
| **C#** | `EntityList` 管理所有实体的增删查改，支持深度排序 |
| **Rust** | 🟠 **部分实现** — `World`（`ecs.rs`）提供 `spawn()`/`despawn()`/`get()`/`iter()`，但无排序/标签/深度排序 |
| **差异** | `World` 是 flat `HashMap<u32, Entity>`，无 `EntityList` 的 add/remove batch、排序、深度排序逻辑 |

### `TagLists` (TagLists)
| | |
|---|---|
| **C#** | 按 BitTag 分组的实体列表，支持 `scene[Tags.Environment]` 查询 |
| **Rust** | 🔴 **缺失** — 无标签系统；Wasm 插件自行管理实体分组 |
| **差异** | 原版 `BitTag` 位掩码系统完全未移植；碰撞查询依赖 `SolidGrid` 网格或插件自行实现 |

### `RendererList` (RendererList)
| | |
|---|---|
| **C#** | 管理场景的渲染器列表（Backdrops、HUD 等） |
| **Rust** | 🟠 **部分实现** — 渲染直接在 `main.rs` 主循环中按顺序调用 `renderer.draw_*()` |
| **差异** | 无 `RendererList` 抽象；渲染顺序硬编码在主循环中（背景→碰撞网格→实体→前景） |

### `HelperEntity` (Entity)
| | |
|---|---|
| **C#** | 无标签/无组件的辅助实体，供引擎内部使用 |
| **Rust** | 🔴 **缺失** — 无等价物 |

### `Tracker` (Tracker)
| | |
|---|---|
| **C#** | 按类型追踪所有实体和组件，支持 `Tracker.Entities[typeof(T)]` 查询 |
| **Rust** | 🔴 **缺失** — 无运行时类型追踪；插件通过实体类型名（字符串）匹配，由 `index_entities_by_type()` 在加载时建立索引 |

### `this[BitTag]` (索引器)
| | |
|---|---|
| **C#** | `scene[Tags.Solid]` 快捷访问标签列表 |
| **Rust** | 🔴 **缺失** — 无标签索引器 |

### `OnEndOfFrame` (event)
| | |
|---|---|
| **C#** | 帧末回调事件，`AfterUpdate()` 中触发 |
| **Rust** | 🔴 **缺失** — 无等价事件机制 |

---

## 2. 生命周期方法

### `Scene()` (构造函数)
| | |
|---|---|
| **C#** | 初始化 `Tracker`、`Entities`、`TagLists`、`RendererList`、`HelperEntity` |
| **Rust** | 🟡 **近似** — `World::new()` 初始化实体注册；`Level::load()` 加载地图数据；`Renderer::new()` 初始化 SDL |
| **差异** | 初始化分散在 `main()` 的多个步骤中，非单一构造函数 |

### `Begin()`
| | |
|---|---|
| **C#** | 设 `Focused = true`，遍历所有实体调用 `entity.SceneBegin(this)` |
| **Rust** | 🟠 **部分实现** — `wasm_host.enter_room()` 为当前房间生成实体（等价于 SceneBegin 的实体激活部分） |
| **差异** | 无 `Focused` 标志；无遍历调用 `SceneBegin` 的机制；插件通过 `entity_init` FFI 获取通知 |

### `End()`
| | |
|---|---|
| **C#** | 设 `Focused = false`，遍历所有实体调用 `entity.SceneEnd(this)` |
| **Rust** | 🔴 **缺失** — 无显式场景结束逻辑；`enter_room()` 隐式 despawn 旧房间实体 |
| **差异** | 原版的 `SceneEnd` 清理回调不存在；插件在 despawn 时无显式清理钩子 |

### `BeforeUpdate()`
| | |
|---|---|
| **C#** | 若 `!Paused` 累加 `TimeActive`；累加 `RawTimeActive`；更新 `Entities`/`TagLists`/`RendererList` 的待处理列表 |
| **Rust** | 🟠 **部分实现** — `main.rs` 中的 `dt` 计算、事件 pump、`input.pump(events, dt)`、`wasm_host.reload_plugins()` 对应该阶段 |
| **差异** | 无 `TimeActive` 累加；无列表排序/批量操作（`UpdateLists`） |

### `Update()`
| | |
|---|---|
| **C#** | 若 `!Paused`，调用 `Entities.Update()` 和 `RendererList.Update()` |
| **Rust** | ✅ **对齐** — `wasm_host.update(dt)` 更新所有 Wasm 插件的 `entity_update`；`audio.update(dt)`；`backdrop.update(dt)` |
| **差异** | 更新顺序相同（实体→渲染器→音频）；暂停逻辑不在引擎层 |

### `AfterUpdate()`
| | |
|---|---|
| **C#** | 触发 `OnEndOfFrame` 事件并清空 |
| **Rust** | 🔴 **缺失** — 无帧末事件机制 |

### `BeforeRender()`
| | |
|---|---|
| **C#** | 调用 `RendererList.BeforeRender()` |
| **Rust** | 🟡 **近似** — `renderer.clear()` 对应该阶段 |
| **差异** | 无独立的 `BeforeRender` 钩子；渲染直接在主循环中执行 |

### `Render()`
| | |
|---|---|
| **C#** | 调用 `RendererList.Render()` |
| **Rust** | ✅ **对齐** — 主循环依次调用 `renderer.draw_backdrops()`、`renderer.draw_solids()`、`wasm_host.draw()`、`renderer.draw_entities()`、`renderer.draw_hitboxes()`、`renderer.present()` |
| **差异** | 渲染顺序硬编码；无 `RendererList` 动态管理 |

### `AfterRender()`
| | |
|---|---|
| **C#** | 调用 `RendererList.AfterRender()` |
| **Rust** | 🟡 **近似** — `renderer.present()` 提交帧；无独立 AfterRender |
| **差异** | 无渲染后回调（如后处理） |

### `HandleGraphicsReset()` / `HandleGraphicsCreate()`
| | |
|---|---|
| **C#** | 图形设备重置/创建时的回调（如窗口大小变化） |
| **Rust** | 🔴 **缺失** — SDL3 通过逻辑分辨率 + LETTERBOX 自动处理缩放 |
| **差异** | SDL3 的 `set_logical_size(320, 180, LETTERBOX)` 替代了原版的手动处理 |

### `GainFocus()` / `LoseFocus()`
| | |
|---|---|
| **C#** | 空虚方法，供子类覆写 |
| **Rust** | 🔴 **缺失** — 无焦点事件处理 |

---

## 3. 时间工具方法

### `OnInterval(float)` / `OnInterval(float, float)`
| | |
|---|---|
| **C#** | 判断当前帧是否跨越了指定间隔（基于 `TimeActive`） |
| **Rust** | 🔴 **缺失** — 插件需自行实现时间间隔检测 |
| **差异** | 此为便利方法，可在插件中用 `fmod(time, interval)` 实现 |

### `BetweenInterval(float)`
| | |
|---|---|
| **C#** | 判断当前帧是否处于两个间隔之间 |
| **Rust** | 🔴 **缺失** |

### `OnRawInterval(float)` / `OnRawInterval(float, float)`
| | |
|---|---|
| **C#** | 基于 `RawTimeActive` 的间隔检测（不受暂停影响） |
| **Rust** | 🔴 **缺失** |

### `BetweenRawInterval(float)`
| | |
|---|---|
| **C#** | 基于 `RawTimeActive` 的间隔间检测 |
| **Rust** | 🔴 **缺失** |

---

## 4. 碰撞检测方法（按标签查询）

原版 Scene 提供了大量基于 `BitTag` 的碰撞查询方法，分三组：
- **按标签** (`CollideCheck(point, tag)` 等)
- **按泛型类型 T** (`CollideCheck<T>(point)` 等)
- **按组件类型 T** (`CollideCheckByComponent<T>(point)` 等)

### 按标签查询

| C# 方法 | Rust 等价 | 状态 | 差异 |
|---|---|---|---|
| `CollideCheck(Vector2 point, int tag)` | `SolidGrid::collide_rect()` + `entity_collide()` | 🟡 近似 | 标签查询无直接等价；碰撞基于网格或实体 hitbox |
| `CollideCheck(Vector2 from, Vector2 to, int tag)` | `SolidGrid::line_of_sight()` | 🟡 近似 | 仅检测网格碰撞，不检测实体 |
| `CollideCheck(Rectangle rect, int tag)` | `SolidGrid::collide_rect()` | 🟡 近似 | 网格碰撞可用；实体碰撞需通过 `entity_collide()` |
| `CollideCheck(Rectangle rect, Entity entity)` | `SolidGrid::entity_collide()` | 🟡 近似 | 逻辑类似，但参数不同（world ID vs entity ref） |
| `CollideFirst(...)` | 🔴 缺失 | 🔴 | 无返回第一个匹配实体的方法 |
| `CollideInto(...)` | 🔴 缺失 | 🔴 | 无收集匹配实体到列表的方法 |
| `CollideAll(...)` | 🔴 缺失 | 🔴 | 无返回所有匹配实体的方法 |
| `CollideDo(...)` | 🔴 缺失 | 🔴 | 无遍历匹配实体并执行回调的方法 |
| `LineWalkCheck(...)` | `SolidGrid::line_of_sight()` | 🟠 部分实现 | 原版逐步检测实体碰撞；Rust 仅检测网格碰撞 |

### 按泛型类型 T 查询

| C# 方法 | Rust 等价 | 状态 | 差异 |
|---|---|---|---|
| `CollideCheck<T>(point)` | 🔴 缺失 | 🔴 | 无基于类型的碰撞查询；插件需自行实现 |
| `CollideCheck<T>(from, to)` | 🔴 缺失 | 🔴 | |
| `CollideCheck<T>(rect)` | 🔴 缺失 | 🔴 | |
| `CollideFirst<T>(point)` | 🔴 缺失 | 🔴 | |
| `CollideFirst<T>(from, to)` | 🔴 缺失 | 🔴 | |
| `CollideFirst<T>(rect)` | 🔴 缺失 | 🔴 | |
| `CollideInto<T>(point, hits)` | 🔴 缺失 | 🔴 | |
| `CollideInto<T>(from, to, hits)` | 🔴 缺失 | 🔴 | |
| `CollideInto<T>(rect, hits)` | 🔴 缺失 | 🔴 | |
| `CollideAll<T>(point)` | 🔴 缺失 | 🔴 | |
| `CollideAll<T>(from, to)` | 🔴 缺失 | 🔴 | |
| `CollideAll<T>(rect)` | 🔴 缺失 | 🔴 | |
| `CollideDo<T>(point, action)` | 🔴 缺失 | 🔴 | |
| `CollideDo<T>(from, to, action)` | 🔴 缺失 | 🔴 | |
| `CollideDo<T>(rect, action)` | 🔴 缺失 | 🔴 | |
| `LineWalkCheck<T>(from, to, precision)` | 🔴 缺失 | 🔴 | |

### 按组件类型 T 查询

| C# 方法 | Rust 等价 | 状态 | 差异 |
|---|---|---|---|
| `CollideCheckByComponent<T>(...)` | 🔴 缺失 | 🔴 | Ruleste 无 Component 系统 |
| `CollideFirstByComponent<T>(...)` | 🔴 缺失 | 🔴 | |
| `CollideIntoByComponent<T>(...)` | 🔴 缺失 | 🔴 | |
| `CollideAllByComponent<T>(...)` | 🔴 缺失 | 🔴 | |
| `CollideDoByComponent<T>(...)` | 🔴 缺失 | 🔴 | |
| `LineWalkCheckByComponent<T>(...)` | 🔴 缺失 | 🔴 | |

### 碰撞检测总结

原版 Scene 的碰撞方法在 Ruleste 中**大部分缺失**。原因是架构差异：
- 原版依赖 `BitTag` 位掩码和 `Tracker` 类型系统
- Ruleste 使用 Wasm 插件 + ECS，碰撞检测下沉到 `SolidGrid`（网格碰撞）和插件自身（实体碰撞）
- 网格碰撞（`collide_rect`、`collide_circle`、`line_of_sight`、`actor_move`）已完整实现
- 实体间碰撞（AABB、圆、射线）需由插件通过 FFI 调用 `SolidGrid` 方法或自行实现

---

## 5. 实体管理方法

### `Add(Entity)` / `Remove(Entity)`
| | |
|---|---|
| **C#** | 向 `Entities` 列表添加/移除实体 |
| **Rust** | 🟡 **近似** — `World::spawn()` 创建实体返回 ID；`World::despawn(id)` 移除 |
| **差异** | Rust 使用 ID 而非引用；添加/移除由 `WasmHost::enter_room()` 批量管理 |

### `Add(IEnumerable<Entity>)` / `Remove(IEnumerable<Entity>)`
| | |
|---|---|
| **C#** | 批量添加/移除实体 |
| **Rust** | 🟡 **近似** — `WasmHost::enter_room()` 批量 spawn 新房间实体 |
| **差异** | 无通用批量 API；房间切换是唯一的批量操作 |

### `Add(params Entity[])` / `Remove(params Entity[])`
| | |
|---|---|
| **C#** | 数组参数版本的批量添加/移除 |
| **Rust** | 🟡 **近似** — 同上 |

### `CreateAndAdd<T>()`
| | |
|---|---|
| **C#** | 从对象池创建并添加实体 |
| **Rust** | 🟡 **近似** — `World::spawn()` 创建实体（无对象池） |
| **差异** | 无对象池机制；Wasm 插件负责实体创建逻辑 |

### `GetEnumerator()`
| | |
|---|---|
| **C#** | 遍历所有实体（`IEnumerable<Entity>`） |
| **Rust** | ✅ **对齐** — `World::iter()` 返回所有实体的迭代器 |
| **差异** | 语义一致 |

### `GetEntitiesByTagMask(int mask)`
| | |
|---|---|
| **C#** | 按标签位掩码过滤实体 |
| **Rust** | 🔴 **缺失** — 无标签位掩码；插件自行过滤 |

### `GetEntitiesExcludingTagMask(int mask)`
| | |
|---|---|
| **C#** | 排除指定标签的实体 |
| **Rust** | 🔴 **缺失** |

### `Add(Renderer)` / `Remove(Renderer)`
| | |
|---|---|
| **C#** | 管理渲染器列表 |
| **Rust** | 🟡 **近似** — 渲染器在主循环中硬编码；无动态添加/移除 |

---

## 6. 深度管理

### `SetActualDepth(Entity)`
| | |
|---|---|
| **C#** | 计算实体的实际深度（基于 `depth` + 小偏移避免重复），标记列表为未排序 |
| **Rust** | 🔴 **缺失** — `Entity.depth` 字段存在（`ecs.rs`），但无深度排序机制；渲染顺序由插件 draw 调用顺序决定 |
| **差异** | 原版通过深度排序确保实体渲染顺序正确；Ruleste 依赖插件的 draw 调用顺序 |

### `actualDepthLookup` (Dictionary)
| | |
|---|---|
| **C#** | 缓存已分配的深度偏移，避免重复 |
| **Rust** | 🔴 **缺失** |

---

## 7. 实用方法

### `index_entities_by_type()`
| | |
|---|---|
| **C#** | 不存在（原版使用 `Tracker` 实现） |
| **Rust** | ✅ **新增** — `level.rs` 中的 `index_entities_by_type()` 在加载时按类型名索引实体 spawn |
| **差异** | 这是 Ruleste 特有的辅助函数，用于高效加载所需插件 |

---

## 8. 差异总结

| 状态 | 数量 | 说明 |
|---|---|---|
| ✅ 完全对齐 | 2 | `Update()`、`GetEnumerator()` |
| 🟠 部分实现 | 5 | `Begin()`、`BeforeUpdate()`、`Render()` 相关、实体批量操作 |
| 🟡 近似 | 8 | 构造、`BeforeRender()`、`AfterRender()`、碰撞网格方法、实体管理 |
| 🔴 缺失 | ~45+ | 所有碰撞查询方法、标签系统、类型追踪、时间工具、深度排序、焦点事件等 |

### 核心架构差异

1. **实体行为外置**：C# 原版实体行为在 `Entity.Update()` 中；Ruleste 外置到 Wasm 插件
2. **标签系统缺失**：C# 的 `BitTag` + `TagLists` 完全未移植；插件自行管理实体分组
3. **碰撞下沉**：原版 Scene 级碰撞方法下沉到 `SolidGrid` 网格碰撞和插件自身
4. **渲染硬编码**：原版 `RendererList` 动态管理替换为主循环硬编码渲染顺序
5. **暂停机制缺失**：无场景级暂停；依赖 dt 传入（可设 dt=0 实现类似效果）
6. **时间追踪缺失**：原版的 `TimeActive`/`RawTimeActive` 和间隔工具方法未移植

### 建议补全优先级

| 优先级 | 内容 | 原因 |
|---|---|---|
| **P0** | 场景级暂停 + `TimeActive` | 插件需要统一时间源 |
| **P1** | 实体碰撞查询 API（`collide_check_point` 等） | 插件频繁需要此类查询 |
| **P1** | `OnInterval` 等时间工具 | 插件需要帧级间隔检测 |
| **P2** | 标签系统 | 支持更高效的主题查询（如 "all enemies"） |
| **P2** | `SetActualDepth` 深度排序 | 渲染顺序正确性保证 |
| **P3** | `RendererList` 动态管理 | 支持运行时渲染器切换 |
| **P3** | `OnEndOfFrame` 事件 | 帧末延迟操作支持 |
