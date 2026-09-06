# Grid.cs vs physics.rs 对比分析

> 源文件:
> - C#: `references/source/Celeste/Monocle/Grid.cs` — Monocle 引擎的 Grid 碰撞器
> - Rust: `src/engine/physics.rs` — Ruleste 的 `SolidGrid` 碰撞 + Actor 移动系统

---

## 概览

| 维度 | C# Grid | Rust SolidGrid |
|------|---------|----------------|
| 角色 | 纯碰撞数据结构（继承 `Collider`） | 碰撞数据 + Actor 移动逻辑（`actor_move`） |
| 存储 | `VirtualMap<bool>`（稀疏/虚拟映射） | `Vec<bool>` + `Vec<Option<char>>`（密集平铺） |
| 单元格尺寸 | 可配置（`CellWidth` / `CellHeight`） | 固定 `TILE = 8.0` 常量 |
| 世界坐标 | 通过 `Position`（继承自 `Collider`） | 通过 `origin_x` / `origin_y` |
| 扩展能力 | 仅碰撞查询 | 额外包含 jump-thru 平台、动态实体碰撞、平台骑乘 |

---

## 逐方法对比

### 数据结构与属性

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 1 | `Data` (`VirtualMap<bool>`) | `solid: Vec<bool>` | 🟠 部分实现 | C# 用 `VirtualMap`（稀疏，支持任意索引），Rust 用密集 `Vec<bool>`。Rust 额外维护 `tile_ids: Vec<Option<char>>` 存储地块字符标识。 |
| 2 | `CellWidth` (float) | `TILE` (f32, 固定 8.0) | 🟡 近似 | C# 允许非正方形单元格（如 6×6），Rust 硬编码为 8×8。对 Celeste 原版足够，但损失通用性。 |
| 3 | `CellHeight` (float) | `TILE` (f32, 固定 8.0) | 🟡 近似 | 同上。 |
| 4 | `CellsX` (int) | `pub width: usize` | ✅ 完全对齐 | 直接对应。 |
| 5 | `CellsY` (int) | `pub height: usize` | ✅ 完全对齐 | 直接对应。 |
| 6 | `Width` (float, override) | `size().0 * TILE` | 🟡 近似 | C# 是可计算属性 `CellWidth * CellsX`；Rust 需手动用 `width * TILE`，无对应 getter。 |
| 7 | `Height` (float, override) | `size().1 * TILE` | 🟡 近似 | 同上。 |
| 8 | `IsEmpty` | — | 🔴 缺失 | 遍历所有单元格检查是否全为 false。Rust 未实现，需要时可用 `solid.iter().all(|&b| !b)` 替代。 |
| 9 | `Left` (Position.X) | `origin_x` | 🟡 近似 | C# 左边界 = `Position.X`；Rust 左边界 = `origin_x`。含义相同，Rust 无 setter。 |
| 10 | `Top` (Position.Y) | `origin_y` | 🟡 近似 | 同上。 |
| 11 | `Right` (Position.X + Width) | `origin_x + width * TILE` | 🟡 近似 | 无对应属性，需计算。 |
| 12 | `Bottom` (Position.Y + Height) | `origin_y + height * TILE` | 🟡 近似 | 无对应属性，需计算。 |

### 索引器 / 按坐标查询

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 13 | `this[int x, int y]` (get) | `solid_at(tx, ty)` | 🟠 部分实现 | C# 直接索引本地坐标（越界返回 false）。Rust 先将世界坐标 `(tx, ty)` 减去 `origin` 转为本地索引，越界返回 `true`（视为固体）——行为相反！这是有意设计：关卡外区域被当作墙。 |
| 14 | `this[int x, int y]` (set) | — | 🔴 缺失 | `solid_at` 只读。设置需直接操作 `solid[idx]`，无公开 API。 |

### 构造函数

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 15 | `Grid(cellsX, cellsY, cellW, cellH)` | `SolidGrid::empty(w, h)` | 🟡 近似 | 功能一致（空网格），Rust 不接受单元格尺寸参数（固定 TILE）。 |
| 16 | `Grid(cellW, cellH, bitstring)` | `SolidGrid::from_rows(rows)` | 🟠 部分实现 | C# 接受 `"101\n010"` 格式 bitstring，用 `LoadBitstring` 解析；Rust 接受 `&[&str]`（每行一个字符串），每行内非 `'0'` 字符均视为实体。Rust 更灵活（保留 tile_id），但 C# 的 bitstring 接口不可用。 |
| 17 | `Grid(cellW, cellH, bool[,])` | — | 🔴 缺失 | C# 直接从二维 bool 数组构造。Rust 无此构造函数，需手动填充。 |
| 18 | `Grid(cellW, cellH, VirtualMap<bool>)` | — | 🔴 缺失 | C# 直接包装已有数据映射。Rust 无此构造函数。 |

