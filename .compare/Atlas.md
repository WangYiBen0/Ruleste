# Atlas 对比分析：Monocle (C#) vs Ruleste (Rust)

> 原版：`references/source/Celeste/Monocle/Atlas.cs` (396 行)
> Rust：`src/data/atlas.rs` (372 行)

---

## 概览

| 维度 | C# (Monocle) | Rust (Ruleste) |
|------|-------------|----------------|
| 类型系统 | 单一 `Atlas` 类 + 内部枚举 | 多结构体：`AtlasMeta`、`Page`、`Frame`、`AtlasPage`、`Atlas` |
| 支持格式 | 7 种 (`TexturePacker_Sparrow`, `CrunchXml`, `CrunchBinary`, `CrunchBinaryNoAtlas`, `CrunchXmlOrBinary`, `Packer`, `PackerNoAtlas`) | 2 种 (`Packer`, `PackerNoAtlas`) |
| 纹理管理 | `VirtualTexture` 延迟加载 + `MTexture` 裁剪 | 直接解码 `.data` RLE 为 `Vec<u8>` RGBA |
| 链接系统 | ✅ `links` 字典 + `GetLinkedTexture` | 🔴 缺失 |
| 子纹理序列 | ✅ `GetAtlasSubtextures` 带缓存 | 🔴 缺失 |
| 目录加载 | ✅ `FromDirectory` | 🟠 `load_atlas_dir`（扫描目录，自动选择格式） |
| 内存管理 | 手动 `Dispose()` | Rust 自动 Drop |
| 默认索引器 | ✅ `this[string]` | 🔴 缺失（使用 `frame_index` HashMap） |

---

## 逐方法对比

### 1. 枚举 `AtlasDataFormat` — 🔴 缺失

**C#：**
```csharp
public enum AtlasDataFormat
{
    TexturePacker_Sparrow,
    CrunchXml,
    CrunchBinary,
    CrunchBinaryNoAtlas,
    CrunchXmlOrBinary,
    Packer,
    PackerNoAtlas
}
```

**Rust：** 无对应枚举。仅硬编码支持 `Packer` 和 `PackerNoAtlas` 两种格式（通过 `load_one` 中检测 `.data` 文件是否存在来自动区分）。

**差异：**
- C# 支持 7 种格式，Rust 仅支持实际使用的 2 种
- 其他格式（Sparrow XML、Crunch 系列）属于 Celeste Mod 生态或旧版格式，Ruleste 暂不需要

---

### 2. 字段 `Sources` — 🟡 近似

**C#：**
```csharp
public List<VirtualTexture> Sources;
```

**Rust：** `Atlas.pages: Vec<AtlasPage>`，每个 `AtlasPage` 内含已解码的 RGBA 像素数据。

**差异：**
- C# 的 `VirtualTexture` 是延迟加载的虚拟纹理，实际像素在渲染时才请求
- Rust 直接在 `load` 时解码全部 `.data` 文件为内存中的 RGBA buffer
- 语义等价：都表示"这个 atlas 由哪些纹理页组成"

---

### 3. 字段 `textures` — 🟠 部分实现

**C#：**
```csharp
private Dictionary<string, MTexture> textures = new Dictionary<string, MTexture>(StringComparer.OrdinalIgnoreCase);
```

**Rust：**
```rust
pub frame_index: HashMap<String, (usize, usize)>
```

**差异：**
- C# 存储完整的 `MTexture` 对象（含裁剪矩形、偏移、帧尺寸、源纹理引用）
- Rust 仅存储 `(page_index, frame_index)` 索引对，纹理数据在 `pages[pi].frames[fi]` 中
- C# 使用 **大小写不敏感** 字典；Rust 使用默认大小写敏感 `HashMap`
- 语义等价：都通过字符串 id 查找纹理

---

### 4. 字段 `orderedTexturesCache` — 🔴 缺失

**C#：**
```csharp
private Dictionary<string, List<MTexture>> orderedTexturesCache = new Dictionary<string, List<MTexture>>();
```

**Rust：** 无对应实现。

**差异：**
- C# 缓存序列化子纹理（如 `"tilegroup0"`, `"tilegroup1"`, ...），用于动画帧等
- Rust 完全缺失此功能，调用方需要自行实现序列帧查找

---

### 5. 字段 `links` — 🔴 缺失

**C#：**
```csharp
private Dictionary<string, string> links = new Dictionary<string, string>();
```

**Rust：** 无对应实现。

