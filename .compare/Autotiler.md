# Autotiler 对比分析：C# 原版 vs Rust 实现

> 生成时间：2025-01-XX
> 源文件：`references/source/Celeste/Celeste/Autotiler.cs` (419 行) vs `src/engine/autotiler.rs` (482 行)

---

## 一、数据结构对比

### 1.1 `TerrainType` (C# 私有类) → `TilesetDef` (Rust 结构体)

| 成员 | C# | Rust | 状态 |
|------|-----|------|------|
| `ID` | `char ID` | `id: char` | ✅ 完全对齐 |
| `Ignores` | `HashSet<char>` | `ignores: HashSet<char>` | ✅ 完全对齐 |
| `Masked` | `List<Masked>` | `masks: Vec<MaskEntry>` | 🟠 结构不同 |
| `Center` | `Tiles Center` | `center: Vec<(u32, u32)>` | 🟠 仅存坐标 |
| `Padded` | `Tiles Padded` | `padding: Vec<(u32, u32)>` | 🟠 仅存坐标 |

差异说明：
- C# 的 `Center`/`Padded` 是 `Tiles` 对象，包含 `List<MTexture> Textures`、`List<string> OverlapSprites`、`bool HasOverlays`。
- Rust 仅保存 `Vec<(u32, u32)>` 坐标，**丢失了 OverlapSprites（动画覆盖层）支持**。

### 1.2 `Masked` (C# 私有类) → `MaskEntry` (Rust 结构体)

| 成员 | C# | Rust | 状态 |
|------|-----|------|------|
| `Mask` | `byte[9] Mask` | `mask: [u8; 9]` | ✅ 完全对齐 |
| `Tiles` | `Tiles` 对象 | `tiles: Vec<(u32, u32)>` | 🟠 仅存坐标 |
| (无) | — | `wildcards: u8` | ✅ 新增（排序优化） |

差异说明：
- Rust 新增 `wildcards` 字段，用于排序时避免重新计数，属于优化。
- C# `Masked.Tiles` 包含 `OverlapSprites`，Rust 不支持。

### 1.3 `Tiles` (C# 私有类)

| 成员 | Rust 对应 | 状态 |
|------|-----------|------|
| `List<MTexture> Textures` | `Vec<(u32, u32)>` (内嵌于 MaskEntry/TilesetDef) | 🟠 仅存坐标 |
| `List<string> OverlapSprites` | ❌ 无 | 🔴 缺失 |
| `bool HasOverlays` | ❌ 无 | 🔴 缺失 |

### 1.4 `Generated` (C# 公共结构体)

| 成员 | Rust 对应 | 状态 |
|------|-----------|------|
| `TileGrid TileGrid` | `TileGrid` 结构体 | 🟠 结构不同 |
| `AnimatedTiles SpriteOverlay` | ❌ 无 | 🔴 缺失 |

差异说明：
- Rust `TileGrid` 将 tileset 路径和坐标平铺存储为独立 `Vec`，C# 使用 `TileGrid` 二维数组存 `MTexture`。
- Rust `TileGrid` 新增 `origin_x`/`origin_y` 用于世界坐标定位。
- Rust `TileGrid` 新增 `tile_at()` 便捷方法。

### 1.5 `Behaviour` (C# 公共结构体)

| 成员 | 状态 |
|------|------|
| `PaddingIgnoreOutOfLevel` | 🟡 硬编码为 true |
| `EdgesIgnoreOutOfLevel` | 🔴 缺失 |
| `EdgesExtend` | 🟡 硬编码为 true |

差异说明：
- C# 的 `Behaviour` 允许调用者控制边沿行为，Rust 实现中这些行为被硬编码（`EdgesExtend = true` 且不检查越界）。

### 1.6 公共字段

| 字段 | C# | Rust | 状态 |
|------|-----|------|------|
| `LevelBounds` | `List<Rectangle>` | ❌ 无 | 🔴 缺失 |

### 1.7 私有字段

| 字段 | C# | Rust | 状态 |
|------|-----|------|------|
| `lookup` | `Dictionary<char, TerrainType>` | `tilesets: HashMap<char, TilesetDef>` | ✅ 完全对齐 |
| `adjacent` | `byte[9]` (实例字段) | `adjacency()` 返回 `[u8; 9]` | ✅ 对齐（Rust 用局部变量） |

---

## 二、构造函数 / XML 解析对比

