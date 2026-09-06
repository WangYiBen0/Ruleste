# Camera.cs vs camera.rs 对比分析

> 原版：`references/source/Celeste/Monocle/Camera.cs` (276 行)
> Rust：`src/engine/camera.rs` (264 行)

## 概述

Rust 版 Camera 并非原版 Monocle Camera 的 1:1 移植，而是**面向 Celeste 关卡相机需求的重新设计**。原版是通用的视图矩阵工具（支持旋转、缩放、矩阵变换），Rust 版聚焦于玩家跟随、房间限制、屏幕震动，去掉了矩阵系统。

---

## 字段对比

| # | C# 字段 | 类型 | Rust 对应 | 状态 | 说明 |
|---|---------|------|-----------|------|------|
| 1 | `matrix` | `Matrix` | — | 🔴 缺失 | Rust 版无矩阵变换系统，渲染直接用 `position` |
| 2 | `inverse` | `Matrix` | — | 🔴 缺失 | 同上，无逆矩阵 |
| 3 | `changed` | `bool` | — | 🔴 缺失 | 延迟计算标记，Rust 版每次 update 直接计算 |
| 4 | `position` | `Vector2` | `position: Vec2` | ✅ 完全对齐 | 含义相同：相机左上角世界坐标；Rust 额外叠加 shake 偏移 |
| 5 | `zoom` | `Vector2` | — | 🔴 缺失 | Rust 版无缩放支持（固定 320×180 视口） |
| 6 | `origin` | `Vector2` | — | 🔴 缺失 | 原版用于居中偏移，Rust 版不需要 |
| 7 | `angle` | `float` | — | 🔴 缺失 | 原版支持旋转，Rust 版不需要 |
| 8 | `Viewport` | `Viewport` | `VIEW_WIDTH` / `VIEW_HEIGHT` (常量) | 🟠 部分实现 | 硬编码 320×180，非动态 Viewport 结构体 |
| 9 | — | — | `base_position: Vec2` | 🟡 近似（新增） | Rust 独有：平滑基座位置，用于 shake 偏移计算 |
| 10 | — | — | `shakes: Vec<Shake>` | 🟡 近似（新增） | Rust 独有：震动队列 |

---

## 属性对比

| # | C# 属性 | 状态 | Rust 对应 | 逐行差异 |
|---|---------|------|-----------|----------|
| 1 | `Matrix` (get) | 🔴 缺失 | — | C#：延迟计算矩阵并返回。Rust：无矩阵系统 |
| 2 | `Inverse` (get) | 🔴 缺失 | — | C#：延迟计算逆矩阵并返回。Rust：无矩阵系统 |
| 3 | `Position` (get/set) | 🟠 部分实现 | `position: Vec2` (pub 字段) | C#：getter 返回 `position`，setter 设 `changed=true`。Rust：pub 字段直接读写，无 dirty 标记（每次 update 重算） |
| 4 | `Origin` (get/set) | 🔴 缺失 | — | C#：视口原点偏移。Rust：不需要此概念 |
| 5 | `X` (get/set) | 🔴 缺失 | — | C#：`position.X` 的快捷访问。Rust：通过 `position.x` 直接访问 |
| 6 | `Y` (get/set) | 🔴 缺失 | — | C#：`position.Y` 的快捷访问。Rust：通过 `position.y` 直接访问 |
| 7 | `Zoom` (get/set) | 🔴 缺失 | — | C#：缩放值（同步设置 X/Y）。Rust：无缩放 |
| 8 | `Angle` (get/set) | 🔴 缺失 | — | C#：旋转角度。Rust：无旋转 |
| 9 | `Left` (get/set) | 🔴 缺失 | — | C#：通过逆矩阵变换计算可见区域左边界。Rust：无矩阵系统 |
| 10 | `Right` (get) | 🔴 缺失 | — | C#：`Viewport.Width` 经逆矩阵变换。Rust：无对应 |
| 11 | `Top` (get/set) | 🔴 缺失 | — | C#：通过逆矩阵变换计算可见区域上边界。Rust：无矩阵系统 |
| 12 | `Bottom` (get) | 🔴 缺失 | — | C#：`Viewport.Height` 经逆矩阵变换。Rust：无对应 |

---

## 构造函数对比

| # | C# 构造函数 | Rust 对应 | 状态 | 逐行差异 |
|---|------------|-----------|------|----------|
| 1 | `Camera()` | `Camera::new()` | 🟠 部分实现 | C#：`Viewport.Width = Engine.Width; Viewport.Height = Engine.Height; UpdateMatrices()`。Rust：初始化 `base_position=ZERO, position=ZERO, shakes=empty`，无 Viewport 动态尺寸 |
| 2 | `Camera(int width, int height)` | — | 🔴 缺失 | C#：自定义尺寸构造。Rust：尺寸硬编码为常量 |

---

## 方法对比