**差异：**
- C# 的 `links` 存储从别名到真实纹理 id 的映射（从 `.meta` 文件尾部的 `"LINKS"` 段读取）
- Rust 的 `AtlasMeta::from_bytes` 在读取完帧数据后直接返回，**忽略了 LINKS 段**
- 影响：某些通过链接查找纹理的功能将失败

---

### 6. 索引器 `this[string id]` — 🔴 缺失

**C#：**
```csharp
public MTexture this[string id]
{
    get { return textures[id]; }
    set { textures[id] = value; }
}
```

**Rust：** 无对应索引器。访问帧需通过 `frame_index` + `pages` 手动查找。

**差异：**
- C# 提供便捷的字典式访问 `atlas["key"]`
- Rust 需要 `atlas.frame_index.get(id).map(|(pi, fi)| &atlas.pages[*pi].frames[*fi])`
- Rust 的设计更显式，但使用略繁琐

---

### 7. `FromAtlas(path, format)` — 🟠 部分实现

**C#：**
```csharp
public static Atlas FromAtlas(string path, AtlasDataFormat format)
{
    Atlas obj = new Atlas { Sources = new List<VirtualTexture>() };
    ReadAtlasData(obj, path, format);
    return obj;
}
```

**Rust：**
```rust
pub fn load(base: &Path) -> ReadResult<Atlas> { ... }     // Packer 格式
pub fn load_no_pack(base: &Path) -> ReadResult<Atlas> { ... }  // PackerNoAtlas 格式
```

**差异：**
- C# 通过 `format` 参数分发到不同解析器
- Rust 将两种格式拆分为独立方法，由调用方（`load_one`）自动选择
- 功能等价（对 Celeste 实际使用的格式）

---

### 8. `ReadAtlasData(atlas, path, format)` — 🟠 部分实现

**C#：** 一个巨大的 `switch` 语句，处理 7 种格式。

**Rust：** 分散在多个方法中：
- `AtlasMeta::from_bytes` — 解析 `.meta` 二进制（Packer/PackerNoAtlas 共用）
- `AtlasPage::decode` — 解析 `.data` RLE 纹理
- `Atlas::load` — 组合 meta + pages（Packer）
- `Atlas::load_no_pack` — 组合 meta + 各自独立的 .data（PackerNoAtlas）

**差异：**
- C# 的 `CrunchBinary` 格式（104-128 行）：读取 short 序列的帧数据，Rust 未实现
- C# 的 `CrunchBinaryNoAtlas` 格式（130-157 行）：每帧单独 .png，Rust 未实现
- C# 的 `CrunchXml` 格式（79-101 行）：XML 解析，Rust 未实现
- C# 的 `TexturePacker_Sparrow` 格式（56-77 行）：Sparrow XML，Rust 未实现
- C# 的 `CrunchXmlOrBinary` 格式（241-250 行）：自动检测 .bin/.xml，Rust 未实现

---

### 9. `FromMultiAtlas(rootPath, dataPath[], format)` — 🔴 缺失

**C#：**
```csharp
public static Atlas FromMultiAtlas(string rootPath, string[] dataPath, AtlasDataFormat format)
{
    Atlas atlas = new Atlas();
    atlas.Sources = new List<VirtualTexture>();
    for (int i = 0; i < dataPath.Length; i++)
        ReadAtlasData(atlas, Path.Combine(rootPath, dataPath[i]), format);
    return atlas;
}
```

**Rust：** 无直接对应。`load_atlas_dir` 提供了类似功能（扫描目录下所有 `.meta`）。

**差异：**
- C# 允许显式指定多个数据文件路径
- Rust 自动发现目录下所有 `.meta` 文件
- 语义近似但接口不同

---

### 10. `FromMultiAtlas(rootPath, filename, format)` — 🔴 缺失

**C#：**
```csharp
public static Atlas FromMultiAtlas(string rootPath, string filename, AtlasDataFormat format)
{
    // 自动探测 filename0.xml, filename1.xml, ... 直到文件不存在
}
```

**Rust：** 无对应实现。

**差异：**
- C# 支持按编号自动探测多个 atlas 文件
- Rust 未实现此功能（Celeste 实际打包后不使用此路径）

---

### 11. `FromDirectory(path)` — 🟠 部分实现

**C#：**
```csharp
public static Atlas FromDirectory(string path)
{
    // 扫描目录下所有 .png/.xnb 文件，每个文件作为一个纹理
}
```

**Rust：**
```rust
pub fn load_atlas_dir(dir: &Path) -> ReadResult<Atlas> {
    // 扫描目录下所有 .meta 文件，自动区分 Packer/PackerNoAtlas
}
```

