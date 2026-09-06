# Hitbox.cs → Rust 对比分析

> **原版**: `references/source/Celeste/Monocle/Hitbox.cs` (199 行)
> **Rust**: `src/engine/physics.rs` + `src/engine/ecs.rs`
>
> **架构差异**: C# 使用多态 `Collider` 类层次（`Hitbox` / `Grid` / `Circle` / `ColliderList`），
> Rust 将碰撞数据内联到 `Entity` 结构体 (`hitbox: Vec2` + `hitbox_offset: Vec2`)，
> 碰撞查询由 `SolidGrid` 方法承担。没有独立的 Hitbox 类型。

---

## 一、字段 / 属性

### 1. `Width` / `Height` — 碰撞箱尺寸

| C# | Rust | 状态 |
|----|------|------|
| `public override float Width { get; set; }` (私有 `width` 字段) | `Entity.hitbox: Vec2` — `.x` = width, `.y` = height | 🟠 部分实现 |

- **差异**: C# 有独立的 `width`/`height` 字段和 `Width`/`Height` 属性；Rust 直接用 `hitbox.x`/`hitbox.y`，没有 setter 语义（直接赋值）。
- **功能上等价**，但 Rust 不能通过 width setter 间接调整 hitbox 矩形。

### 2. `Position` — 碰撞箱相对偏移

| C# | Rust | 状态 |
|----|------|------|
| `Vector2 Position`（继承自 `Collider`，碰撞箱相对实体的位置） | `Entity.hitbox_offset: Vec2` | ✅ 完全对齐 |

- **差异**: 命名不同 (`Position` vs `hitbox_offset`)，语义完全一致——碰撞箱相对实体原点的偏移。

### 3. `Left` / `Top` — 左/上边界

| C# | Rust | 状态 |
|----|------|------|
| `Left { get => Position.X; set => Position.X = value; }` | `e.position.x + e.hitbox_offset.x`（多处计算） | 🟠 部分实现 |

- **差异**: C# 的 `Left` 是可读写属性（set 直接修改 `Position.X`）；Rust 中 left 是只读计算值，没有 setter。
- `Top` 同理：C# `Top { get => Position.Y; }` → Rust `e.position.y + e.hitbox_offset.y`。
- **功能上**，读取对齐，但缺少 setter。

### 4. `Right` / `Bottom` — 右/下边界

| C# | Rust | 状态 |
|----|------|------|
| `Right { get => Position.X + Width; set => Position.X = value - Width; }` | `e.position.x + e.hitbox_offset.x + e.hitbox.x`（只读计算） | 🟠 部分实现 |

- **差异**: C# 的 `Right` set 会反推 `Position.X`；Rust 没有这种反向传播。只在读取场景（碰撞检测）中使用。

### 5. `AbsoluteLeft` / `AbsoluteTop` / `AbsoluteRight` / `AbsoluteBottom`

| C# | Rust | 状态 |
|----|------|------|
| `AbsoluteLeft { get => Left + Entity.Position.X; }` | `e.position.x + e.hitbox_offset.x` | ✅ 完全对齐 |

- **差异**: C# 中 `Absolute*` 加上实体的世界位置；Rust 的 `Entity` 没有 Collider 分离，`position` 已经是世界坐标，`hitbox_offset` 就是"Local Position"，二者相加即绝对坐标。语义完全对齐。

### 6. `CenterX` / `CenterY` (继承自 Collider)

| C# | Rust | 状态 |
|----|------|------|
| `CenterX { get => Left + Width / 2f; }` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。需要时可计算 `e.position.x + e.hitbox_offset.x + e.hitbox.x / 2.0`。目前无代码使用。

### 7. `Size` / `HalfSize` (继承自 Collider)

| C# | Rust | 状态 |
|----|------|------|
| `Vector2 Size => new Vector2(Width, Height);` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。`Entity.hitbox` 本身就是 Size。

### 8. `TopLeft` / `TopRight` / `BottomLeft` / `BottomRight` / `Center` / 等组合属性

| C# | Rust | 状态 |
|----|------|------|
| 各种 `Vector2` 组合 getter/setter | 无 | 🔴 缺失 |

- **影响**: 低。这些是便利属性，实际游戏逻辑很少直接使用。

### 9. `Bounds` (Rectangle)

| C# | Rust | 状态 |
|----|------|------|
| `Rectangle Bounds => new Rectangle((int)AbsoluteLeft, (int)AbsoluteTop, (int)Width, (int)Height);` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。Grid 的 `collide_rect` 直接接收 `(x, y, w, h)` 参数。