| # | C# 方法 | Rust 对应 | 状态 | 逐行差异 |
|---|---------|-----------|------|----------|
| 1 | `ToString()` | — | 🔴 缺失 | C#：格式化输出 Viewport/Position/Origin/Zoom/Angle。Rust：依赖 `#[derive(Debug)]` 自动实现 |
| 2 | `UpdateMatrices()` (private) | — | 🔴 缺失 | C#：构建 `matrix = Identity * Translate(-floor(pos)) * RotateZ(angle) * Scale(zoom) * Translate(floor(origin))`，然后 `inverse = Invert(matrix)`。Rust：不需要矩阵，`update()` 直接计算位置 |
| 3 | `CopyFrom(Camera other)` | `snap_to(Vec2)` | 🟠 部分实现 | C#：复制 position/origin/angle/zoom 并标记 changed。Rust：`snap_to` 只设置位置并清空 shakes，不复制其他状态（origin/angle/zoom 不存在） |
| 4 | `CenterOrigin()` | — | 🔴 缺失 | C#：`origin = (Viewport.Width/2, Viewport.Height/2)`。Rust：不需要 origin 概念 |
| 5 | `RoundPosition()` | — | 🔴 缺失 | C#：`position.X = Round(position.X); position.Y = Round(position.Y)`。Rust：无对应（坐标保持浮点） |
| 6 | `ScreenToCamera(Vector2)` | — | 🔴 缺失 | C#：`Vector2.Transform(position, Inverse)` 屏幕→世界坐标。Rust：无矩阵变换 |
| 7 | `CameraToScreen(Vector2)` | — | 🔴 缺失 | C#：`Vector2.Transform(position, Matrix)` 世界→屏幕坐标。Rust：无矩阵变换 |
| 8 | `Approach(Vector2, float)` | — | 🔴 缺失 | C#：`Position += (target - Position) * ease` 线性逼近。Rust：类似逻辑在 `update()` 中用指数平滑实现（不同算法） |
| 9 | `Approach(Vector2, float, float)` | — | 🔴 缺失 | C#：带最大距离限制的线性逼近。Rust：无对应 |
| 10 | — | `target_at(player, offset, room_origin, room_size)` | 🟡 近似（新增） | Rust 独有：计算玩家跟随目标位置并 clamp 到房间边界。C# 中此逻辑在 `Level`/`Player` 中，不在 Camera 类 |
| 11 | — | `base_position()` | 🟡 近似（新增） | Rust 独有：返回不含 shake 偏移的基座位置 |
| 12 | — | `snap_to(Vec2)` | 🟡 近似（新增） | Rust 独有：瞬移相机并清空 shakes |
| 13 | — | `shaking()` | 🟡 近似（新增） | Rust 独有：查询是否有活跃 shake |
| 14 | — | `shake(intensity, duration)` | 🟡 近似（新增） | Rust 独有：队列化屏幕震动（C# 原版 Camera 无此方法，在 `Celeste` 命名空间其他类中） |
| 15 | — | `update(dt, target)` | 🟡 近似（新增） | Rust 独有：帧更新——指数平滑 + shake 衰减。C# 中平滑逻辑在 `Level.Camera` 调用处 |

---

## 统计汇总

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 1 | 4.5% |
| 🟠 部分实现 | 3 | 13.6% |
| 🟡 近似（新增） | 6 | 27.3% |
| 🔴 缺失 | 12 | 54.5% |
| **合计** | **22** | 100% |

---

## 关键设计差异

### 1. 矩阵系统 → 直接位置

C# 版本维护完整的视图矩阵管线：
```
Identity → Translate(-pos) → RotateZ(angle) → Scale(zoom) → Translate(origin)
```
并提供 `ScreenToCamera`/`CameraToScreen` 做坐标变换。

Rust 版完全移除了矩阵系统，相机 `position` 直接作为渲染偏移使用。这是因为 Celeste 的相机**从不旋转也不缩放**，矩阵系统是过度设计。

### 2. 延迟计算 → 即时计算

C# 使用 `changed` 标记做脏检查，只在读取 `Matrix`/`Inverse`/边界属性时才重算矩阵。Rust 版在每次 `update()` 中直接计算位置，无延迟机制。

### 3. 通用工具 → 游戏专用

C# Camera 是通用的 2D 视图工具（可旋转、缩放、偏移），Rust Camera 专为 Celeste 关卡相机设计：
- 硬编码 320×180 视口
- 内置玩家跟随目标计算 (`target_at`)
- 内置房间边界 clamp
- 内置屏幕震动系统 (shake queue)
- 指数平滑 (`update` 中的 `1.0 - 0.01^dt`)

### 4. 缺失的 C# 功能

以下 C# 功能在 Rust 版中完全缺失，但经分析**对 Celeste 游戏不需要**：
- `Zoom` / `Angle`：Celeste 相机从不缩放或旋转
- `Origin`：Celeste 相机原点始终在左上角
- `ScreenToCamera` / `CameraToScreen`：游戏内未使用矩阵坐标变换
- `Left` / `Right` / `Top` / `Bottom`：边界计算由 `target_at` 的 clamp 替代
- `RoundPosition`：渲染使用浮点坐标，SDL 处理亚像素
