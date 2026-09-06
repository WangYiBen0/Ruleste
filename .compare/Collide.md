# Collide.cs → physics.rs 对比分析

> **原版**: `references/source/Celeste/Monocle/Collide.cs` (390 行, 静态工具类)
> **实现**: `src/engine/physics.rs` (991 行, 含测试; 基于 `SolidGrid` 的网格碰撞引擎)

## 架构差异概述

原版 `Collide` 是 Monocle 引擎的**通用静态碰撞工具类**，提供实体-实体、点/线/矩形/圆形之间的任意组合碰撞检测。

Rust 端 `physics.rs` 是**网格碰撞引擎**，专注 tile-based 碰撞 + Actor 移动解析（`MoveH`/`MoveV`），设计哲学完全不同：
- 原版：实体持有 `Collider` 组件，`Collide` 类做通用 AABB/形状查询
- Rust：实体只有简单 `hitbox` + `hitbox_offset`，网格碰撞由 `SolidGrid` 处理，实体碰撞由 `solid_entity_hitbox`/`entity_collide` 特化处理

**结论：两个文件功能重叠极少。** Rust 端缺失了大部分原版通用碰撞工具方法，但在网格碰撞和 Actor 移动方面有远超原版的实现。

---

## 逐方法对比

### 1. `Collide.Check(Entity a, Entity b)` — 实体 AABB 碰撞检测

**状态**: 🟠 部分实现

**原版逻辑**:
```csharp
if (a.Collider == null || b.Collider == null) return false;
if (a != b && b.Collidable) return a.Collider.Collide(b);
return false;
```
通用实体-实体碰撞：任一实体有 Collider 且 `b.Collidable` 为 true 时检测。

**Rust 实现**:
```rust
// solid_entity_hitbox — 仅检查 solid_entities 集合中的实体
fn solid_entity_hitbox(&self, world: &World, exclude: u32, x, y, w, h) -> Option<...>
```

**差异**:
- Rust 只对 `solid_entities` 集合中的实体做 AABB 检测，不是通用 Check
- 没有 `Collider` trait/组件抽象；碰撞检测硬编码在 `SolidGrid` 方法中
- 无 `Collidable` 开关控制
- **未实现**: 任意两个实体间的通用 AABB overlap check

---

### 2. `Collide.Check(Entity a, Entity b, Vector2 at)` — 带临时位置的碰撞检测

**状态**: 🔴 缺失

**原版逻辑**: 临时将 `a.Position` 设为 `at`，执行 Check，然后恢复。

**Rust 实现**: 无对应。Rust 端所有碰撞检测直接读取 `position` 字段，没有临时位置的概念。

**差异**:
- 原版通过临时修改 Entity 位置实现"预判"碰撞
- Rust 端没有类似机制；预判碰撞需调用方自行管理

---

### 3. `Collide.Check(Entity a, IEnumerable<Entity> b)` — 对实体集合的碰撞检测

**状态**: 🟠 部分实现

**原版逻辑**: 遍历集合，任一碰撞即返回 true。

**Rust 近似实现**:
```rust
fn platform_top(&self, world, x, y, w, exclude) -> bool
// 遍历 solid_platforms + solid_entities，找到第一个碰撞即返回
```

**差异**:
- Rust 的遍历是特化的：仅针对 `solid_platforms` / `solid_entities`
- 无通用 `Check(a, &[Entity])` 方法
- 语义不同：原版是"与任意实体碰撞"，Rust 是"与特定类型实体碰撞"

---

### 4. `Collide.Check(Entity a, IEnumerable<Entity> b, Vector2 at)` — 带临时位置

**状态**: 🔴 缺失

无对应实现。

---

### 5. `Collide.First(Entity a, IEnumerable<Entity> b)` — 返回第一个碰撞实体

**状态**: 🟠 部分实现

**原版逻辑**: 遍历集合，返回第一个碰撞的 Entity，无则返回 null。

