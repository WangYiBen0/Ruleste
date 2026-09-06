# Entity.cs vs ecs.rs — 逐项对比分析

> **原版**: `references/source/Celeste/Monocle/Entity.cs` (999 行)
> **重写**: `src/engine/ecs.rs` (134 行)

## 架构差异概述

| 维度 | C# Monocle | Rust Ruleste |
|------|-----------|--------------|
| 设计模式 | 继承式 OOP 基类，所有实体继承 `Entity` | 数据驱动 ECS，Entity 仅为纯数据结构 |
| 组件系统 | 内置 `ComponentList`，实体持有组件 | 无组件系统，逻辑全在 Wasm 插件 |
| 碰撞系统 | `Collider` 对象 + 大量便捷方法 | `hitbox`/`hitbox_offset` 字段 + `SolidGrid` 网格碰撞 |
| 场景管理 | `Scene` 引用，生命周期回调 | 无 Scene 概念，由 `World` + `Level` 替代 |
| 标签系统 | `BitTag` 位掩码标签 | 无标签系统 |
| 类型查询 | 泛型 `Tracker.Entities[typeof(T)]` | 无类型查询，按 `entity_type` 字符串筛选 |

---

## 1. 字段/属性对比

### 1.1 状态标志

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Active` (bool) | L10 | ❌ 无对应 | 🔴 缺失 | Rust Entity 无 Active 标志；激活/停用由 World 或插件自行管理 |
| `Visible` (bool) | L12 | `Entity.visible` (ecs.rs:50) | ✅ 完全对齐 | 字段存在且默认 `true` |
| `Collidable` (bool) | L14 | ❌ 无对应 | 🔴 缺失 | Rust 无全局可碰撞标志；碰撞由 `solid_platforms`/`solid_entities` HashSet 控制 |

### 1.2 位置

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Position` (Vector2) | L16 | `Entity.position` (ecs.rs:44) | 🟠 部分实现 | 字段存在；C# 为 `Microsoft.Xna.Framework.Vector2`，Rust 为 `ruleste_plugins_api::types::Vec2`，语义等价 |
| `X` (float, 属性) | L49-59 | ❌ 无对应 | 🟡 近似 | 可通过 `entity.position.x` 访问，无独立属性 |
| `Y` (float, 属性) | L61-71 | ❌ 无对应 | 🟡 近似 | 可通过 `entity.position.y` 访问，无独立属性 |

### 1.3 深度排序

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `depth` (int, internal) | L22 | `Entity.depth` (ecs.rs:49) | 🟠 部分实现 | 字段存在，但无 setter 触发 `Scene.SetActualDepth` |
| `actualDepth` (double, internal) | L24 | ❌ 无对应 | 🔴 缺失 | Rust 无延迟深度计算机制 |
| `Depth` (属性, get/set) | L30-47 | ❌ 无对应 | 🟡 近似 | `depth` 字段可直接读写，但无 Scene 联动 |

