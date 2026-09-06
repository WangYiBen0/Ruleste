# BinaryPacker 对比分析

> 原版：`references/source/Celeste/Celeste/BinaryPacker.cs`
> 实现：`src/data/binary_packer.rs`

---

## 总览

| 区域 | 状态 | 说明 |
|------|------|------|
| Element 数据结构 | 🟠 部分实现 | C# 用 `Dictionary<string, object>`，Rust 用 `Vec<(String, Attr)>`；缺少 `Package` 字段（移至 `MapBin`）|
| Element 属性访问器 | 🟡 近似 | 签名和行为近似，C# `Attr` 统一返回 `string`，Rust 分类型返回 |
| 二进制写入（序列化） | 🔴 缺失 | Rust 实现仅读取，不支持从 XML/Element 生成 .bin 文件 |
| 二进制读取（反序列化） | ✅ 完全对齐 | 读取逻辑与原版一致，包含所有类型标签（0–7）|
| RLE 编解码 | 🟠 部分实现 | 仅实现解码（`rle_decode`），缺少编码（`RunLengthEncoding.Encode`）|
| 类型系统 | 🟡 近似 | Rust 用枚举 `Attr` 替代 C# 的 `object` 装箱，更类型安全 |

---

## 1. Element 内部类 / 结构体

### 1.1 字段对比

| C# 字段 | Rust 字段 | 状态 | 差异 |
|---------|----------|------|------|
| `string Package` | `MapBin.package: String` | 🟠 部分实现 | C# 中 `Package` 是 `Element` 的字段；Rust 中提升为 `MapBin` 顶层结构体的字段 |
| `string Name` | `Element.name: String` | ✅ 完全对齐 | — |
| `Dictionary<string, object> Attributes` | `Vec<(String, Attr)> attrs` | 🟠 部分实现 | C# 用哈希表（O(1) 查找），Rust 用有序向量（O(n) 查找）；C# 值类型为 `object`（装箱），Rust 用 `Attr` 枚举 |
| `List<Element> Children` | `Vec<Element> children` | ✅ 完全对齐 | — |

### 1.2 `HasAttr(string name)` 方法

| | C# | Rust |
|-|-----|------|
| 方法签名 | `public bool HasAttr(string name)` | 无直接对应 |
| 实现 | 检查 `Attributes.ContainsKey(name)` | 通过 `attr(name)` 返回 `Option<&Attr>` 间接实现 |
| 状态 | | 🟠 部分实现 |

**差异：** Rust 没有单独的 `HasAttr` 方法，但 `attr()` 返回 `Option<&Attr>` 可以通过 `.is_some()` 实现等效检查。

### 1.3 `Attr(string name, string defaultValue = "")` 方法

| | C# | Rust |
|-|-----|------|
| 方法签名 | `public string Attr(string name, string defaultValue = "")` | `pub fn attr_str(&self, name: &str, default: &str) -> String` |
| 返回类型 | `string`（始终返回 `string`，所有值调用 `.ToString()`） | `String` |
| 默认值 | 空字符串 `""` | 需显式传入 |
| 状态 | | 🟡 近似 |

**差异：**
- C# 中 `Attr` 对**所有类型**的值都调用 `.ToString()` 返回字符串（如 `bool` → `"True"` / `"False"`，`int` → 数字字符串）
- Rust 的 `attr_str` 仅对 `String` / `RleString` 类型返回内容，对其他类型（`Bool`/`Byte`/`Short`/`Int`/`Float`）返回空字符串 `""`（因为 `Attr::as_str` 对非字符串变体返回 `""`）
- 这是一个**行为差异**：C# 可以用 `Attr("someInt")` 获取 `"42"`，Rust 只能通过 `attr("someInt")` 拿到 `Attr::Int(42)`

### 1.4 `AttrBool(string name, bool defaultValue = false)` 方法

| | C# | Rust |
|-|-----|------|
| 方法签名 | `public bool AttrBool(string name, bool defaultValue = false)` | `pub fn attr_bool(&self, name: &str, default: bool) -> bool` |
| 返回类型 | `bool` | `bool` |
| 状态 | | ✅ 完全对齐 |