**Rust 近似实现**:
```rust
fn platform_top_y(&self, world, x, y, w, exclude) -> Option<f32>
fn solid_entity_hitbox(&self, world, exclude, x, y, w, h) -> Option<(f32,f32,f32,f32)>
```

**差异**:
- Rust 返回碰撞几何信息（坐标/尺寸），不返回 Entity 引用
- 返回类型是 `Option<(f32,f32,f32,f32)>` 而非 `Option<Entity>`
- 仅限 `solid_platforms` + `solid_entities`，非通用

---

### 6. `Collide.First(Entity a, IEnumerable<Entity> b, Vector2 at)` — 带临时位置

**状态**: 🔴 缺失

---

### 7-8. `Collide.All(...)` — 返回所有碰撞实体 (3 个重载)

**状态**: 🔴 缺失

**原版逻辑**: 收集所有碰撞实体到 List，支持临时位置版本。

**Rust 实现**: 无对应。Rust 端没有任何"收集所有碰撞实体"的通用方法。

**差异**:
- 完全缺失
- Rust 的迭代器链（`platform_top` 中的 `.find_map`）只找第一个，不收集全部

---

### 9-10. `Collide.All(... Vector2 at)` — 带临时位置的 All

**状态**: 🔴 缺失

---

### 11. `Collide.CheckPoint(Entity a, Vector2 point)` — 点在实体内检测

**状态**: 🔴 缺失

**原版逻辑**: 检测点是否在实体的 Collider 内。

**Rust 实现**: 无独立函数。`collide_rect` 内部有隐式的点-in-rect 逻辑但不暴露。

**差异**:
- 无 `check_point_in_entity` 或类似公开 API
- 原版依赖 `Collider.Collide(point)` 虚方法调用；Rust 无 Collider trait

---

### 12. `Collide.CheckPoint(Entity a, Vector2 point, Vector2 at)` — 带临时位置

**状态**: 🔴 缺失

---

### 13. `Collide.CheckLine(Entity a, Vector2 from, Vector2 to)` — 线段与实体碰撞

**状态**: 🔴 缺失

**原版逻辑**: 线段是否与实体的 Collider 相交。

**Rust 实现**: 无对应。

**差异**:
- `line_of_sight` 是网格采样式的视线检测，不是几何线段-AABB 相交测试
- 目的完全不同：`line_of_sight` 检测路径上是否有 solid tile，不是线段与实体边界相交

---

### 14. `Collide.CheckLine(Entity a, Vector2 from, Vector2 to, Vector2 at)` — 带临时位置

**状态**: 🔴 缺失

---

### 15. `Collide.CheckRect(Entity a, Rectangle rect)` — 矩形与实体碰撞

**状态**: 🟠 部分实现

**原版逻辑**: 检测 Rectangle 是否与实体 Collider 重叠。

**Rust 近似**:
```rust
pub fn collide_rect(&self, x, y, w, h) -> bool
// 检测 (x,y,w,h) 是否与 solid tiles 重叠
```

**差异**:
- `collide_rect` 检测的是与 **grid tiles** 的碰撞，不是与 Entity 的碰撞
- `entity_collide` 是 entity 的 hitbox 与 grid + solid entities 的碰撞
- 无通用的 "rect vs entity collider" 检测

---

### 16. `Collide.CheckRect(Entity a, Rectangle rect, Vector2 at)` — 带临时位置

**状态**: 🔴 缺失

---

### 17. `Collide.LineCheck(Vector2 a1, Vector2 a2, Vector2 b1, Vector2 b2)` — 线段-线段相交

**状态**: 🔴 缺失

**原版逻辑** (L157-178):
```csharp
// 标准线段相交测试：两线段 (a1→a2) 和 (b1→b2) 是否相交
// 使用叉积法：计算参数 t 和 u，均在 [0,1] 内则相交
float num = vector.X * vector2.Y - vector.Y * vector2.X;
if (num == 0f) return false;
float num2 = (vector3.X * vector2.Y - vector3.Y * vector2.X) / num;
if (num2 < 0f || num2 > 1f) return false;
float num3 = (vector3.X * vector.Y - vector3.Y * vector.X) / num;
if (num3 < 0f || num3 > 1f) return false;
return true;
```