### 2.1 `Autotiler(string filename)` (C#) → `Autotiler::load()` + `Autotiler::parse()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| 加载 XML | `Calc.LoadContentXML(filename)` | `std::fs::read_to_string` + `roxmltree::Document::parse` | ✅ 对齐 |
| 遍历 `<Tileset>` 节点 | `GetElementsByTagName("Tileset")` | `doc.descendants().filter(\|n\| n.has_tag_name("Tileset"))` | ✅ 对齐 |
| 读取 `id` 属性 | `item.AttrChar("id")` | `tileset_node.attribute("id")` | ✅ 对齐 |
| 读取 `path` 属性 | `item.Attr("path")` | `tileset_node.attribute("path")` | ✅ 对齐 |
| `copy` 继承 | `ReadInto(terrainType, tileset, dictionary[key])` | `masks.extend(parent.masks.iter().cloned())` + 手动复制 center/padding | 🟠 近似 |
| `ignores` 解析 | `item.Attr("ignores").Split(',')` | `tileset_node.attribute("ignores").unwrap_or("").split(',')` | ✅ 对齐 |
| 排序 Masked | `data.Masked.Sort(delegate ...)` 按 wildcard 数量 | `masks.sort_by_key(\|m\| m.wildcards)` | ✅ 对齐 |

差异说明：
- C# 的 `copy` 通过再次调用 `ReadInto` 从已解析的 `XmlElement` 继承，Rust 直接克隆 `Vec<MaskEntry>` 和手动复制 center/padding。逻辑等价，但 Rust 对 center/padding 的 copy 处理是手动的（当自身为空时才从父级复制），与 C# 的 `ReadInto` 行为略有差异。
- Rust 新增了 `load()` 方法（从文件路径），C# 构造函数直接从文件路径加载。

---

## 三、XML 解析子方法对比

### 3.1 `ReadInto()` (C#) → `parse_masks()` + `parse_special_tiles()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| `mask="center"` 处理 | `tiles = data.Center` | `parse_special_tiles(node, "center")` | ✅ 对齐 |
| `mask="padding"` 处理 | `tiles = data.Padded` | `parse_special_tiles(node, "padding")` | ✅ 对齐 |
| 数字 mask 解析 | 逐字符 `text[i]` 解析为 `0/1/2` | `parse_mask_str()` 过滤 `-` 后解析 | ✅ 对齐 |
| 坐标解析 | `array[j].Split(',')` + `int.Parse` | `parse_tile_coords()` | ✅ 对齐 |
| `sprites` 属性 | `xml2.Attr("sprites").Split(',')` → `OverlapSprites` | ❌ 无 | 🔴 缺失 |
| `HasOverlays` 标记 | `tiles.HasOverlays = true` | ❌ 无 | 🔴 缺失 |

### 3.2 `parse_mask_str()` (Rust 独有)

Rust 新增了专门的 mask 字符串解析方法，支持 `-` 分隔符（如 `"x0x-111-x1x"`），C# 版本不使用 `-` 分隔符。这是一个**增强**，可能是为了适配实际 XML 中使用的格式。

---

## 四、生成方法对比

### 4.1 `GenerateMap(VirtualMap<char>, bool)` (C#) → ❌ 缺失

| 状态 | 🔴 缺失 |
|------|---------|

Rust 版本不接受 `VirtualMap<char>` 输入，无法从关卡地图数据生成 tile grid。这是一个**核心功能缺失**。

### 4.2 `GenerateMap(VirtualMap<char>, Behaviour)` (C#) → ❌ 缺失

| 状态 | 🔴 缺失 |
|------|---------|

同上，Rust 不支持带自定义 `Behaviour` 的地图生成。

### 4.3 `GenerateBox(char, int, int)` (C#) → `generate_box()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| 生成实心矩形 | 调用 `Generate(null, ..., forceSolid: true, ...)` | 创建 `SolidGrid::from_rows()` 然后调用 `self.generate()` | 🟡 近似 |
| forceSolid 逻辑 | `Rectangle forceFill` 覆盖区域 | 通过构造全实心 `SolidGrid` 实现 | 🟡 近似 |

差异说明：
- C# 通过 `forceFill` 矩形在 `CheckTile`/`GetTile` 中强制返回 solid，Rust 通过构造一个全部填充的 `SolidGrid` 达到类似效果。
- 两种方式在结果上等价，但 Rust 的实现更简洁，C# 的 `forceFill` 机制允许在有 `mapData` 的情况下叠加强制区域。