### 数据操作方法

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 19 | `Extend(left, right, up, down)` | `blit(src, dx, dy)` | 🟡 近似 | 两者都用于扩展/复合网格，但 API 不同：C# 就地扩展当前网格并填充边界（edge-clamp）；Rust 是将 `src` 覆盖到 `self` 的指定偏移。C# 的边界填充（edge-clamp 逻辑）在 Rust 中不存在。 |
| 20 | `LoadBitstring(string)` | `from_rows(rows)` (仅构造时) | 🟠 部分实现 | C# 支持构造后重新加载 bitstring；Rust 只能在构造时设置，无后期重载方法。 |
| 21 | `GetBitstring()` | — | 🔴 缺失 | Rust 无法将网格序列化为 bitstring 字符串。 |
| 22 | `Clear(bool to = false)` | — | 🔴 缺失 | Rust 无清空方法。需要时可用 `solid.fill(to)`。 |
| 23 | `SetRect(x, y, w, h, to = true)` | — | 🔴 缺失 | Rust 无矩形区域设置方法。 |
| 24 | `CheckRect(x, y, w, h)` | `collide_rect(x, y, w, h)` | 🟡 近似 | C# 检查矩形内是否有任意 true 单元格；Rust 检查 AABB 是否与实体地块重叠。两者概念相同，但 C# 用本地像素坐标除以 CellWidth 转 tile，Rust 用世界坐标除以 TILE。Rust 特别处理了边界 epsilon 问题。 |
| 25 | `CheckColumn(x)` | — | 🔴 缺失 | C# 检查整列是否全为 true。Rust 无此方法。 |
| 26 | `CheckRow(y)` | — | 🔴 缺失 | C# 检查整行是否全为 true。Rust 无此方法。 |
| 27 | `Clone()` | — | 🔴 缺失 | C# 深拷贝整个网格。Rust `SolidGrid` 已 derive `Clone`，可通过 `.clone()` 实现。 |

### 渲染

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 28 | `Render(Camera, Color)` | — | 🔴 缺失 | C# 渲染所有 solid 单元格的空心矩形（含相机视口裁剪）。Rust 将渲染分离到 `Renderer`，physics 模块不含绘制逻辑。不一定是缺失——可能是架构分离。 |

### 碰撞查询

| # | C# 方法/属性 | Rust 对应 | 状态 | 差异说明 |
|---|-------------|-----------|------|---------|
| 29 | `Collide(Vector2 point)` | — | 🔴 缺失 | 点是否在 solid 单元格内。Rust 无对应方法。可通过 `solid_at` 手动实现。 |
| 30 | `Collide(Rectangle rect)` | `collide_rect(x, y, w, h)` | ✅ 完全对齐 | 功能等价：检查 AABB 是否与任何 solid 地块重叠。Rust 额外处理了 epsilon 精度问题。 |
| 31 | `Collide(Vector2 from, Vector2 to)` | `line_of_sight(x1, y1, x2, y2)` | 🟡 近似 | C# 用 Bresenham 风格逐格扫描（转换为单元格坐标后沿主轴步进）。Rust 用等距采样步进（步长 = 半 tile），非 Bresenham。两者都检测线段是否穿过 solid，但算法不同，精度特征不同。 |
| 32 | `Collide(Hitbox hitbox)` | `collide_rect` (via hitbox bounds) | 🟠 部分实现 | C# 直接委托给 `Collide(Rectangle)`。Rust 无 Hitbox 类型，用户需从 Entity 提取 hitbox 参数后调用 `collide_rect`。 |
| 33 | `Collide(Grid grid)` | — | 🔴 缺失 | C# 本身也未实现（抛 `NotImplementedException`）。 |
| 34 | `Collide(Circle circle)` | `collide_circle(cx, cy, r)` | ✅ 完全对齐 | 功能等价。Rust 实现了包围盒预过滤 + 最近点距离检查。C# 返回 false（原始实现是空操作）。 |
| 35 | `Collide(ColliderList list)` | — | 🔴 缺失 | C# 委托给 `ColliderList.Collide(this)`，是 Collider 接口的多态分发。Rust 无 ColliderList 概念。 |
| 36 | `IsBitstringEmpty(string)` | — | 🔴 缺失 | 静态方法，检查 bitstring 是否不含 '1'。Rust 无对应。 |

### Rust 独有方法（C# 无对应）