**差异：**
- C# 直接加载原始图片文件（.png/.xnb）
- Rust 加载的是已转换的 `.meta` + `.data` 格式
- Rust 的 `load_one` 自动检测格式（通过检查 `.data` 文件是否存在）
- 语义近似：都是"从目录加载所有可用纹理"

---

### 12. `Has(id)` — 🔴 缺失

**C#：**
```csharp
public bool Has(string id)
{
    return textures.ContainsKey(id);
}
```

**Rust：** 无对应方法。调用方需直接检查 `frame_index.contains_key(id)`。

**差异：**
- 功能简单，Rust 缺失此便捷方法
- 调用方可用 `atlas.frame_index.contains_key(id)` 替代

---

### 13. `GetOrDefault(id, defaultTexture)` — 🔴 缺失

**C#：**
```csharp
public MTexture GetOrDefault(string id, MTexture defaultTexture)
{
    if (string.IsNullOrEmpty(id) || !Has(id))
        return defaultTexture;
    return textures[id];
}
```

**Rust：** 无对应方法。

**差异：**
- Rust 调用方可通过 `frame_index.get(id).or(Some(fallback))` 实现
- 缺失 null/空字符串检查（Rust 中 `&str` 不存在 null）

---

### 14. `GetAtlasSubtextures(key)` — 🔴 缺失

**C#：**
```csharp
public List<MTexture> GetAtlasSubtextures(string key)
{
    // 查找 key, key0, key000000, key000001, ... 直到不存在
    // 结果缓存到 orderedTexturesCache
}
```

**Rust：** 无对应实现。

**差异：**
- C# 支持自动发现序列帧（如 `"tilegroup0"`, `"tilegroup1"`）
- Rust 完全缺失此功能
- 影响：动画帧、序列纹理的查找需要调用方自行实现

---

### 15. `GetAtlasSubtextureFromCacheAt(key, index)` — 🔴 缺失

**C#：**
```csharp
private MTexture GetAtlasSubtextureFromCacheAt(string key, int index)
{
    return orderedTexturesCache[key][index];
}
```

**Rust：** 无对应实现（依赖 `orderedTexturesCache`，而该缓存不存在）。

---

### 16. `GetAtlasSubtextureFromAtlasAt(key, index)` — 🔴 缺失

**C#：**
```csharp
private MTexture GetAtlasSubtextureFromAtlasAt(string key, int index)
{
    // index==0 时先尝试精确匹配 key
    // 然后尝试 key + 零填充序号（至少 6 位）
}
```

**Rust：** 无对应实现。

**差异：**
- C# 的零填充逻辑（`"0" + text` 循环至 6 位）是序列帧查找的核心
- Rust 无此功能

---

### 17. `GetAtlasSubtexturesAt(key, index)` — 🔴 缺失

**C#：**
```csharp
public MTexture GetAtlasSubtexturesAt(string key, int index)
{
    // 先查缓存，缓存未命中则调用 GetAtlasSubtextureFromAtlasAt
}
```

**Rust：** 无对应实现。

---

### 18. `GetLinkedTexture(key)` — 🔴 缺失

**C#：**
```csharp
public MTexture GetLinkedTexture(string key)
{
    if (key != null && links.TryGetValue(key, out var value) 
        && textures.TryGetValue(value, out var value2))
        return value2;
    return null;
}
```

**Rust：** 无对应实现。`links` 字段也不存在。

**差异：**
- C# 从 `.meta` 尾部的 `"LINKS"` 段读取别名映射
- Rust 的 `from_bytes` 完全忽略了 LINKS 段
- 影响：如果存在链接别名，查找会失败

---

### 19. `Dispose()` — 🟡 近似

**C#：**
```csharp
public void Dispose()
{
    foreach (VirtualTexture source in Sources)
        source.Dispose();
    Sources.Clear();
    textures.Clear();
}
```

**Rust：** 无对应方法。`Atlas` 结构体自动实现 `Drop`。

**差异：**
- C# 需要手动释放 `VirtualTexture`（可能持有 GPU 资源或文件句柄）
- Rust 的 `Vec<u8>` 和 `HashMap` 在离开作用域时自动释放
- 功能等价：内存都会被回收

---

### 20. `AtlasMeta::from_bytes` — ✅ 完全对齐

**C#（Packer 格式，158-197 行）：**
```csharp
binaryReader.ReadInt32();        // unused
binaryReader.ReadString();       // source dir
binaryReader.ReadInt32();        // unused
short num6 = binaryReader.ReadInt16();  // page count
// ... 读取每个 page 的 name, frame count, frame 数据
```