**Rust 最接近**:
```rust
pub fn line_of_sight(&self, x1, y1, x2, y2) -> bool
// 网格步进视线检测，完全不同的算法和目的
```

**差异**:
- 这是纯几何问题（两线段是否相交），Rust 端完全缺失
- `line_of_sight` 是网格采样式检测（在 path 上检查 solid tiles），不是线段相交
- 缺失此方法会影响：精确弹射轨迹计算、绳索/绳索碰撞等

---

### 18. `Collide.LineCheck(... out Vector2 intersection)` — 带交点输出的线段相交

**状态**: 🔴 缺失

**原版逻辑** (L180-203): 同上，但额外计算并返回交点坐标 `intersection = a1 + num2 * vector`。

**差异**: 完全缺失。对弹射轨迹、绳索系统等需要交点信息的场景有需求。

---

### 19. `Collide.CircleToLine(Vector2, float, Vector2, Vector2)` — 圆与线段碰撞

**状态**: 🔴 缺失

**原版逻辑** (L205-208):
```csharp
return Vector2.DistanceSquared(cPosiition, Calc.ClosestPointOnLine(lineFrom, lineTo, cPosiition)) < cRadius * cRadius;
```
使用 `Calc.ClosestPointOnLine` 求线段上最近点，然后比较距离平方。

**差异**:
- Rust 端无 `ClosestPointOnLine` 辅助函数
- `circle_to_rect` 仅处理圆与矩形，不处理圆与线段

---

### 20. `Collide.CircleToPoint(Vector2, float, Vector2)` — 圆与点碰撞

**状态**: 🔴 缺失

**原版逻辑** (L210-213):
```csharp
return Vector2.DistanceSquared(cPosition, point) < cRadius * cRadius;
```

**差异**: 极其简单的距离平方比较，但 Rust 端未提供独立函数。

---

### 21. `Collide.CircleToRect(Vector2, float, float, float, float, float)` — 圆与矩形 (float 参数)

**状态**: ✅ 完全对齐

**原版逻辑** (L215-218): 委托给 `RectToCircle`。

**Rust 实现** (L373-382):
```rust
pub fn circle_to_rect(cx, cy, r, x, y, w, h) -> bool {
    let cx_closest = cx.clamp(x, x + w);
    let cy_closest = cy.clamp(y, y + h);
    let dx = cx - cx_closest;
    let dy = cy - cy_closest;
    dx * dx + dy * dy <= r * r
}
```

**差异**:
- ✅ 算法等价：Rust 使用 clamp 求最近点，比原版的 sector-based 方法更简洁但结果一致
- 原版经过 `RectToCircle → GetSector → CircleToLine` 多层委托，Rust 直接用 clamp
- 语义完全相同：圆心到矩形最近点的距离 ≤ 半径

---

### 22. `Collide.CircleToRect(Vector2, float, Rectangle)` — 圆与矩形 (Rectangle 参数)

**状态**: ✅ 完全对齐

**原版逻辑** (L220-223): 委托给 `RectToCircle(Rectangle, ...)`。
**Rust**: 同一个 `circle_to_rect` 函数，Rectangle 重载只是参数包装。

---

### 23. `Collide.RectToCircle(float, float, float, float, Vector2, float)` — 矩形与圆 (float 参数)

**状态**: ✅ 完全对齐

**原版逻辑** (L225-269): 完整的 sector-based 矩形-圆检测：
1. 先检测圆心是否在矩形内 (`RectToPoint`)
2. 用 `GetSector` 确定圆心相对矩形的方位
3. 对相关边调用 `CircleToLine`

**差异**:
- ✅ 算法结果等价，但 Rust 实现更简洁（clamp 法 vs sector 枚举法）
- 原版 `GetSector` + 多边检测 → Rust 一行 clamp + 距离比较
- 数学上完全等价：`clamp(x, x0, x0+w)` 正确处理了圆心在矩形内外所有情况