### 4.4 `GenerateOverlay()` (C#) → ❌ 缺失

| 状态 | 🔴 缺失 |
|------|---------|

Rust 不支持覆盖层生成（用于叠加 tile 到已有地图上）。原版用于生成额外的 tile 覆盖在已有 tile 上。

### 4.5 `Generate()` (C# 私有) → `generate()` (Rust 公有)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| 创建 TileGrid | `new TileGrid(8, 8, tilesX, tilesY)` | 直接创建 `TileGrid` 结构体 | ✅ 对齐 |
| 创建 AnimatedTiles | `new AnimatedTiles(...)` | ❌ 无 | 🔴 缺失 |
| 50-tile 分段优化 | `i += 50; j += 50` + `AnyInSegmentAtTile` | ❌ 无（逐 tile 遍历） | 🟡 近似 |
| `forceFill` 逻辑 | `Rectangle forceFill` 参数 | ❌ 无（通过 SolidGrid 隐式实现） | 🟡 近似 |
| 随机选择纹理 | `Calc.Random.Choose(tiles.Textures)` | `DefaultHasher` 基于位置的确定性哈希 | 🟡 近似 |
| OverlapSprites | `animatedTiles.Set(...)` | ❌ 无 | 🔴 缺失 |
| 返回值 | `Generated` 结构体 | `TileGrid` 结构体 | 🟠 结构不同 |

差异说明：
- C# 使用 `Calc.Random.Choose()` 真正随机选择，Rust 使用位置哈希实现确定性伪随机。功能上近似但不完全一致。
- C# 有 50-tile 分段跳过空区域的优化，Rust 无此优化。

### 4.6 `TileHandler()` (C# 私有) → 内联于 `generate()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| 获取当前 tile | `GetTile(mapData, x, y, ...)` | `grid.tile_id_at_local(tx, ty)` | ✅ 对齐 |
| 空 tile 检查 | `IsEmpty(tile)` 返回 null | `None` 匹配直接 continue | ✅ 对齐 |
| 查找 TerrainType | `lookup[tile]` | `self.tilesets.get(&tile_ch)` | ✅ 对齐 |
| 3×3 邻域遍历 | `for i in -1..2; for j in -1..2` | `adjacency()` 函数用 offsets 数组 | ✅ 对齐 |
| `EdgesIgnoreOutOfLevel` | 检查 `behaviour.EdgesIgnoreOutOfLevel` | ❌ 不支持 | 🔴 缺失 |
| `CheckForSameLevel` | 调用 `CheckForSameLevel` | ❌ 不支持 | 🔴 缺失 |
| 全邻域 solid → center/padded | 检查 2-away 邻居 | 检查 `(-2,0),(2,0),(0,-2),(0,2)` | 🟡 近似 |
| `PaddingIgnoreOutOfLevel` | 影响 2-away 检查行为 | 硬编码为 true（始终检查 2-away） | 🟡 近似 |
| 遍历 Masked 查找匹配 | `foreach Masked` + `mask_matches` | `def.masks.iter().find()` | ✅ 对齐 |
| 无匹配返回 null | `return null` | `matched_tiles` 为 `None` | ✅ 对齐 |

### 4.7 `CheckForSameLevel()` (C#) → ❌ 缺失

| 状态 | 🔴 缺失 |
|------|---------|

原版遍历 `LevelBounds` 判断两个坐标是否在同一关卡边界内。Rust 完全不支持此功能。

### 4.8 `CheckTile()` (C# 私有) → `neighbor_connects()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| forceFill 检查 | `forceFill.Contains(x, y)` → return true | ❌ 无（通过 SolidGrid 隐式） | 🟡 近似 |
| mapData 为 null | `behaviour.EdgesExtend` | ❌ 不适用 | — |
| 越界 clamp | `Calc.Clamp(x, 0, Columns-1)` | `tx.clamp(0, w-1)` | ✅ 对齐 |
| `EdgesExtend` false 时越界返回 false | `return false` | ❌ 不支持 | 🔴 缺失 |
| 空 tile 检查 | `IsEmpty(c)` → return false | `Some(c) if c != '0'` 模式匹配 | ✅ 对齐 |
| Ignore 检查 | `!set.Ignore(c)` | `Self::connects_id(def, c)` | ✅ 对齐 |
| 同类型始终连接 | `ID != c` 时检查 ignores | `other == def.id` → return true | ✅ 对齐 |
| `*` 忽略所有 | `Ignores.Contains('*')` | `ignores.contains(&'*')` | ✅ 对齐 |