---

## 二、构造函数

### 10. `Hitbox(float width, float height, float x = 0f, float y = 0f)`

| C# | Rust | 状态 |
|----|------|------|
| `new Hitbox(width, height, x, y)` | `Entity { hitbox: Vec2::new(w, h), hitbox_offset: Vec2::new(x, y), .. }` | ✅ 完全对齐 |

- **差异**: C# 通过构造函数设置；Rust 通过直接字段赋值。语义一致：`hitbox` = 尺寸，`hitbox_offset` = 相对偏移。
- 默认值一致：`hitbox` 默认 `(8, 11)`（`ecs.rs:63`），`hitbox_offset` 默认 `(0, 0)`（`ecs.rs:64`）。

---

## 三、碰撞检测方法

### 11. `Intersects(Hitbox hitbox)` — 矩形-矩形相交

| C# | Rust | 状态 |
|----|------|------|
| AABB overlap: `AbsoluteLeft < hitbox.AbsoluteRight && ...` | `SolidGrid::solid_entity_hitbox` 内部: `x < sx+sw && x+w > sx && y < sy+sh && y+h > sy` | ✅ 完全对齐 |

- **差异**: C# 是实例方法 `this.Intersects(other)`；Rust 是 `SolidGrid` 的私有方法 `solid_entity_hitbox`，参数化为 `(world, exclude, x, y, w, h)`，遍历所有 solid entity 寻找重叠。
- 算法完全一致：标准 AABB 重叠测试。

### 12. `Intersects(float x, float y, float width, float height)` — 矩形与原始参数相交

| C# | Rust | 状态 |
|----|------|------|
| `AbsoluteRight > x && AbsoluteBottom > y && AbsoluteLeft < x+w && AbsoluteTop < y+h` | 同上 `solid_entity_hitbox` 中的 AABB 测试 | ✅ 完全对齐 |

### 13. `Collide(Vector2 point)` — 点-矩形碰撞

| C# | Rust | 状态 |
|----|------|------|
| `Monocle.Collide.RectToPoint(AbsoluteLeft, AbsoluteTop, Width, Height, point)` | `SolidGrid::collide_rect` 隐式包含（点碰撞是 rect 碰撞的特例） | 🟡 近似 |

- **差异**: C# 有专门的 `Collide.Point` 方法调用 `Collide.RectToPoint`（精确的点在 AABB 内判断）。Rust 没有独立的"点碰撞" API；`collide_rect` 用于 grid-based 碰撞，不直接用于"点是否在 hitbox 内"。
- **影响**: 中等。某些实体可能需要点碰撞查询（如检测鼠标点击、投射物命中）。目前未发现直接使用场景。

### 14. `Collide(Rectangle rect)` — 矩形-矩形碰撞（使用 XNA Rectangle）

| C# | Rust | 状态 |
|----|------|------|
| AABB overlap with `rect.Left/Right/Top/Bottom` | `SolidGrid::collide_rect(x, y, w, h)` | ✅ 完全对齐 |

- **差异**: C# 使用 XNA `Rectangle` 类型（整数）；Rust 使用 `(f32, f32, f32, f32)`。算法完全一致。

### 15. `Collide(Vector2 from, Vector2 to)` — 线段-矩形碰撞

| C# | Rust | 状态 |
|----|------|------|
| `Monocle.Collide.RectToLine(AbsoluteLeft, AbsoluteTop, Width, Height, from, to)` | `SolidGrid::line_of_sight(x1, y1, x2, y2)` | 🟠 部分实现 |

- **差异**:
  - C# 的 `RectToLine` 是精确的线段-AABB 相交测试（使用 sectors + `LineCheck` 交叉积算法）。
  - Rust 的 `line_of_sight` 是 **grid-stepped 采样**（沿线段每半个 tile 采样一个点，检查是否命中 solid tile），不是精确的线段-AABB 测试。
  - `line_of_sight` 用于 `Seeker.CanSeePlayer` 等 AI 视线，不是通用碰撞方法。
- **缺失**: 通用的线段-矩形精确碰撞（`RectToLine`）未实现。

### 16. `Collide(Hitbox hitbox)` — Hitbox-Hitbox 碰撞

| C# | Rust | 状态 |
|----|------|------|
| `return Intersects(hitbox);` | `solid_entity_hitbox` 中的 AABB 测试 | ✅ 完全对齐 |

- 本质就是 `Intersects` 的代理调用。

### 17. `Collide(Grid grid)` — Hitbox 与 Grid 碰撞