---

### 24. `Collide.RectToCircle(Rectangle, Vector2, float)` — 矩形与圆 (Rectangle 参数)

**状态**: ✅ 完全对齐

同上，参数包装。

---

### 25. `Collide.RectToLine(float, float, float, float, Vector2, Vector2)` — 矩形与线段

**状态**: 🔴 缺失

**原版逻辑** (L276-326):
1. 检测线段两端点是否在矩形内 (`GetSector`)
2. 若任一端在矩形内 → 相交
3. 若两端在同一外部区域 → 不相交
4. 对相关边调用 `LineCheck` (线段-线段相交)

**差异**: 完全缺失。需要 `GetSector` + `LineCheck` 两个前置函数。

---

### 26. `Collide.RectToLine(Rectangle, Vector2, Vector2)` — 矩形与线段 (Rectangle 参数)

**状态**: 🔴 缺失

参数包装版本，同样缺失。

---

### 27. `Collide.RectToPoint(float, float, float, float, Vector2)` — 点在矩形内 (float 参数)

**状态**: 🔴 缺失

**原版逻辑** (L333-340):
```csharp
if (point.X >= rX && point.Y >= rY && point.X < rX + rW)
    return point.Y < rY + rH;
return false;
```
注意：左闭右开 `[rX, rX+rW)` × `[rY, rY+rH)`。

**差异**: 无独立公开函数。`collide_rect` 内部隐含类似逻辑但不暴露。

---

### 28. `Collide.RectToPoint(Rectangle, Vector2)` — 点在矩形内 (Rectangle 参数)

**状态**: 🔴 缺失

---

### 29. `Collide.GetSector(Rectangle, Vector2)` — 点的区域分类 (Rectangle 参数)

**状态**: 🔴 缺失

**原版逻辑** (L347-367): 返回 `PointSectors` 位标志，标识点在矩形的哪个区域：
```csharp
enum PointSectors {
    Center = 0, Top = 1, Bottom = 2, Left = 4, Right = 8
}
// 返回 Center | Top | Bottom | Left | Right 的组合
```

**差异**: 完全缺失。`RectToLine` 和 `RectToCircle` 依赖此函数。

---

### 30. `Collide.GetSector(float, float, float, float, Vector2)` — 点的区域分类 (float 参数)

**状态**: 🔴 缺失

同上，参数包装版本。

---

## Rust 端额外实现 (原版无对应)

| Rust 方法 | 说明 |
|---|---|
| `SolidGrid::empty(w, h)` | 创建空网格 |
| `SolidGrid::from_rows(rows)` | 从字符串行构建网格 |
| `SolidGrid::blit(src, dx, dy)` | 网格合并/composite |
| `SolidGrid::add_jumpthru(jt)` | 注册单向平台 |
| `SolidGrid::jumpthru_top(x, y, w)` | 跳板顶部检测 |
| `SolidGrid::platform_top(world, x, y, w, exclude)` | 动态平台顶部检测 |
| `SolidGrid::platform_top_y(...)` | 返回平台顶部 y 坐标 |
| `SolidGrid::platform_landing(...)` | 下落时的平台着陆检测 |
| `SolidGrid::mark_solid_platform(world, id, on)` | 标记/取消动态平台 |
| `SolidGrid::mark_solid_entity(world, id, on)` | 标记/取消完全实体块 |
| `SolidGrid::solid_entity_hitbox(...)` | 实体碰撞箱 AABB 检测 |
| `SolidGrid::platform_riders(world, platform_id)` | 获取平台上的骑乘者 |
| `SolidGrid::solid_at(tx, ty)` | 网格单元 solid 查询 |
| `SolidGrid::line_of_sight(x1, y1, x2, y2)` | 网格式视线检测 |
| `SolidGrid::tile_id_at(tx, ty)` | 获取 tile 字符 ID |
| `SolidGrid::tile_id_at_local(tx, ty)` | 本地坐标 tile ID |
| `SolidGrid::collide_rect(x, y, w, h)` | 矩形与 solid tile 碰撞 |
| `SolidGrid::collide_circle(cx, cy, r)` | 圆与 solid tile 碰撞 |
| `SolidGrid::circle_to_circle(...)` | 圆与圆碰撞 (原版无) |
| `SolidGrid::actor_move(world, id, dx, dy)` | Actor 移动解析 (MoveH+MoveV) |
| `SolidGrid::is_grounded(world, id)` | 着地检测 |
| `SolidGrid::entity_collide(world, id, dx, dy)` | 实体偏移碰撞检测 |