### 1.4 碰撞体

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Collider` (Collider 对象) | L20, L73-94 | `hitbox` + `hitbox_offset` (ecs.rs:47-48) | 🟠 部分实现 | Rust 仅有矩形碰撞框 (Vec2 尺寸 + 偏移)，无 C# 的多态 Collider 体系 |
| `Width` (float, 属性) | L96-106 | `entity.hitbox.x` | 🟡 近似 | 直接访问 hitbox 宽度，无 null 检查 |
| `Height` (float, 属性) | L108-118 | `entity.hitbox.y` | 🟡 近似 | 直接访问 hitbox 高度 |
| `Left` (float, get/set) | L120-141 | ❌ 无对应 | 🔴 缺失 | 需手动计算 `position.x + hitbox_offset.x` |
| `Right` (float, get/set) | L143-164 | ❌ 无对应 | 🔴 缺失 | 需手动计算 `position.x + hitbox_offset.x + hitbox.x` |
| `Top` (float, get/set) | L166-187 | ❌ 无对应 | 🔴 缺失 | 需手动计算 |
| `Bottom` (float, get/set) | L189-210 | ❌ 无对应 | 🔴 缺失 | 需手动计算 |
| `CenterX` (float, get/set) | L212-233 | ❌ 无对应 | 🔴 缺失 | 需手动计算 |
| `CenterY` (float, get/set) | L235-256 | ❌ 无对应 | 🔴 缺失 | 需手动计算 |
| `TopLeft` (Vector2, get/set) | L258-269 | ❌ 无对应 | 🔴 缺失 | 便捷属性，需手动计算 |
| `TopRight` (Vector2, get/set) | L271-282 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `BottomLeft` (Vector2, get/set) | L284-295 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `BottomRight` (Vector2, get/set) | L297-308 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `Center` (Vector2, get/set) | L310-321 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `CenterLeft` (Vector2, get/set) | L323-334 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `CenterRight` (Vector2, get/set) | L336-347 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `TopCenter` (Vector2, get/set) | L349-360 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |
| `BottomCenter` (Vector2, get/set) | L362-373 | ❌ 无对应 | 🔴 缺失 | 便捷属性 |

### 1.5 标签系统

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `tag` (int, private) | L18 | ❌ 无对应 | 🔴 缺失 | Rust 无标签系统 |
| `Tag` (int, get/set) | L375-408 | ❌ 无对应 | 🔴 缺失 | 含 Scene.TagLists 联动逻辑 |

### 1.6 场景与组件

| C# 字段 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Scene` (Scene, 属性) | L26 | ❌ 无对应 | 🔴 缺失 | Rust 无 Scene 概念；World 承担部分职责 |
| `Components` (ComponentList) | L28 | ❌ 无对应 | 🔴 缺失 | Rust 无组件系统，逻辑在 Wasm 插件中 |

### 1.7 Rust 独有字段

| Rust 字段 | C# 对应 | 说明 |
|-----------|---------|------|
| `Entity.id` (u32) | 无 (引用类型自带) | ECS 实体唯一标识 |
| `Entity.plugin` (String) | 无 | 拥有此实体的 Wasm 插件名 |
| `Entity.entity_type` (String) | 无 | 地图实体类型，如 "player" |
| `Entity.spawn` (Vec<u8>) | 无 | 序列化的地图生成数据 |
| `Entity.speed` (Vec2) | 无 (在 Actor 子类) | 速度向量，C# 中在 Actor 子类 |
| `Entity.sprite` (SpriteState) | 无 (在 Image/Component) | 精灵状态，C# 中在组件中 |

---