| # | Rust 方法 | 说明 |
|---|-----------|------|
| A | `add_jumpthru(JumpThru)` | 注册单向平台（C# 中 JumpThru 是单独的 Collider） |
| B | `jumpthru_top(x, y, w)` | 检测水平线段是否在 jump-thru 顶部 |
| C | `platform_top/world(id)` | 动态实体平台的顶部检测 |
| D | `platform_landing(...)` | 下落时检测是否落在动态平台上 |
| E | `mark_solid_platform(world, id, on)` | 标记实体为可站立平台 |
| F | `mark_solid_entity(world, id, on)` | 标记实体为完全实体块 |
| G | `solid_entity_hitbox(...)` | 检测与完全实体块的 AABB 重叠 |
| H | `platform_riders(world, platform_id)` | 查找站在平台上的骑乘实体 |
| I | `shift(world, id, dx, dy)` | 平移实体位置 |
| J | `jumpthru_landing(...)` | 下落时检测 jump-thru 着陆 |
| K | `solid_at(tx, ty)` | 按世界坐标查询地块 solid 状态 |
| L | `tile_id_at(tx, ty)` | 按世界坐标查询地块字符 ID |
| M | `tile_id_at_local(tx, ty)` | 按本地坐标查询地块字符 ID |
| N | `size()` | 返回 `(width, height)` |
| O | `collide_circle(cx, cy, r)` | 圆形碰撞检测 |
| P | `circle_to_rect(...)` | 圆与 AABB 碰撞检测 |
| Q | `circle_to_circle(...)` | 圆与圆碰撞检测 |
| R | `actor_move(world, id, dx, dy)` | 完整的 Actor 移动 + 碰撞解析（含跳板/骑乘） |
| S | `is_grounded(world, id)` | 判断实体是否着地 |
| T | `entity_collide(world, id, dx, dy)` | 判断实体偏移后是否与 solid 重叠 |

---

## 统计

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 4 | 11% |
| 🟠 部分实现 | 5 | 14% |
| 🟡 近似 | 13 | 36% |
| 🔴 缺失 | 14 | 39% |
| **C# 方法总数** | **36** | — |
| **Rust 独有方法** | **20** | — |

---

## 关键设计差异

### 1. Tile 尺寸硬编码
C# 的 `Grid` 允许非均匀单元格（如 6×12），适用于各种非标准碰撞场景。Rust 固定为 `TILE = 8.0`，牺牲通用性换取简洁——对 Celeste 的所有网格碰撞足够，因为原版也只用 8×8 tile。

### 2. 越界语义相反
C# 索引器越界返回 `false`（安全，不碰撞）。Rust `solid_at` 越界返回 `true`（关卡外视为实体墙）。这是有意设计，避免实体飞出关卡边界。

### 3. 存储布局
C# `VirtualMap<bool>` 支持稀疏存储（只存 true 的格子），适合大面积空网格。Rust 用密集 `Vec<bool>`（`width * height` 个元素），对 8×8 tile 的关卡内存可忽略（~200×135 = 27000 bool ≈ 27KB）。

### 4. 功能扩展方向不同
- C# `Grid` 是纯碰撞数据 + 查询，移动逻辑在 `Actor.MoveH/MoveV` 中。
- Rust `SolidGrid` 将碰撞数据、Actor 移动（`actor_move`）、jump-thru 平台、动态实体碰撞、骑乘系统全部整合。这意味着 Rust 端的 physics 模块职责远大于 C# 的 Grid 类。

### 5. tile_ids 额外能力
Rust 维护 `tile_ids: Vec<Option<char>>`，存储每个地块的字符标识（如 `'1'`, `'K'`, `'F'` 等），用于自动拼接（autotiler）和游戏逻辑判断。C# 的 Grid 只存 bool，tile 类型信息在别处管理。

### 6. 渲染分离
C# `Grid.Render` 直接绘制碰撞可视化（`Draw.HollowRect`）。Rust 将渲染完全分离到 `Renderer` 模块，physics 模块保持纯逻辑——更干净的关注点分离。

---

## 缺失方法优先级建议

| 优先级 | 缺失方法 | 原因 |
|--------|---------|------|
| 高 | `Clear(bool)` | 构造后重置网格，地图热重载时需要 |
| 中 | `SetRect(x, y, w, h, to)` | 动态修改网格（如关卡脚本开启/关闭区域） |
| 中 | `Collide(Vector2 point)` | 点碰撞查询，粒子效果和触发器常用 |
| 中 | `Clone()` | derive 已满足，确保深拷贝语义正确 |
| 低 | `IsEmpty` | 极少使用 |
| 低 | `CheckColumn` / `CheckRow` | 特定关卡效果使用 |
| 低 | `GetBitstring` / `IsBitstringEmpty` | 序列化/调试用途 |
| 低 | `Collide(Grid)` / `Collide(ColliderList)` | C# 原版也未实现/仅委托 |