**Rust：**
```rust
r.read_i32()?;                           // unused
let source_dir = r.read_dotnet_string()?.to_string();
r.read_i32()?;                           // unused
let page_count = r.read_i16()?;
// ... 读取每个 page 的 name, frame count, frame 数据
```

**差异：** 二进制格式完全对齐，读取逻辑一致。Rust 额外处理了反斜杠替换（`replace('\\', "/")`），与 C# 一致。

---

### 21. `AtlasMeta::from_file` — ✅ 完全对齐

**C#：** 在各 `ReadAtlasData` 分支中使用 `File.OpenRead`。

**Rust：**
```rust
pub fn from_file(path: &Path) -> ReadResult<AtlasMeta> {
    let bytes = std::fs::read(path)?;
    Self::from_bytes(&bytes)
}
```

**差异：** 功能一致，错误处理方式不同（Rust 返回 `Result`）。

---

### 22. `AtlasMeta::frame_ids` — ✅ 完全对齐

**C#：** 无直接对应（C# 通过遍历 `textures` 字典获取所有 id）。

**Rust：**
```rust
pub fn frame_ids(&self) -> impl Iterator<Item = &str> {
    self.pages.iter().flat_map(|p| p.frames.iter()).map(|f| f.id.as_str())
}
```

**差异：** Rust 提供了便捷的迭代器方法，C# 需要手动遍历。

---

### 23. `AtlasPage::decode` — ✅ 完全对齐

**C#（VirtualTexture 的 RLE 解码逻辑，未在 Atlas.cs 中直接体现）：**
```csharp
// VirtualTexture 内部解码 .data 格式
// u32 width, u32 height, u8 has_alpha
// runs: u8 run_len, [u8 alpha,] u8 b, g, r [, alpha]
```

**Rust：**
```rust
pub fn decode(name: String, bytes: &[u8]) -> ReadResult<AtlasPage> {
    let width = r.read_u32()?;
    let height = r.read_u32()?;
    let has_alpha = r.read_u8()? != 0;
    // ... RLE 解码循环
}
```

**差异：** RLE 格式解析完全对齐，字节序和编码方式一致。

---

### 24. `Atlas::load` — ✅ 完全对齐

**C#（Packer 格式，158-197 行）：**
```csharp
// 读取 .meta → 解析 pages → 为每个 page 创建 VirtualTexture
// 读取帧数据 → 创建 MTexture（带裁剪矩形和偏移）
```

**Rust：**
```rust
pub fn load(base: &Path) -> ReadResult<Atlas> {
    let meta = AtlasMeta::from_file(base)?;
    // 解析 pages → 为每个 page 加载 .data → 解码 RGBA → 建立 frame_index
}
```

**差异：**
- C# 使用延迟加载（`VirtualTexture`），Rust 直接解码到内存
- C# 的 `MTexture` 包含更多元数据（如 `AtlasPath`），Rust 仅保留必要字段
- 功能等价：都建立了从帧 id 到纹理数据的映射

---

### 25. `Atlas::load_no_pack` — ✅ 完全对齐

**C#（PackerNoAtlas 格式，199-239 行）：**
```csharp
// 读取 .meta → 每帧单独加载 .data 文件
// clip rect 为整个纹理尺寸
// offset 保留未裁剪的框
```

**Rust：**
```rust
pub fn load_no_pack(base: &Path) -> ReadResult<Atlas> {
    let meta = AtlasMeta::from_file(base)?;
    // 每帧单独加载 .data → clip 设为全尺寸 → offset 保留原值
}
```

**差异：**
- C# 设置了 `AtlasPath` 属性，Rust 未设置
- 功能等价：都为每帧创建独立的纹理页

---

### 26. `Atlas::merge` — 🟡 近似

**C#：** 无直接对应。通过 `FromMultiAtlas` 多次调用 `ReadAtlasData` 实现类似效果。

**Rust：**
```rust
pub fn merge(&mut self, other: Atlas) {
    let base = self.pages.len();
    for (id, (pi, fi)) in other.frame_index {
        self.frame_index.insert(id, (base + pi, fi));
    }
    self.pages.extend(other.pages);
}
```

**差异：**
- Rust 提供了显式的合并方法，C# 需要多次调用工厂方法
- Rust 的合并会覆盖同名帧（`insert` 行为），C# 的 `FromMultiAtlas` 也会覆盖
- 设计差异：Rust 更灵活，支持运行时动态合并

---

### 27. `Atlas::frame_rgba_into` — 🔴 缺失