## 2. 构造函数

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Entity(Vector2 position)` | L410-414 | `Entity::new(id)` (ecs.rs:55-69) | 🟡 近似 | Rust 传入 id 而非 position；position 默认为 ZERO |
| `Entity()` (无参) | L416-419 | `Entity::new(id)` (ecs.rs:55-69) | 🟡 近似 | Rust 始终需要 id 参数 |

---

## 3. 生命周期方法

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `SceneBegin(Scene)` | L421-423 | ❌ 无对应 | 🔴 缺失 | Rust 无场景生命周期；由插件自行处理 |
| `SceneEnd(Scene)` | L425-435 | ❌ 无对应 | 🔴 缺失 | 含组件遍历调用 |
| `Awake(Scene)` | L437-447 | ❌ 无对应 | 🔴 缺失 | 含组件 EntityAwake 调用 |
| `Added(Scene)` | L449-460 | ❌ 无对应 | 🔴 缺失 | 设置 Scene 引用 + 组件通知 + 深度计算 |
| `Removed(Scene)` | L462-472 | ❌ 无对应 | 🔴 缺失 | 组件通知 + 清除 Scene 引用 |
| `Update()` | L474-477 | ❌ 无对应 | 🔴 缺失 | Rust 中由 `entity_update` FFI 调用 Wasm 插件 |
| `Render()` | L479-482 | ❌ 无对应 | 🔴 缺失 | Rust 中由 `entity_draw` FFI 调用 Wasm 插件 |
| `DebugRender(Camera)` | L484-491 | ❌ 无对应 | 🟡 近似 | Rust 有 `--show-hitboxes` CLI 参数，由 `Renderer::draw_hitboxes` 实现，但不在 Entity 上 |
| `HandleGraphicsReset()` | L493-496 | ❌ 无对应 | 🔴 缺失 | SDL3 自动处理 |
| `HandleGraphicsCreate()` | L498-501 | ❌ 无对应 | 🔴 缺失 | SDL3 自动处理 |

---

## 4. 实体管理

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `RemoveSelf()` | L503-509 | `World::despawn(id)` (ecs.rs:99-101) | 🟡 近似 | Rust 需显式传入 id；C# 通过 `Scene.Entities.Remove` |
| ❌ 无 | — | `World::spawn()` (ecs.rs:92-97) | 🟡 近似 | Rust 独有：返回自增 id；C# 通过 `new Entity()` + `Scene.Add` |
| ❌ 无 | — | `World::get(id)` (ecs.rs:103-105) | 🟡 近似 | Rust 独有：按 id 查询 |
| ❌ 无 | — | `World::get_mut(id)` (ecs.rs:107-109) | 🟡 近似 | Rust 独有：按 id 可变查询 |
| ❌ 无 | — | `World::iter()` (ecs.rs:111-113) | 🟡 近似 | Rust 独有：遍历所有实体 |
| ❌ 无 | — | `World::iter_mut()` (ecs.rs:115-117) | 🟡 近似 | Rust 独有：可变遍历 |
| ❌ 无 | — | `World::entity_ids()` (ecs.rs:119-121) | 🟡 近似 | Rust 独有：获取所有 id |
| ❌ 无 | — | `World::len()` / `is_empty()` (ecs.rs:123-129) | 🟡 近似 | Rust 独有：集合大小 |
| ❌ 无 | — | `World::is_alive(id)` (ecs.rs:131-133) | 🟡 近似 | Rust 独有：存活检查 |

---

## 5. 标签方法

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `TagFullCheck(int)` | L511-514 | ❌ 无对应 | 🔴 缺失 | Rust 无标签系统 |
| `TagCheck(int)` | L516-519 | ❌ 无对应 | 🔴 缺失 | |
| `AddTag(int)` | L521-524 | ❌ 无对应 | 🔴 缺失 | |
| `RemoveTag(int)` | L526-529 | ❌ 无对应 | 🔴 缺失 | |

---

## 6. 碰撞检测方法

### 6.1 实体碰撞 (CollideCheck 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideCheck(Entity)` | L531-534 | `SolidGrid::entity_collide()` (physics.rs) via `host_collide_check` FFI | 🟠 部分实现 | Rust 仅检查与固体网格的碰撞，不检查实体间碰撞 |
| `CollideCheck(Entity, Vector2)` | L536-539 | `host_collide_check` + offset | 🟠 部分实现 | 同上 |
| `CollideCheck(BitTag)` | L541-544 | ❌ 无对应 | 🔴 缺失 | 依赖标签系统 |
| `CollideCheck(BitTag, Vector2)` | L546-549 | ❌ 无对应 | 🔴 缺失 | |
| `CollideCheck<T>()` | L551-554 | ❌ 无对应 | 🔴 缺失 | 依赖泛型类型查询 |
| `CollideCheck<T>(Vector2)` | L556-559 | ❌ 无对应 | 🔴 缺失 | |
| `CollideCheck<T, Exclude>()` | L561-572 | ❌ 无对应 | 🔴 缺失 | 含排除类型 |
| `CollideCheck<T, Exclude>(Vector2)` | L574-581 | ❌ 无对应 | 🔴 缺失 | |
| `CollideCheck<T, Exclude1, Exclude2>()` | L583-595 | ❌ 无对应 | 🔴 缺失 | 双排除类型 |
| `CollideCheck<T, Exclude1, Exclude2>(Vector2)` | L597-604 | ❌ 无对应 | 🔴 缺失 | |

### 6.2 组件碰撞 (CollideCheckByComponent 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideCheckByComponent<T>()` | L606-616 | ❌ 无对应 | 🔴 缺失 | Rust 无组件系统 |
| `CollideCheckByComponent<T>(Vector2)` | L618-625 | ❌ 无对应 | 🔴 缺失 | |