**差异（细微）：**
- C#：若值为 `bool` 类型直接返回；否则调用 `bool.Parse(value.ToString())`
- Rust：`Attr::as_bool` 对所有类型做转换（`u8 != 0`、`i16 != 0`、`i32 != 0`、`f32 != 0.0`、字符串匹配 `"true" | "1"`）
- Rust 版本行为更宽松，能处理更多类型，但可能对 `"True"`（大写 T）不匹配（C# 的 `bool.Parse` 接受 `"True"`）

### 1.5 `AttrFloat(string name, float defaultValue = 0f)` 方法

| | C# | Rust |
|-|-----|------|
| 方法签名 | `public float AttrFloat(string name, float defaultValue = 0f)` | `pub fn attr_f32(&self, name: &str, default: f32) -> f32` |
| 返回类型 | `float` | `f32` |
| 状态 | | ✅ 完全对齐 |

**差异（细微）：**
- C#：若值为 `float` 直接返回；否则 `float.Parse(value.ToString(), InvariantCulture)`
- Rust：`Attr::as_f32` 对字符串做 `.parse().unwrap_or(0.0)`，解析失败返回 `0.0` 而非 panic

### 1.6 Rust 额外方法（C# 中不存在）

| 方法 | 说明 | 状态 |
|------|------|------|
| `pub fn attr(&self, name: &str) -> Option<&Attr>` | 返回属性的枚举引用 | 🟡 新增（C# 无对应，因为 C# 返回 `object`） |
| `pub fn as_f32(&self) -> f32` | `Attr` 枚举上的方法，将任意变体转为 `f32` | 🟡 新增 |
| `pub fn as_bool(&self) -> bool` | `Attr` 枚举上的方法，将任意变体转为 `bool` | 🟡 新增 |
| `pub fn as_str(&self) -> &str` | `Attr` 枚举上的方法，取字符串内容 | 🟡 新增 |
| `pub fn child(&self, name: &str) -> Option<&Element>` | 查找第一个子元素 | 🔴 缺失（C# 无对应） |
| `pub fn children_named(&self, name: &str) -> impl Iterator<Item = &Element>` | 遍历同名子元素 | 🔴 缺失（C# 无对应） |

---

## 2. 静态字段

| C# 字段 | Rust 对应 | 状态 | 差异 |
|---------|----------|------|------|
| `HashSet<string> IgnoreAttributes = { "_eid" }` | 无 | 🟡 近似 | C# 在写入时忽略 `_eid` 属性；Rust 读取端无需处理（写入端已跳过） |
| `string InnerTextAttributeName = "innerText"` | 无（硬编码在格式中） | 🟡 近似 | 仅写入端使用 |
| `string OutputFileExtension = ".bin"` | 无 | 🟡 近似 | 仅写入端使用 |
| `Dictionary<string, short> stringValue` | 无（写入专用） | 🟡 近似 | 写入时的字符串→索引映射表 |
| `string[] stringLookup` | `table: &[String]`（`read_element` 参数） | ✅ 完全对齐 | 作用相同：索引→字符串查找表。C# 存为静态字段，Rust 通过参数传递 |
| `short stringCounter` | 无（写入专用） | 🟡 近似 | 写入时的计数器 |

---

## 3. 写入方法（ToBinary 系列）

> **整体状态：🔴 缺失** — Rust 实现不含任何写入逻辑。

| C# 方法 | Rust 对应 | 状态 | 说明 |
|---------|----------|------|------|
| `public static void ToBinary(string filename, string outdir = null)` | 无 | 🔴 缺失 | 从 XML 文件转换为 .bin |
| `public static void ToBinary(XmlElement rootElement, string outfilename)` | 无 | 🔴 缺失 | 将 XML 元素写入 .bin 文件 |
| `private static void CreateLookupTable(XmlElement element)` | 无 | 🔴 缺失 | 递归构建字符串查找表 |
| `private static void AddLookupValue(string name)` | 无 | 🔴 缺失 | 向字符串表添加条目 |
| `private static void WriteElement(BinaryWriter writer, XmlElement element)` | 无 | 🔴 缺失 | 递归写入元素 |
| `private static bool ParseValue(string value, out byte type, out object result)` | 无 | 🔴 缺失 | 将字符串值解析为类型化值 |

**说明：** 这是**设计决策**，不是遗漏。Rust 版本专注于读取预编译的 .bin 文件。原版 Celeste 使用此写入功能在编辑器中将 XML 地图转为二进制格式，而 Ruleste 不需要重新生成这些文件。

---