---

## 统计摘要

| 状态 | 数量 | 占比 |
|---|---|---|
| ✅ 完全对齐 | 4 | 13% |
| 🟠 部分实现 | 3 | 10% |
| 🟡 近似 | 0 | 0% |
| 🔴 缺失 | 23 | 77% |
| **合计 C# 方法** | **30** | 100% |

### 完全对齐 (4 个)
- `CircleToRect` (float) / `CircleToRect` (Rectangle) → `SolidGrid::circle_to_rect`
- `RectToCircle` (float) / `RectToCircle` (Rectangle) → `SolidGrid::circle_to_rect`

### 部分实现 (3 个)
- `Check(Entity, Entity)` → `solid_entity_hitbox` (仅 solid entities)
- `Check(Entity, IEnumerable)` → `platform_top` / `platform_landing` (仅平台)
- `CheckRect(Entity, Rectangle)` → `collide_rect` (仅 tile grid)

### 完全缺失 (23 个)
- **所有带 `Vector2 at` 临时位置的重载** (6 个): `Check(a,b,at)`, `Check(a,list,at)`, `First(a,list,at)`, `All(a,list,into,at)`, `CheckPoint(a,pt,at)`, `CheckLine(a,from,to,at)`, `CheckRect(a,rect,at)`
- **通用实体查询** (4 个): `All` (3 个重载), `First(a,list)`
- **点/线检测** (4 个): `CheckPoint`, `CheckLine`
- **几何工具** (8 个): `LineCheck` (2 个), `CircleToLine`, `CircleToPoint`, `RectToLine` (2 个), `RectToPoint` (2 个)
- **区域分类** (2 个): `GetSector` (2 个)

---

## 设计差异分析

### 1. 碰撞模型差异
- **原版**: 实体持有 `Collider` 多态组件 (Box, Circle, etc.)，`Collide` 类通过虚方法调用检测
- **Rust**: 实体只有固定 `(hitbox: Vec2, hitbox_offset: Vec2)` 的 AABB，网格碰撞由 `SolidGrid` 处理

### 2. 临时位置机制缺失
- 原版大量使用 `a.Position = at; ... a.Position = position;` 模式实现"预判"碰撞
- Rust 端完全缺失此模式，预判碰撞需调用方自行管理

### 3. 实体碰撞范围不同
- 原版 `Check(a, b)` 检测任意两个有 Collider 的实体
- Rust 仅在 `solid_entities` 集合中的实体参与碰撞（`solid_entity_hitbox`）
- 这是微内核设计的有意选择：通用碰撞检测可能由插件自行实现

### 4. 几何工具函数缺失
- `LineCheck` (线段相交)、`CircleToLine`、`RectToLine`、`GetSector` 等纯几何函数完全缺失
- 这些函数在弹射轨迹计算、绳索系统、粒子碰撞等场景有需求
- 可能由插件通过 host API 自行实现，或在需要时补充

### 5. Rust 端独有能力
- `actor_move`: 完整的 Actor 移动解析（水平/垂直分离、着陆、骑乘者携带）
- `collide_circle` / `circle_to_circle`: 网格圆碰撞
- `line_of_sight`: 网格视线检测
- `blit`: 网格 composite
- 平台骑乘者系统