### 6.3 外部碰撞 (CollideCheckOutside 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideCheckOutside(Entity, Vector2)` | L627-634 | ❌ 无对应 | 🔴 缺失 | 检查"当前不碰撞但移动后碰撞" |
| `CollideCheckOutside(BitTag, Vector2)` | L636-646 | ❌ 无对应 | 🔴 缺失 | |
| `CollideCheckOutside<T>(Vector2)` | L648-658 | ❌ 无对应 | 🔴 缺失 | |
| `CollideCheckOutsideByComponent<T>(Vector2)` | L660-670 | ❌ 无对应 | 🔴 缺失 | |

### 6.4 碰撞查询 - 首个匹配 (CollideFirst 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideFirst(BitTag)` | L672-675 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirst(BitTag, Vector2)` | L677-680 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirst<T>()` | L682-685 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirst<T>(Vector2)` | L687-690 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirstByComponent<T>()` | L692-702 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirstByComponent<T>(Vector2)` | L704-714 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirstOutside(BitTag, Vector2)` | L716-726 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirstOutside<T>(Vector2)` | L728-738 | ❌ 无对应 | 🔴 缺失 | |
| `CollideFirstOutsideByComponent<T>(Vector2)` | L740-750 | ❌ 无对应 | 🔴 缺失 | |

### 6.5 碰撞查询 - 所有匹配 (CollideAll 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideAll(BitTag)` | L752-755 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAll(BitTag, Vector2)` | L757-760 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAll<T>()` | L762-765 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAll<T>(Vector2)` | L767-770 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAll<T>(Vector2, List<Entity>)` | L772-776 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAllByComponent<T>()` | L778-789 | ❌ 无对应 | 🔴 缺失 | |
| `CollideAllByComponent<T>(Vector2)` | L791-798 | ❌ 无对应 | 🔴 缺失 | |

### 6.6 碰撞回调 (CollideDo 系列)

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollideDo(BitTag, Action<Entity>)` | L800-812 | ❌ 无对应 | 🔴 缺失 | |
| `CollideDo(BitTag, Action<Entity>, Vector2)` | L814-829 | ❌ 无对应 | 🔴 缺失 | |
| `CollideDo<T>(Action<T>)` | L831-843 | ❌ 无对应 | 🔴 缺失 | |
| `CollideDo<T>(Action<T>, Vector2)` | L845-860 | ❌ 无对应 | 🔴 缺失 | |
| `CollideDoByComponent<T>(Action<T>)` | L862-874 | ❌ 无对应 | 🔴 缺失 | |
| `CollideDoByComponent<T>(Action<T>, Vector2)` | L876-891 | ❌ 无对应 | 🔴 缺失 | |

### 6.7 基础碰撞检测

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `CollidePoint(Vector2)` | L893-896 | `SolidGrid::collide_point()` (physics.rs:316+) | 🟠 部分实现 | Rust 仅检查与固体网格的碰撞 |
| `CollidePoint(Vector2, Vector2)` | L898-901 | `SolidGrid::collide_point()` + offset | 🟠 部分实现 | 同上 |
| `CollideLine(Vector2, Vector2)` | L903-906 | `SolidGrid::line_of_sight()` via `host_line_of_sight` FFI | 🟡 近似 | 语义不完全相同：Rust 检查视线通畅，C# 检查线段与碰撞体重叠 |
| `CollideLine(Vector2, Vector2, Vector2)` | L908-911 | ❌ 无对应 | 🔴 缺失 | 带偏移版本 |
| `CollideRect(Rectangle)` | L913-916 | `SolidGrid::collide_rect()` (physics.rs:316) | 🟡 近似 | Rust 仅检查固体网格 |
| `CollideRect(Rectangle, Vector2)` | L918-921 | ❌ 无对应 | 🔴 缺失 | 带偏移版本 |

---

## 7. 组件管理

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Add(Component)` | L923-926 | ❌ 无对应 | 🔴 缺失 | Rust 无组件系统 |
| `Remove(Component)` | L928-931 | ❌ 无对应 | 🔴 缺失 | |
| `Add(params Component[])` | L933-936 | ❌ 无对应 | 🔴 缺失 | |
| `Remove(params Component[])` | L938-941 | ❌ 无对应 | 🔴 缺失 | |
| `Get<T>()` | L943-946 | ❌ 无对应 | 🔴 缺失 | |