**C#：** 无直接对应。`MTexture` 的像素数据通过 `VirtualTexture` 的延迟加载机制获取。

**Rust：**
```rust
pub fn frame_rgba_into(&self, id: &str) -> Option<Vec<u8>> {
    let (pi, fi) = self.frame_index.get(id)?;
    let page = &self.pages[*pi];
    let frame = &page.frames[*fi];
    // 从 page.rgba 中提取 clip 区域的像素
}
```

**差异：**
- Rust 提供了直接获取帧像素数据的方法
- C# 依赖 `VirtualTexture` 的延迟加载机制，不直接暴露像素数据
- Rust 的设计更适合软件渲染或自定义纹理上传

---

### 28. `Atlas::frame_clip` — 🔴 缺失

**C#：** 无直接对应。裁剪矩形存储在 `MTexture` 对象中。

**Rust：**
```rust
pub fn frame_clip(&self, id: &str) -> Option<FrameRect> {
    let (pi, fi) = self.frame_index.get(id)?;
    Some(self.pages[*pi].frames[*fi].clip)
}
```

**差异：**
- Rust 提供了直接获取帧裁剪矩形的方法
- C# 需要通过 `MTexture.ClipRect` 属性访问
- 功能等价，接口不同

---

### 29. `load_one`（内部辅助函数） — 🔴 缺失

**C#：** 无直接对应。格式检测在 `ReadAtlasData` 的 switch 语句中处理。

**Rust：**
```rust
fn load_one(dir: &Path, meta_name: &str) -> Option<Atlas> {
    // 自动检测 Packer vs PackerNoAtlas 格式
}
```

**差异：**
- Rust 提供了自动格式检测的便捷函数
- C# 需要调用方显式指定格式
- Rust 的设计更符合"约定优于配置"原则

---

### 30. `load_atlas_dir`（公共入口） — 🔴 缺失

**C#：** 无直接对应。`FromDirectory` 加载原始图片，而非 `.meta` + `.data` 格式。

**Rust：**
```rust
pub fn load_atlas_dir(dir: &Path) -> ReadResult<Atlas> {
    // 扫描目录 → 加载所有 .meta → 自动检测格式 → 合并
    // 特殊处理：Gameplay 和 Misc 最后加载（最高优先级）
}
```

**差异：**
- Rust 提供了完整的目录级 atlas 加载和合并逻辑
- C# 的 `FromDirectory` 仅加载原始图片，不处理 `.meta` 格式
- Rust 的实现考虑了加载顺序（`Gameplay` 和 `Misc` 最后加载，覆盖其他同名帧）

---

## 总结

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 7 | 23% |
| 🟠 部分实现 | 3 | 10% |
| 🟡 近似 | 3 | 10% |
| 🔴 缺失 | 17 | 57% |

### 关键缺失功能

1. **序列帧查找**（`GetAtlasSubtextures` / `GetAtlasSubtextureFromAtlasAt`）
   - 影响：动画帧、序列纹理无法自动发现
   - 建议：在 Rust 中实现类似的零填充序号查找逻辑

2. **链接系统**（`links` + `GetLinkedTexture`）
   - 影响：通过别名查找纹理会失败
   - 建议：在 `from_bytes` 中解析 LINKS 段，添加 `links` 字段和查询方法

3. **便捷查询方法**（`Has` / `GetOrDefault`）
   - 影响：调用方代码略显冗余
   - 建议：添加 `has_id` 和 `get_or_default` 方法

4. **其他格式支持**（Sparrow XML / Crunch 系列）
   - 影响：仅影响 Celeste Mod 生态或旧版格式
   - 建议：按需实现，优先级较低

### Rust 的优势

1. **更清晰的类型分离**：`AtlasMeta`（元数据）与 `AtlasPage`（像素数据）分离，职责明确
2. **直接像素访问**：`frame_rgba_into` 和 `frame_clip` 提供了 C# 中不存在的底层访问能力
3. **自动格式检测**：`load_one` 自动区分 Packer/PackerNoAtlas，无需调用方指定
4. **内存安全**：无需手动 `Dispose`，Rust 的所有权系统确保资源自动释放
5. **合并逻辑**：`merge` 方法支持运行时动态合并，C# 需要多次调用工厂方法

### 建议优先实现

1. **序列帧查找**：实现类似 C# 的零填充序号查找，支持动画帧
2. **链接系统**：解析 LINKS 段，添加别名映射查询
3. **便捷方法**：添加 `has_id`、`get_or_default`、`get_subtextures` 等方法
4. **格式扩展**：按需添加 Sparrow XML / Crunch 格式支持（优先级低）