## 4. 读取方法

### 4.1 `FromBinary(string filename)`

| | C# | Rust |
|-|-----|------|
| 方法签名 | `public static Element FromBinary(string filename)` | `pub fn from_file(path: &Path) -> ReadResult<MapBin>` / `pub fn from_bytes(bytes: &[u8]) -> ReadResult<MapBin>` |
| 返回类型 | `Element`（`Package` 设置在 Element 上） | `MapBin { package, root }` |
| 错误处理 | 无（可抛出异常） | `ReadResult<MapBin>`（Result 类型） |
| 状态 | | ✅ 完全对齐 |

**差异：**
- C# 版本将 `package` 存储在 `Element.Package` 中；Rust 版本使用独立的 `MapBin` 结构体包裹 `package` 和 `root`
- Rust 版本增加了 magic 字符串校验（`"CELESTE MAP"`），C# 版本不校验
- Rust 版本支持从内存字节切片读取（`from_bytes`），不仅限于文件路径

### 4.2 `ReadElement(BinaryReader reader)`

| | C# | Rust |
|-|-----|------|
| 方法签名 | `private static Element ReadElement(BinaryReader reader)` | `fn read_element(reader: &mut Reader<'_>, table: &[String]) -> ReadResult<Element>` |
| 参数 | 仅 `BinaryReader` | `Reader` + `&[String]`（字符串查找表通过参数传递） |
| 状态 | | ✅ 完全对齐 |

**类型标签处理对比：**

| 标签 | C# 读取 | Rust 读取 | 状态 |
|------|---------|----------|------|
| 0 (Bool) | `reader.ReadBoolean()` → `object` | `reader.read_bool()?` → `Attr::Bool(bool)` | ✅ |
| 1 (Byte) | `Convert.ToInt32(reader.ReadByte())` → `object` | `reader.read_u8()?` → `Attr::Byte(u8)` | ✅ |
| 2 (Short) | `Convert.ToInt32(reader.ReadInt16())` → `object` | `reader.read_i16()?` → `Attr::Short(i16)` | ✅ |
| 3 (Int) | `reader.ReadInt32()` → `object` | `reader.read_i32()?` → `Attr::Int(i32)` | ✅ |
| 4 (Float) | `reader.ReadSingle()` → `object` | `reader.read_f32()?` → `Attr::Float(f32)` | ✅ |
| 5 (Str) | `stringLookup[reader.ReadInt16()]` → `object` | `table[i16]` → `Attr::String(String)` | ✅ |
| 6 (String) | `reader.ReadString()` → `object` | `reader.read_dotnet_string()?` → `Attr::String(String)` | ✅ |
| 7 (RleString) | `RunLengthEncoding.Decode(reader.ReadBytes(count))` → `object` | `rle_decode(reader.take(len)?)` → `Attr::RleString(String)` | ✅ |

**关键差异：**
- C# 将 Byte 和 Short 读取后统一转换为 `int`（`Convert.ToInt32`），丢失了原始类型信息
- Rust 保留了原始类型：`Attr::Byte(u8)` 和 `Attr::Short(i16)`，信息更精确

### 4.3 未知类型处理

| | C# | Rust |
|-|-----|------|
| 未知类型标签 | 静默跳过（`value = null`） | 返回错误 `ReadError::new("unknown attr type")` |
| 状态 | | 🟡 近似 |

**差异：** Rust 版本更严格，遇到未知类型会报错；C# 版本则静默忽略。

---

## 5. Attr 枚举类型系统

### 5.1 AttrType 枚举

| C# 标签 | Rust 变体 | 值 | 状态 |
|---------|----------|-----|------|
| 0 | `AttrType::Bool` | 0 | ✅ |
| 1 | `AttrType::Byte` | 1 | ✅ |
| 2 | `AttrType::Short` | 2 | ✅ |
| 3 | `AttrType::Int` | 3 | ✅ |
| 4 | `AttrType::Float` | 4 | ✅ |
| 5 | `AttrType::Str` | 5 | ✅ |
| 6 | `AttrType::String` | 6 | ✅ |
| 7 | `AttrType::RleString` | 7 | ✅ |

### 5.2 Attr 枚举值

