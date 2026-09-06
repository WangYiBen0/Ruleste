# SpriteBank 对比分析

> C# 原版: `references/source/Celeste/Monocle/SpriteBank.cs`
> Rust 实现: `src/data/spritebank.rs`

---

## 类型/结构体对应

| C# | Rust | 状态 | 说明 |
|----|------|------|------|
| `class SpriteBank` | `struct SpriteBank` | 🟠 部分实现 | C# 是含运行时 Sprite 创建能力的类；Rust 仅是纯数据解析器 |
| `Atlas Atlas` 字段 | ❌ | 🔴 缺失 | Rust 侧无 Atlas 引用，SpriteBank 纯做 XML 解析 |
| `XmlDocument XML` 字段 | ❌ | 🔴 缺失 | Rust 用 `roxmltree::Document` 解析后丢弃，不保留 XML |
| `Dictionary<string, SpriteData> SpriteData` | `HashMap<String, SpriteData> sprites` | ✅ 完全对齐 | 存储结构等价（Rust 侧 Key 为大小写敏感，C# 为 `OrdinalIgnoreCase`） |
| `SpriteData` 类（Monocle 内置） | `struct SpriteData` | 🟠 部分实现 | Rust 侧是自定义数据结构，包含原版 SpriteData + Animation 的部分字段 |

---

## 方法/函数逐一对比

### 构造函数 `SpriteBank(Atlas, XmlDocument)`

| 方面 | C# | Rust | 状态 |
|------|----|----|------|
| 签名 | `SpriteBank(Atlas atlas, XmlDocument xml)` | `SpriteBank::from_xml(xml: &str) -> Result<SpriteBank>` | 🟡 近似 |
| 存储 Atlas | ✅ `this.Altas = atlas` | ❌ 无 Atlas 参数 | 🔴 缺失 |
| 存储 XML | ✅ `this.XML = xml` | ❌ 解析后丢弃 | 🔴 缺失 |
| 遍历 `<Sprites>` 子节点 | ✅ `XML["Sprites"].ChildNodes` | ✅ `root.children().filter(n.is_element())` | ✅ 完全对齐 |
| 重复名称检测 | ✅ `throw new Exception("Duplicate sprite name...")` | 🟡 覆盖写入（`HashMap::insert` 不报错） | 🟡 近似 |
| `copy` 属性处理 | ✅ `if (xmlElement.HasAttr("copy")) spriteData.Add(dictionary[xmlElement.Attr("copy")], ...)` | ❌ 未实现 | 🔴 缺失 |
| 创建 SpriteData 并调用 `.Add(xml)` | ✅ 解析 XML 元素填充 SpriteData | ✅ `parse_sprite(node)` | ✅ 完全对齐（逻辑等价） |

### 重载构造函数 `SpriteBank(Atlas, string xmlPath)`

| 方面 | C# | Rust | 状态 |
|------|----|----|------|
| 签名 | `SpriteBank(Atlas atlas, string xmlPath)` | `SpriteBank::load(path: &Path) -> Result<SpriteBank>` | 🟡 近似 |
| 从文件加载 | ✅ `Calc.LoadContentXML(xmlPath)` | ✅ `std::fs::read_to_string(path)` | ✅ 完全对齐 |

### 方法 `Has(string id)`

| 方面 | C# | Rust | 状态 |
|------|----|----|------|
| 签名 | `bool Has(string id)` | `SpriteBank::sprite(&self, name: &str) -> Option<&SpriteData>` | 🟠 部分实现 |
| 功能 | 返回 `bool` | 返回 `Option<&SpriteData>`（可间接判断） | 🟡 近似 |
| 调用方式 | `bank.Has("player")` | `bank.sprite("player").is_some()` | 语义等价 |

### 方法 `Create(string id)`

| 方面 | C# | Rust | 状态 |
|------|----|----|------|
| 签名 | `Sprite Create(string id)` | ❌ 无对应方法 | 🔴 缺失 |
| 说明 | 根据 id 从 SpriteData 创建并返回 `Sprite` 实例 | Rust 侧纯数据解析，不涉及运行时 Sprite 对象 | 架构差异 |

### 方法 `CreateOn(Sprite sprite, string id)`

| 方面 | C# | Rust | 状态 |
|------|----|----|------|
| 签名 | `Sprite CreateOn(Sprite sprite, string id)` | ❌ 无对应方法 | 🔴 缺失 |
| 说明 | 在已有 Sprite 上应用动画数据 | Rust 侧无此概念，需由 engine 层的 Sprite 组件实现 | 架构差异 |

---

## Rust 侧额外实现（C# 无对应）

| Rust 方法/结构 | 说明 | 状态 |
|----------------|------|------|
| `Animation` 结构体 | 统一建模 `Anim` + `Loop` 元素（`is_loop` 字段区分），含 `id`, `path`, `delay`, `frames`, `goto` | 🟠 部分实现 |
| `SpriteData::animation(&self, id)` | 按名称查找动画，返回 `Option<&Animation>` | 🟢 C# 侧由 SpriteData 内部处理 |
| `SpriteData::texture_prefix(&self, anim)` | 拼接 `path + anim.path` 作为图集路径 | 🟢 C# 侧在运行时计算 |
| `parse_frames(s)` | 解析 `"0,1,2-7"` 格式的帧索引 | 🟢 C# 侧在 SpriteData 内部处理 |
| `parse_i32` / `parse_f32` | 辅助解析函数 | 🟢 C# 使用内置方法 |

---

## SpriteData 字段对比

| C# SpriteData 字段 | Rust SpriteData 字段 | 状态 |
|--------------------|---------------------|------|
| （由 SpriteData.Add(xml) 解析填充，字段不可见于 SpriteBank.cs） | `name: String` | 🟢 Rust 显式暴露 |
| | `path: String` | 🟢 Rust 显式暴露 |
| | `start: String` | 🟢 Rust 显式暴露 |
| | `origin: (i32, i32)` | 🟢 Rust 显式暴露 |
| | `center: bool` | 🟢 Rust 显式暴露 |
| | `justify: Option<(f32, f32)>` | 🟢 Rust 显式暴露 |
| | `animations: HashMap<String, Animation>` | 🟢 Rust 显式暴露 |

---

## 总结

| 统计 | 数量 |
|------|------|
| ✅ 完全对齐 | 5 |
| 🟠 部分实现 | 3 |
| 🟡 近似 | 4 |
| 🔴 缺失 | 5 |

### 关键差异

1. **`copy` 属性未实现**：原版支持 `<SpriteName copy="OtherSprite" path="..."/>` 继承其他精灵的动画定义，Rust 侧完全忽略了此特性。
2. **重复名称检测缺失**：原版抛出异常，Rust 静默覆盖。
3. **大小写不敏感查找**：C# `Dictionary` 使用 `OrdinalIgnoreCase`，Rust `HashMap` 使用精确匹配。原版 XML 中 Sprite 名称通常大小写一致，实际影响较小，但需注意。
4. **Sprite 创建能力**：`Create()` / `CreateOn()` 在 Rust 侧缺失，这是架构性差异——Rust 的 ECS 体系中 Sprite 由 engine 层的 `SpriteComponent` 处理，而非从 SpriteBank 直接实例化。这是合理的分离，不算缺陷。
5. **Atlas 引用**：Rust 侧不持有 Atlas，路径前缀在解析阶段确定（`texture_prefix`），运行时按需查找图集。