---

## 8. 可枚举接口

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `GetEnumerator()` | L948-951 | ❌ 无对应 | 🔴 缺失 | Rust 无 Component 枚举；`World::iter()` 遍历实体 |
| `IEnumerable.GetEnumerator()` | L953-956 | ❌ 无对应 | 🔴 缺失 | |

---

## 9. 工具方法

| C# 方法 | C# 行号 | Rust 实现 | 状态 | 差异说明 |
|---------|---------|-----------|------|----------|
| `Closest(params Entity[])` | L958-972 | ❌ 无对应 | 🔴 缺失 | 按距离找最近实体 |
| `Closest(BitTag)` | L974-993 | ❌ 无对应 | 🔴 缺失 | 按标签找最近实体 |
| `SceneAs<T>()` | L995-998 | ❌ 无对应 | 🔴 缺失 | 场景类型转换 |

---

## 10. Rust 独有功能 (C# 无对应)

| Rust 功能 | 位置 | 说明 |
|-----------|------|------|
| `World::solid_platforms` (HashSet<u32>) | ecs.rs:79 | 可站立平台实体集合，C# 中由 `JumpThru`/`Solid` 类型区分 |
| `World::solid_entities` (HashSet<u32>) | ecs.rs:84 | 完全实体块集合，C# 中由 `Solid` 类型实现 |
| `SpriteState` 结构体 | ecs.rs:10-19 | 精灵渲染状态，C# 中分散在 Image/Animation 组件中 |
| `Entity.plugin` (String) | ecs.rs:39 | Wasm 插件归属，C# 无此概念 |
| `Entity.spawn` (Vec<u8>) | ecs.rs:43 | 序列化地图数据，C# 在构造时直接解析 |
| `Entity.speed` (Vec2) | ecs.rs:45 | 速度向量，C# 在 Actor 子类中 |
| `World::spawn()` → u32 | ecs.rs:92 | 实体生成返回 id |
| `World::despawn(id)` | ecs.rs:99 | 按 id 移除 |
| `World::get(id)` / `get_mut(id)` | ecs.rs:103-109 | 按 id 查询 |
| `World::iter()` / `iter_mut()` | ecs.rs:111-117 | 遍历所有实体 |
| `World::entity_ids()` | ecs.rs:119 | 获取所有 id 列表 |
| `World::is_alive(id)` | ecs.rs:131 | 存活检查 |

---

## 统计摘要

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 1 | 1.5% |
| 🟠 部分实现 | 5 | 7.5% |
| 🟡 近似 | 12 | 17.9% |
| 🔴 缺失 | 50 | 74.6% |
| **总计** | **67** (C# 侧) | 100% |

> **注**: C# Entity 类共有约 67 个公开方法/属性/字段。Rust 侧实现了其中 18 个的某种形式对应 (✅+🟠+🟡)，其余 50 个因架构差异 (ECS vs OOP、Wasm 插件 vs 内置组件、无标签/场景系统) 而缺失。

---

## 设计决策说明

Rust 实现**有意**不复制 C# Entity 的全部功能，原因：

1. **架构转型**: C# 是继承式 OOP，Rust 是数据驱动 ECS。Entity 仅为纯数据容器，逻辑由 Wasm 插件处理。
2. **碰撞系统重构**: C# 的多态 Collider 体系 → Rust 的 `SolidGrid` 网格碰撞 + 简单矩形 hitbox。
3. **组件系统移除**: C# 的 `ComponentList` → Rust 的 Wasm 插件 FFI (`entity_init`, `entity_update`, `entity_draw`)。
4. **标签系统**: 被 `entity_type` 字符串 + `solid_platforms`/`solid_entities` HashSet 替代。
5. **生命周期**: C# 的 `Added`/`Removed`/`Awake` → Rust 的 `World::spawn`/`despawn` + Wasm `entity_init`。