| C# 类型 | Rust 变体 | Rust 值类型 | 状态 | 差异 |
|---------|----------|------------|------|------|
| `bool` | `Attr::Bool(bool)` | `bool` | ✅ | — |
| `byte`（实际存为 `int`） | `Attr::Byte(u8)` | `u8` | ✅ | Rust 保留原始类型；C# 升级为 `int` |
| `short`（实际存为 `int`） | `Attr::Short(i16)` | `i16` | ✅ | Rust 保留原始类型；C# 升级为 `int` |
| `int` | `Attr::Int(i32)` | `i32` | ✅ | — |
| `float` | `Attr::Float(f32)` | `f32` | ✅ | — |
| `string`（查表） | `Attr::String(String)` | `String` | ✅ | — |
| `string`（内联） | `Attr::String(String)` | `String` | ✅ | C# 类型 5 和 6 都存为 `string`，Rust 也统一为 `Attr::String`（但类型 7 区分为 `Attr::RleString`）|
| `RleString` | `Attr::RleString(String)` | `String` | ✅ | — |
| C# `object`（通用装箱） | 无 | — | 🟡 近似 | Rust 用枚举替代装箱，类型更安全 |

---

## 6. RLE 编解码

### 6.1 `RunLengthEncoding.Decode` → `rle_decode`

| | C# | Rust |
|-|-----|------|
| 方法 | `RunLengthEncoding.Decode(byte[])` | `fn rle_decode(bytes: &[u8]) -> String` |
| 算法 | 相同：每对 `(count, char)` 重复 char count 次 | 相同 |
| 返回类型 | `string` | `String` |
| 状态 | | ✅ 完全对齐 |

### 6.2 `RunLengthEncoding.Encode` → 无

| | C# | Rust |
|-|-----|------|
| 方法 | `RunLengthEncoding.Encode(string)` | 无 |
| 状态 | | 🔴 缺失 |

**说明：** 编码仅在写入端使用（`WriteElement` 中 `solids` 和 `bg` 元素的 `InnerText`）。Rust 不需要此功能。

---

## 7. 额外差异

### 7.1 错误处理

| | C# | Rust |
|-|-----|------|
| 策略 | 异常（`FileNotFoundException`, `FormatException` 等） | `Result<T, ReadError>` |
| 状态 | | 🟡 近似（语言惯用差异）|

### 7.2 不可变性

| | C# | Rust |
|-|-----|------|
| `Element` | 可变（字段为 public，无封装） | 不可变（字段为 `pub`，但 `Element` 本身无 `&mut self` 方法） |
| `Attributes` | 可在读取后动态增删 | `Vec<(String, Attr)>`，只读 |
| 状态 | | 🟡 近似 |

### 7.3 线程安全

| | C# | Rust |
|-|-----|------|
| 静态字段 | `stringValue`, `stringLookup`, `stringCounter` 是共享静态可变状态 | 无全局状态，`table` 通过参数传递 |
| 状态 | | 🟠 部分实现 |

**差异：** C# 的 `FromBinary` 使用全局静态 `stringLookup`，多线程调用时存在数据竞争风险。Rust 版本将字符串表作为参数传递，天然线程安全。

---

## 8. 总结

| 功能 | C# | Rust | 说明 |
|------|:--:|:----:|------|
| Element 结构 | ✅ | ✅ | 字段等价，`Package` 提升为 `MapBin` |
| 属性访问器 | ✅ | ✅ | 签名近似，行为微调 |
| 二进制读取 | ✅ | ✅ | 核心功能完全对齐 |
| 类型标签 (0-7) | ✅ | ✅ | 所有类型正确解析 |
| 二进制写入 | ✅ | 🔴 | 设计决策：Rust 不实现写入 |
| RLE 解码 | ✅ | ✅ | 算法一致 |
| RLE 编码 | ✅ | 🔴 | 写入端不需要 |
| 类型安全 | 🟠 | ✅ | Rust 枚举优于 C# `object` 装箱 |
| 线程安全 | 🟠 | ✅ | Rust 无全局可变状态 |
| 错误处理 | 🟠 | ✅ | Rust 使用 `Result` 类型 |
| `child()` / `children_named()` | ❌ | ✅ | Rust 新增便利方法 |

**整体评估：** Rust 实现在**读取路径**上与原版 C# 完全对齐，所有二进制格式标签和解码逻辑一致。写入路径未实现（设计决策）。类型系统更优（枚举 vs 装箱），线程安全性更好。新增了 `child()` 和 `children_named()` 便利方法。