| C# | Rust | 状态 |
|----|------|------|
| `grid.Collide(base.Bounds)` — 将 hitbox 的绝对边界交给 Grid 查询 | `SolidGrid::collide_rect(x, y, w, h)` | ✅ 完全对齐 |

- **差异**: C# 是 Hitbox 方法调用 Grid；Rust 是 `SolidGrid` 自身的 `collide_rect` 方法，传入矩形区域查询。语义一致。

### 18. `Collide(Circle circle)` — Hitbox 与圆形碰撞

| C# | Rust | 状态 |
|----|------|------|
| `Monocle.Collide.RectToCircle(AbsoluteLeft, AbsoluteTop, Width, Height, circle.AbsolutePosition, circle.Radius)` | `SolidGrid::circle_to_rect(cx, cy, r, x, y, w, h)` | 🟡 近似 |

- **差异**:
  - C# 的 `RectToCircle` 使用 sector-based 精确算法（先检查圆心是否在 rect 内，再按 sector 检测各边的最近距离）。
  - Rust 的 `circle_to_rect` 使用 **closest-point-on-AABB** 算法（将圆心 clamp 到 AABB 上，检测距离）。这是等价且更简洁的实现。
- **结论**: 功能完全对齐，算法更优。

### 19. `Collide(ColliderList list)` — 与碰撞列表碰撞

| C# | Rust | 状态 |
|----|------|------|
| `list.Collide(this)` — 遍历 ColliderList 内所有 collider | 无 | 🔴 缺失 |

- **影响**: 中等。C# 的 `ColliderList` 允许一个实体有多个碰撞箱（如玩家的头部和身体分别碰撞）。Rust 没有多碰撞箱概念。
- **当前需求**: 暂未发现需要多碰撞箱的场景。如果未来需要，可考虑在 ECS 中添加多 hitbox 支持。

---

## 四、其他方法

### 20. `Clone()` — 克隆碰撞箱

| C# | Rust | 状态 |
|----|------|------|
| `return new Hitbox(width, height, Position.X, Position.Y);` | `Entity` derives `Clone` | ✅ 完全对齐 |

### 21. `Render(Camera camera, Color color)` — 渲染碰撞箱

| C# | Rust | 状态 |
|----|------|------|
| `Draw.HollowRect(AbsoluteX, AbsoluteY, Width, Height, color)` | `renderer.rs:draw_hitboxes`: `renderer.draw_rect(hx, hy, e.hitbox.x, e.hitbox.y, color, true)` | ✅ 完全对齐 |

- **差异**: Rust 在 `Renderer::draw_hitboxes` 中统一渲染所有实体碰撞箱，支持 `--show-hitboxes` CLI 参数。颜色逻辑：
  - 红色 = 普通实体
  - 黄色 = solid_platform
  - 绿色 = solid_entity
- C# 默认红色，`Entity.Render` 在 `Collidable` 为 false 时用 DarkRed。颜色策略略有不同但功能对齐。

### 22. `SetFromRectangle(Rectangle rect)` — 从 Rectangle 设置

| C# | Rust | 状态 |
|----|------|------|
| `Position = new Vector2(rect.X, rect.Y); Width = rect.Width; Height = rect.Height;` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。可以直接赋值 `e.position`, `e.hitbox_offset`, `e.hitbox`。目前无调用点。

### 23. `Set(float x, float y, float w, float h)` — 批量设置

| C# | Rust | 状态 |
|----|------|------|
| `Position = new Vector2(x, y); Width = w; Height = h;` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。同上，直接赋值等价。

### 24. `GetTopEdge(out Vector2 from, out Vector2 to)` — 获取上边线段

| C# | Rust | 状态 |
|----|------|------|
| `from = (AbsoluteLeft, AbsoluteTop); to = (AbsoluteRight, AbsoluteTop);` | 无 | 🔴 缺失 |

- **影响**: 低。这些 edge 方法用于 slope/platform 碰撞计算。需要时可手动计算。

### 25. `GetBottomEdge` / `GetLeftEdge` / `GetRightEdge`

| C# | Rust | 状态 |
|----|------|------|
| 各边的 `(from, to)` 线段输出 | 无 | 🔴 缺失 |

- **影响**: 低。同上。

---

## 五、Collider 基类成员（继承到 Hitbox）

### 26. `Entity` 引用

| C# | Rust | 状态 |
|----|------|------|
| `Entity Entity { get; private set; }` — 碰撞箱所属实体的引用 | 反向关系：`Entity` 持有碰撞数据，`SolidGrid` 通过 `world.get(id)` 获取 | ✅ 完全对齐（架构反转） |