### 4.9 `GetTile()` (C# 私有) → 内联于 `generate()` (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| forceFill 检查 | `forceFill.Contains(x, y)` → forceID | ❌ 无 | 🔴 缺失 |
| mapData 为 null + EdgesExtend | 返回 forceID | ❌ 不适用 | — |
| 越界 + EdgesExtend false | 返回 '0' | ❌ 不支持（总是 clamp） | 🔴 缺失 |
| 越界 + EdgesExtend true | Clamp 到边界 tile | Clamp 到边界 tile | ✅ 对齐 |
| 正常读取 | `mapData[x, y]` | `grid.tile_id_at_local(tx, ty)` | ✅ 对齐 |

### 4.10 `IsEmpty()` (C# 私有) → 内联检查 (Rust)

| 功能点 | C# | Rust | 状态 |
|--------|-----|------|------|
| 空判断 | `id == '0' \|\| id == '\0'` | `c != '0'`（在 match 中 None 也处理） | 🟠 近似 |

差异说明：
- C# 明确检查 `'\0'` 作为空 tile，Rust 依赖 `SolidGrid::tile_id_at_local` 返回 `Option`，`None` 等价于 `'\0'`。

---

## 五、Rust 独有功能

### 5.1 `parse_mask_str()` — 增强的 mask 解析

支持 `-` 分隔符格式（如 `"x0x-111-x1x"`），C# 版本不支持此格式。这是对实际 XML 格式的适配。

### 5.2 `adjacency()` — 独立的邻域计算函数

将邻域计算提取为独立函数，比 C# 的内联方式更清晰。

### 5.3 `connects_id()` — 独立的连接判断

将 `TerrainType.Ignore()` 的反转逻辑提取为独立函数 `connects_id()`。

### 5.4 `mask_matches()` — 独立的 mask 匹配

将 mask 匹配逻辑提取为独立函数。

### 5.5 `TileGrid::tile_at()` — 便捷访问方法

Rust 新增了 `tile_at(tx, ty)` 方法，方便按坐标查询 tile 信息。

### 5.6 `origin_x` / `origin_y` — 世界坐标

Rust `TileGrid` 新增世界坐标原点，用于渲染时定位。

---

## 六、总结

### 完全对齐 ✅ (8 项)
- 数据结构：`ID`, `Ignores`, `lookup`/`tilesets`, `adjacent`/`adjacency()`
- XML 解析：id/path/ignores 属性读取, mask 解析（数字部分）, 坐标解析, 排序逻辑
- 核心逻辑：空 tile 检查, 同类型连接, `*` 忽略, 3×3 邻域, mask 匹配

### 部分实现 🟠 (5 项)
- `TerrainType`/`TilesetDef` 成员映射（Center/Padded 仅存坐标）
- `Masked`/`MaskEntry`（仅存坐标无纹理引用）
- `copy` 继承逻辑
- `Generated`/`TileGrid` 结构差异
- `IsEmpty` 的 null 检查方式

### 近似 🟡 (7 项)
- `Behaviour` 字段硬编码
- `GenerateBox` 实现方式
- `Generate` 的随机选择（哈希 vs 真随机）
- `forceFill` 通过 SolidGrid 隐式实现
- 50-tile 分段优化缺失
- `PaddingIgnoreOutOfLevel` 硬编码为 true
- 2-away padding 检查（Rust 仅检查边界内）

### 缺失 🔴 (9 项)
- **`GenerateMap(VirtualMap<char>, bool)`** — 核心：从关卡地图生成
- **`GenerateMap(VirtualMap<char>, Behaviour)`** — 核心：自定义行为生成
- **`GenerateOverlay()`** — 覆盖层生成
- **`AnimatedTiles SpriteOverlay`** — 动画覆盖层输出
- **`OverlapSprites` / `HasOverlays`** — 精灵覆盖层支持
- **`Behaviour` 结构体** — 完整行为控制
- **`LevelBounds`** — 关卡边界列表
- **`CheckForSameLevel()`** — 跨关卡边界检查
- **`EdgesExtend = false` 分支** — 边沿不延伸行为
