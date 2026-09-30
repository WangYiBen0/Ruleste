# Autotiler 对比分析：C# 原版 vs Rust 实现

> 生成时间：2026-09-13
> 源文件：`references/source/Celeste/Celeste/Autotiler.cs`（419 行） vs `crates/ruleste-core/src/engine/autotiler.rs`（约 480 行）

## 一、数据结构对比

### 1.1 `TerrainType` / `TilesetDef`

| 成员 | C# | Rust | 状态 |
|------|-----|------|------|
| `ID` | `id: char` | ✅ |
| `Ignores` | `ignores: HashSet<char>` | ✅ |
| `Masked` | `masks: Vec<MaskEntry>` | 🟠 结构不同 |
| `Center`（Tiles） | `center: Vec<(u32,u32)>` | 🟠 仅存坐标 |
| `Padded`（Tiles） | `padding: Vec<(u32,u32)>` | 🟠 仅存坐标 |

差异：C# `Center/Padded` 是 `Tiles` 对象（含 `OverlapSprites` 动画覆盖层）；Rust 只存坐标，**丢失 OverlapSprites**（`AnimatedTiles` 覆盖层）。

### 1.2 `Masked` / `MaskEntry`

| 成员 | C# | Rust | 状态 |
|------|-----|------|------|
| `Mask[9]` | `mask: [u8;9]` | ✅ |
| `Tiles` | `tiles: Vec<(u32,u32)>` | 🟠 无 OverlapSprites |
| — | `wildcards`（排序优化） | ✅ 新增 |

### 1.3 `Tiles`

| 成员 | C# | Rust | 状态 |
|------|-----|------|------|
| `List<MTexture> Textures` | `Vec<(u32,u32)>`（tileset 坐标） | 🟠 |
| `List<string> OverlapSprites` | ❌ | 🔴 |
| `HasOverlays` | ❌ | 🔴 |

### 1.4 `Generated`

| C# | Rust | 状态 |
|----|------|------|
| `TileGrid TileGrid` | `TileGrid`（自研平铺存储） | 🟠 |
| `AnimatedTiles SpriteOverlay` | ❌ | 🔴 |

### 1.5 `Behaviour`

| C# | Rust | 状态 |
|----|------|------|
| `PaddingIgnoreOutOfLevel` | 硬编码 true | 🟡 |
| `EdgesIgnoreOutOfLevel` | 缺失 | 🔴 |
| `EdgesExtend` | 硬编码 true | 🟡 |

### 1.6 公共字段

`LevelBounds: List<Rectangle>` → ❌ 缺失（跨关卡边界检查 `CheckForSameLevel` 不存在）。

### 1.7 私有字段

`lookup` ↔ `tilesets: HashMap<char,TilesetDef>` ✅；`adjacent` ↔ `adjacency()` 局部返回 ✅。

## 二、逐方法对比

### `ReadInto`（读取 XML）
C# 用 `Tileset` + `SetMask`；Rust `Autotiler::parse` 解析 `<Tileset id path copy ignores>`、`<set mask tiles/>`、`padding`、`center`。✅ 基本对齐（`copy` 继承同 `SpriteBank`）。

### `GenerateMap(VirtualMap<char>, bool)` / `GenerateMap(...,Behaviour)`
Rust **没有直接 `GenerateMap` 方法名**，而是 `generate(&SolidGrid) -> TileGrid`，语义等价：对整张实心网格跑 3×3 邻域匹配 + 中心/边沿/隔离选块。50-tile 分段优化没有（直接全遍历），`EdgesIgnoreOutOfLevel`/`LevelBounds` 分支缺失，`Behaviour` 硬编码。

| 行为 | C# | Rust | 状态 |
|------|-----|------|------|
| 邻域中心恒 1 | ✅ | ✅ | ✅ |
| EdgesExtend（越界 clamp 取边格子） | ✅ | ✅ `neighbor_connects` clamp | ✅ |
| Padding via 2-away | ✅ | ✅ | ✅ |
| `PaddingIgnoreOutOfLevel` 时越界按空 | ✅ | Rust 越界直接不连（同忽略语义） | 🟠 |
| OverlapSprites/AnimatedTiles | ✅ | ❌ | 🔴 |
| 随机变体 | `Calc.Random.Choose` | 确定性 hasher（位置 hash） | 🟠 种子不同 |
| `CheckForSameLevel` | ✅ | ❌ | 🔴 |

### `GenerateBox(char id, int tilesX, int tilesY)`
Rust `generate_box(tile_id, tiles_x, tiles_y)` ✅ 对齐：构造全 `row` 的 `SolidGrid` 再 `generate`。插件 `introCrusher` 等已使用，宿主有 `tile_box_cache`。

### `GenerateOverlay(char id, x, y, tilesX, tilesY, mapData)`
❌ 缺失（无 AnimatedTiles 覆盖层系统，插件无对应 FFI）。

### `TileHandler`/`CheckTile`/`GetTile`/`CheckForSameLevel`/`IsEmpty`
合并进 Rust `adjacency`/`neighbor_connects`/`connects_id`/`mask_matches`/`generate`。除 `LevelBounds`/`Behaviour` 外逻辑一致（详见上表）。

## 三、差异根因

1. OverlapSprites（动画叠加层，如草/雪在拼块上的可动画饰物）依赖 `AnimatedTilesBank`，Rust 无 2D 动画瓦片渲染通道。
2. `LevelBounds`（跨房间边界保持边缘延伸）在此仓库由**整关单一 SolidGrid 合成**替代（`level.rs`），所以单关内恒同 level，`CheckForSameLevel` 恒 true，行为不变；只有多 Level 拼接场景才真的需要它。
3. 50-tile 分段是纯性能优化，Rust 当前关卡规模不需要。
4. 随机变体用位置 hash，导致与原版不同的视觉变体分布，但“随机选一个变体”语义一致。

## 四、缺失 🔴（更新）

- **`GenerateOverlay()` / `AnimatedTiles SpriteOverlay` / `OverlapSprites` / `HasOverlays`** 仍然全缺
- **`Behaviour` 完整结构**（EdgesIgnoreOutOfLevel、可配 PaddingIgnoreOutOfLevel）仍缺
- **`LevelBounds`** 仍缺（单关合成下无影响）
- **50-tile 分段**：无（性能优化，不影响正确性）

## 五、Rust 独有

- `TileGrid.tile_at()` 便捷查询
- `tile_id_at_local` / `SolidGrid` 像素/局部网格双向
- 确定性 hash 随机（便于回归测试）
- `generate_box` 被宿主缓存复用（`tile_box_cache`）