- **差异**: C# 是碰撞箱 → 实体的引用；Rust 是实体 → 碰撞数据，`SolidGrid` 通过 `World` 查询实体。架构反转但功能等价。

### 27. `Added(Entity entity)` / `Removed()` — 生命周期

| C# | Rust | 状态 |
|----|------|------|
| 碰撞箱注册/注销到实体 | ECS `World::spawn()` / 实体销毁时自动管理 | ✅ 完全对齐 |

### 28. `CenterOrigin()` — 居中原点

| C# | Rust | 状态 |
|----|------|------|
| `Position.X = -Width / 2f; Position.Y = -Height / 2f;` | 无直接等价 | 🔴 缺失 |

- **影响**: 低。需要时可直接设置 `e.hitbox_offset = Vec2::new(-e.hitbox.x / 2.0, -e.hitbox.y / 2.0)`。

### 29. `Collide(Entity entity)` — 与另一个实体碰撞

| C# | Rust | 状态 |
|----|------|------|
| `Collide(entity.Collider)` — 多态分发 | `SolidGrid::entity_collide(world, id, dx, dy)` | 🟡 近似 |

- **差异**:
  - C# 是通用多态碰撞：遍历所有实体的 Collider 类型分发。
  - Rust 的 `entity_collide` 只检查 solid entities（全实体块），不检查所有实体类型。
  - C# 的 `Collide.Check(a, b)` 检查任意两个实体（只要都有 Collider）。

---

## 六、`Collide` 静态类中 Hitbox 使用的辅助方法

### 30. `Collide.RectToPoint(float rX, float rY, float rW, float rH, Vector2 point)`

| C# | Rust | 状态 |
|----|------|------|
| 精确点在 AABB 内判断 | 无独立 API | 🔴 缺失 |

### 31. `Collide.RectToLine(...)` — 精确线段-AABB 测试

| C# | Rust | 状态 |
|----|------|------|
| sector-based 精确线段-AABB 测试 | `SolidGrid::line_of_sight`（grid-stepped 近似） | 🟠 部分实现 |

### 32. `Collide.RectToCircle(...)` — 精确圆-AABB 测试

| C# | Rust | 状态 |
|----|------|------|
| sector-based 圆-AABB 测试 | `SolidGrid::circle_to_rect`（closest-point 算法） | ✅ 完全对齐 |

---

## 汇总统计

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 13 | 43% |
| 🟠 部分实现 | 3 | 10% |
| 🟡 近似 | 3 | 10% |
| 🔴 缺失 | 11 | 37% |
| **合计** | **30** | 100% |

### 缺失项影响评估

| 缺失项 | 影响等级 | 说明 |
|--------|----------|------|
| `CenterX/Y`, `Size`, `HalfSize` | 🟢 低 | 便利属性，可随时计算 |
| `TopLeft/TopRight/BottomLeft/BottomRight/Center` | 🟢 低 | 组合属性，极少使用 |
| `Bounds` (Rectangle) | 🟢 低 | 可随时构造 |
| `Set()` / `SetFromRectangle()` | 🟢 低 | 直接赋值等价 |
| `GetXxxEdge()` (4 个) | 🟡 中 | Slope/平台边缘检测可能需要 |
| `CenterOrigin()` | 🟢 低 | 一行代码可替代 |
| `Collide(ColliderList)` | 🟡 中 | 多碰撞箱场景暂无需求 |
| `Collide(Point)` 独立 API | 🟡 中 | 投射物/鼠标交互可能需要 |
| `RectToPoint` / `RectToLine` 精确版 | 🟢 低 | `circle_to_rect` 已用更优算法；`line_of_sight` 覆盖主要场景 |

### 架构差异总结

1. **数据模型反转**: C# 的 `Collider` 是独立对象挂在 `Entity` 上；Rust 将碰撞数据内联到 `Entity`，碰撞逻辑由 `SolidGrid` 承担。
2. **多态消失**: C# 的 `Collide(Vector2/Rectangle/Hitbox/Grid/Circle/ColliderList)` 多态分发被 Rust 的独立函数替代（`collide_rect`、`circle_to_rect` 等）。
3. **Grid 职责合并**: C# 的 `Grid` collider + `Actor.MoveH/MoveV` 在 Rust 中合并为 `SolidGrid`，包含碰撞查询和物理移动全部逻辑。
4. **实体碰撞**: Rust 的 `entity_collide` 只检查 solid entities（全实体块），不支持通用实体-实体碰撞（C# 的 `Collide.Check(a, b)` 可检查任意两实体）。
