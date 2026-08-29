# COMPARE.md — 原版 Celeste C# 与 Ruleste(Rust) 逐文件/逐函数对照

> 审计基准：原版源码 `references/source/Celeste/`（Monocle 引擎 + Celeste 游戏逻辑）。
> 对照目标：本仓库 `src/`（核心/引擎/数据/界面）与 `plugins/*`（Wasm 实体插件）。
> 生成方式：按模块并行对原始 C# 类与 Rust 实现逐函数比对（见各节来源）。

## 图例
| 标记 | 含义 |
|---|---|
| ✅ | 已实现且行为对齐 |
| 🟠 | 部分实现（主干在，缺视觉/音效/粒子/旗标等） |
| 🟡 | 近似（数值或机制有偏差） |
| 🔴 | 缺失（无对应实现） |
| 🔴 占位 | 存在空 `update`/`draw` 或空壳 |

## 覆盖范围总览
| 区块 | 原始文件 | Rust 落点 | 文件 |
|---|---|---|---|
| 微内核/引擎 | Monocle/*（Entity/Scene/Camera/Draw/Input/Sprite/Atlas…） | src/engine/*, src/data/* | c1 |
| 数据解析 | BinaryPacker/AreaData/Dialog/Font/Audio/Reader | src/data/* | c2 |
| 玩家 | Player.cs（~6300 行） | plugins/player | c3 |
| 动平台/固体 | SwapBlock/FallingBlock/CrushBlock/ZipMover/DashBlock/Cloud/Bridge… | plugins/*（21 个） | c4 |
| 危险物 | Spinner/Spikes/TriggerSpikes/Killbox/Fire/Spring/Seeker… | plugins/*（19 个） | c5 |
| 道具/NPC/装饰 | Strawberry/Refill/NPC/Bonfire/Water/… | plugins/*（26 个） | c6 |
| 助推/羽毛/撞针 | BadelineBoost/FlyFeather/Bumper | plugins/badeline-boost, infinite-star, big-spinner | c7 |
| 关卡/背景/音频/界面 | Level/Backdrop/Autotiler/Audio/Celeste.Main/Oui* | src/engine/*, src/interface/*, src/main.rs | c8 |
| 补充实体 | DreamBlock/IntroCrusher/Plateau | plugins/dream-block, intro-crusher, plateau | c9 |

---


<!-- ========== SECTION c1 ========== -->

# Monocle 引擎核心 (Entity/Scene/Camera/Draw/Input/Sprites/Atlas)

> 比对说明：原版为 Celeste 的 C# `Monocle` 引擎；我方为 Ruleste 的 Rust 微内核 +
> Wasm 插件架构。Rust host（`src/engine`、`src/data`、`crates/ruleste-plugins-api`）
> 只承担「实体注册表 + 物理碰撞 + 相机 + 输入 + 精灵/图集/自动拼接解析」等基础设施；
> `Entity`/`Scene`/`Component`/`Renderer` 生命周期、粒子、补间、协程、状态机等
> **按设计由 Wasm 插件实现**，host 侧不建模这些类。因此大量方法标记为 `🔴 缺失`
> 是架构预期，而非遗漏。
>
> 图例：`✅ 已实现且对齐` / `🟠 部分实现` / `🟡 近似` / `🔴 缺失` / `🔴 占位(空实现)`

---

## Entity.cs ↔ src/engine/ecs.rs + crates/ruleste-plugins-api/src/host.rs

### Class Entity

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Entity(position) / Entity() | 410 / 416 | `World::spawn` (ecs.rs:92) | 🟡 近似 | 实体创建改为 `World::spawn`，无 Entity 类 |
| SceneBegin / SceneEnd / Awake / Added / Removed | 421 / 425 / 437 / 449 / 462 | — | 🔴 缺失 | 生命周期由插件 `ruleste_entity_init`/`destroy` 承担 |
| Update / Render | 474 / 479 | — | 🔴 缺失 | 由插件导出 `ruleste_entity_update`/`draw` 承担 |
| DebugRender / HandleGraphicsReset / HandleGraphicsCreate | 484 / 493 / 498 | — | 🔴 缺失 | host 无图形对象概念 |
| RemoveSelf | 503 | `host::remove` (host.rs:320) | 🟡 近似 | 仅移除实体，不触发组件回调 |
| TagFullCheck / TagCheck / AddTag / RemoveTag | 511 / 516 / 521 / 526 | — | 🔴 缺失 | host 无 BitTag/Tag 系统 |
| CollideCheck(Entity/at) | 531 / 536 | `host_collide_check` (host.rs:56→595) | 🟠 部分实现 | 仅做「地块/平台」碰撞，非实体对实体 |
| CollideCheck(BitTag/at) | 541 / 546 | — | 🔴 缺失 | 无 Tag 体系 |
| CollideCheckOutside | 627 / 636 | — | 🔴 缺失 | — |
| CollideFirst / CollideFirstOutside | 672 / 677 / 716 | — | 🔴 缺失 | 无 scene 级实体查询 |
| CollideAll | 752 / 757 | — | 🔴 缺失 | — |
| CollideDo | 800 / 814 | — | 🔴 缺失 | — |
| CollidePoint / CollideLine / CollideRect | 893 / 903 / 913 | `host_collide_check` (host.rs:595) | 🟠 部分实现 | 仅以 hitbox 偏移做地块碰撞 |
| Add / Remove(Component) | 923 / 928 | — | 🔴 缺失 | 组件状态在插件 Wasm 内存中 |
| Add / Remove(params Component[]) | 933 / 938 | — | 🔴 缺失 | — |
| GetEnumerator | 948 | — | 🔴 缺失 | — |
| Closest | 958 / 974 | `host::entities_by_type` (host.rs:345) | 🟡 近似 | 可按类型取实体，但无 Closest 距离计算 |

---

## Scene.cs ↔ src/engine/ecs.rs (World) + 主循环

### Class Scene

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Scene() | 34 | — | 🔴 缺失 | 无 Scene 类，主循环在 core |
| Begin / End / BeforeUpdate / Update / AfterUpdate / BeforeRender / Render / AfterRender | 45 / 54 / 63 / 75 / 84 / 93 / 98 / 103 | — | 🔴 缺失 | 由核心主循环代替（非 Scene 类） |
| HandleGraphicsReset / HandleGraphicsCreate / GainFocus / LoseFocus | 108 / 113 / 118 / 122 | — | 🔴 缺失 | — |
| OnInterval / BetweenInterval / OnRawInterval / BetweenRawInterval | 126 / 136 / 141 / 151 | — | 🔴 缺失 | 计时辅助；插件自行实现 |
| CollideCheck(Scene 级 point/line/rect) | 156 / 169 / 182 / 195 | — | 🔴 缺失 | 无 scene 级实体查询 |
| CollideFirst / CollideInto / CollideAll / CollideDo | 204 / 243 / 279 / 300 | — | 🔴 缺失 | — |
| LineWalkCheck | 336 | — | 🔴 缺失 | — |
| SetActualDepth | 810 | — | 🔴 缺失 | host 按 depth 字段排序渲染 |
| CreateAndAdd<T> | 832 | `World::spawn` (ecs.rs:92) | 🟡 近似 | 无泛型构造 |
| Add / Remove(Entity) | 839 / 844 | `World::spawn`/`despawn` (ecs.rs:92/99) | 🟡 近似 | — |
| Add / Remove(IEnumerable/params Entity[]) | 849 / 854 / 859 / 864 | — | 🔴 缺失 | — |
| GetEnumerator | 869 | `World::iter`/`entity_ids` (ecs.rs:111/119) | 🟡 近似 | — |
| GetEntitiesByTagMask / Excluding | 879 / 892 | — | 🔴 缺失 | 无 Tag |
| Add / Remove(Renderer) | 905 / 910 | — | 🔴 缺失 | 无 Renderer 列表 |

---

## Camera.cs ↔ src/engine/camera.rs

### Class Camera

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Camera() | 199 | `Camera::new` (camera.rs:27) | 🟡 近似 | — |
| Camera(w,h) | 207 | `Camera::new` (camera.rs:27) | 🟡 近似 | 视口固定 320×180，忽略入参 |
| ToString | 215 | — | 🔴 缺失 | — |
| UpdateMatrices | 220 | — | 🔴 缺失 | SDL 渲染器无矩阵概念 |
| CopyFrom | 227 | — | 🔴 缺失 | — |
| CenterOrigin | 236 | — | 🔴 缺失 | — |
| RoundPosition | 242 | — | 🔴 缺失 | — |
| ScreenToCamera / CameraToScreen | 249 / 254 | — | 🔴 缺失 | 由 SDL 逻辑呈现处理缩放 |
| Approach(pos, ease) / Approach(pos, ease, max) | 259 / 264 | `Camera::target_at`+`update` (camera.rs:38/56) | 🟡 近似 | 用 target+glide 近似，非原版 ease 插值 |
| Shake(intensity, duration) / Update(shake) | — | `Camera::shake`/`update` (camera.rs:105/118) | ✅ | 支持多段衰减随机偏移，强度/时长由宿主 `host_shake` 入队；`snap_to` 在换房时重置基础位 |

---

## Draw.cs ↔ src/engine/draw.rs (命令) + host.rs (FFI)

### Class Draw (调试/图元绘制)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Initialize / UseDebugPixelTexture | 21 / 28 | — | 🔴 缺失 | — |
| Point | 35 | `host_draw_line` (host.rs:64/191) | 🟠 部分实现 | 仅线/矩形/图元，无 Point |
| Line (全部重载) | 40 / 45 / 50 / 55 / 70 / 75 | `host::draw_line` (host.rs:191) | 🟡 近似 | 仅直线；无 thickness 参数 |
| LineAngle | 60 / 65 / 70 | — | 🔴 缺失 | — |
| Circle (全部重载) | 75 / 92 / 97 / 114 | `host::draw_circle` (host.rs) + `renderer::draw_circles` (renderer.rs) | ✅ | Bresenham 中点圆算法，像素级描边，支持颜色/alpha |
| Rect (全部重载) | 119 / 128 / 133 / 139 | `host::draw_rect` (host.rs:208) | 🟡 近似 | 仅实心矩形 |
| HollowRect (全部重载) | 144 / 161 / 166 / 171 | `host::draw_hollow_rect` (host.rs) + `renderer::draw_hollow_rects` (renderer.rs) | ✅ | 四条边线绘制空心矩形 |
| Text / TextJustified / TextCentered / OutlineText* | 176…302 | `host::draw_text` (host.rs) + `renderer::draw_texts` (renderer.rs) | ✅ | 三种对齐 + 可选 1px 描边，宿主拥有活动 SpriteFont；多行 \n 支持 |
| SineTextureH/V / TextureBannerV | 302 / 318 / 334 | — | 🔴 缺失 | — |

---

## MInput.cs ↔ src/engine/input.rs

### Class MInput (输入设备)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| KeyboardData() / Update / UpdateNull | 16 / 20 / 26 | `Input::pump` (input.rs:89) | 🟡 近似 | 仅键盘；无 Null 设备 |
| KeyboardData.Check/Pressed/Released (单/双/三键) | 38 / 51 / 64 / 77 / 86 / 95 / 104 / 113 / 122 | `Input::button`/`pressed`/`released` (input.rs:136/144/154) | 🟡 近似 | 按 action 索引，无独立键枚举 |
| KeyboardData.AxisCheck | 131 / 148 | `Input::axis` (input.rs:126) | ✅ 近似 | 轴输入已对齐 |
| MouseData.Check/Pressed/Released | 303 / 309 / 315 | `Mouse::position`/`left_pressed` (host.rs) + `Input::mouse` (input.rs:46/124) | ✅ | 鼠标坐标（窗口像素→逻辑→世界空间）+ 左键按下/释放边沿；GameState.pixel_scale 由主循环同步，camera 偏移在 FFI 层应用 |
| GamePadData (全部) | 476…1359 | — | 🔴 缺失 | host 暂不支持手柄 |
| Initialize / Shutdown / Update / UpdateNull | 1359 / 1371 / 1380 / 1426 | `Input::pump` (input.rs:89) | 🟡 近似 | — |
| UpdateVirtualInputs | 1437 | — | 🔴 缺失 | 无虚拟输入注册表 |
| RumbleFirst | 1445 | — | 🔴 缺失 | — |
| Axis(...) 重载 | 1450 / 1467 / 1476 | `Input::axis` (input.rs:126) | 🟡 近似 | — |

---

## VirtualInput.cs ↔ src/engine/input.rs

### Class VirtualInput / VirtualInputNode

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Deregister | 24 | — | 🔴 缺失 | 无虚拟输入注册表 |
| Update (抽象) | 29 | `Input::pump` (input.rs:89) | 🟡 近似 | — |
| VirtualInputNode.Update | 5 | — | 🔴 缺失 | 抽象节点未建模 |

---

## VirtualButton.cs ↔ src/engine/input.rs (Binding + Input)

### Class VirtualButton

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| VirtualButton(...) 构造 | 79 / 87 | `Binding::add` (input.rs:17) + `Input` | 🟡 近似 | 键盘绑定 + buffer_time |
| SetRepeat | 91 / 96 | — | 🔴 缺失 | 无重复触发 |
| Update | 107 | `Input::pump` (input.rs:89) | 🟡 近似 | pressed/buffered 在 pump 中计算 |
| ConsumeBuffer | 148 | `Input::consume` (input.rs:163 / host.rs:576) | ✅ 近似 | 清空缓冲 |
| ConsumePress | 153 | — | 🔴 缺失 | — |
| implicit operator bool | 159 | `Input::button` (input.rs:136) | 🟡 近似 | — |

---

## VirtualIntegerAxis.cs ↔ src/engine/input.rs

### Class VirtualIntegerAxis

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (3 个) | 27 / 31 / 40 | `Input::axis` (input.rs:126) | 🟡 近似 | 轴输入近似 |
| Update | 51 | `Input::pump` (input.rs:89) | 🟡 近似 | — |
| implicit operator float | 100 | `Input::axis` (input.rs:126) | 🟡 近似 | — |

---

## VirtualJoystick.cs ↔ src/engine/input.rs

### Class VirtualJoystick

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (2 个) | 45 / 56 | `Input::axis` (input.rs:126) | 🟡 近似 | 四向绑定→轴；无手柄 stick |
| Update | 71 | `Input::pump` (input.rs:89) | 🟡 近似 | — |
| implicit operator Vector2 | 235 | `Input::axis` (input.rs:126) | 🟡 近似 | 组合两轴得到方向向量 |

---

## Sprite.cs ↔ src/engine/sprites.rs + host.rs (Sprite)

### Class Sprite

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Sprite(Atlas, path) | 76 | `SpriteBank` + `SpriteAnimator` | 🟡 近似 | 无 Sprite 对象，状态存 World.sprite |
| Reset | 85 | — | 🔴 缺失 | — |
| GetFrame | 99 | — | 🔴 缺失 | — |
| Update | 104 | `SpriteAnimator::update` (sprites.rs:27) | 🟠 部分实现 | 仅推进帧/loop/goto，无协程动画 |
| SetFrame | 209 | `host Sprite::set_frame` (host.rs:492) | ✅ 近似 | — |
| AddLoop / Add (全部重载) | 216…316 | — | 🔴 缺失 | 动画在 SpriteBank.xml 声明，非运行时加 |
| ClearAnimations | 353 | — | 🔴 缺失 | — |
| Play / PlayOffset | 358 / 384 | `host Sprite::play` (host.rs:462) | ✅ 近似 | — |
| PlayRoutine / ReverseRoutine / PlayUtil | 420 / 426 / 432 | — | 🔴 缺失 | 协程动画未实现 |
| Reverse | 440 | — | 🔴 缺失 | — |
| Has | 449 | `SpriteBank::sprite` (spritebank.rs:76) | 🟡 近似 | — |
| Stop | 458 | — | 🔴 缺失 | — |
| CreateClone / CloneInto | 465 / 470 | — | 🔴 缺失 | — |
| DrawSubrect | 493 | `host::draw_image` (host.rs:226) | 🟡 近似 | — |
| LogAnimations | 503 | — | 🔴 缺失 | — |

---

## MTexture.cs ↔ src/data/atlas.rs + host.rs (draw_image)

### Class MTexture

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (全部) | 33 / 37 / 48 / 59 / 64 / 75 / 81 | — | 🔴 缺失 | 无 MTexture 类，图帧以 string id 表示 |
| Unload | 100 | — | 🔴 缺失 | — |
| GetSubtexture | 106 / 122 | `SpriteAnimator::subtexture_key` (sprites.rs:117) | 🟡 近似 | 按前缀枚举子图 |
| ToString | 127 | — | 🔴 缺失 | — |
| GetRelativeRect | 140 / 145 | — | 🔴 缺失 | — |
| Draw / DrawCentered / DrawJustified (全部) | 156…281 | `host::draw_image` (host.rs:226) | 🟡 近似 | 仅中心绘制+翻转+缩放 |
| DrawOutline / DrawOutlineCentered / DrawOutlineJustified (全部) | 286…646 | — | 🔴 缺失 | 无描边绘制 |

---

## Image.cs ↔ src/engine/draw.rs (Image) + host.rs

### Class Image

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Image(MTexture) | 15 | `draw::Image` (draw.rs:32) | 🟡 近似 | 图元命令结构 |
| Image(MTexture, active) | 21 | — | 🔴 缺失 | — |
| Render | 27 | — | 🔴 缺失 | 渲染由 host 收集命令完成 |
| SetOrigin / CenterOrigin | 35 / 42 | — | 🔴 缺失 | 原点由插件算世界坐标 |
| JustifyOrigin | 49 / 56 | — | 🔴 缺失 | — |
| SetColor | 63 | `host::draw_image_color` (host.rs:263) | 🟡 近似 | — |

---

## SpriteBank.cs ↔ src/data/spritebank.rs

### Class SpriteBank

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| SpriteBank(Atlas, xml) / (Atlas, xmlPath) | 15 / 42 | `SpriteBank::from_xml`/`load` (spritebank.rs:56/69) | ✅ 近似 | 从 Sprites.xml 解析 |
| Has | 47 | `SpriteBank::sprite` (spritebank.rs:76) | ✅ 近似 | — |
| Create / CreateOn | 52 / 61 | — | 🟠 部分实现 | 动画播放经 `SpriteAnimator`+`host Sprite::play`；无运行时 Sprite 对象 |

---

## SpriteData.cs ↔ src/data/spritebank.rs (SpriteData/Animation)

### Class SpriteData

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| SpriteData(Atlas) | 16 | `parse_sprite` (spritebank.rs:81) | ✅ 近似 | — |
| Add | 22 | `parse_sprite` (spritebank.rs:81) | ✅ 近似 | 解析 Anim/Loop |
| HasFrames | 94 | — | 🔴 缺失 | 无运行时帧校验 |
| CheckAnimXML | 110 | — | 🔴 缺失 | — |
| Create / CreateOn | 123 / 128 | — | 🟠 部分实现 | 见 SpriteBank.Create |

---

## Atlas.cs ↔ src/data/atlas.rs

### Class Atlas

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| FromAtlas | 42 | `Atlas::load` (atlas.rs:200) | ✅ 近似 | Packer 格式 |
| FromMultiAtlas | 256 / 267 | `Atlas::merge` (atlas.rs:261) | 🟡 近似 | 由 load_atlas_dir 合并 |
| FromDirectory | 285 | `load_atlas_dir` (atlas.rs:331) | ✅ 近似 | 目录扫描合并 |
| Has | 310 | `Atlas::frame_clip` (atlas.rs:290) | ✅ 近似 | frame_index 查询 |
| GetOrDefault | 315 | `Atlas::frame_rgba_into`/`frame_clip` | ✅ 近似 | — |
| GetAtlasSubtextures | 324 | `SpriteAnimator::subtexture_count` (sprites.rs:109) | 🟡 近似 | 枚举子图 |
| GetAtlasSubtextureFromCacheAt / FromAtlasAt | 345 / 350 | `Atlas::frame_index` (atlas.rs:194) | 🟡 近似 | — |
| GetAtlasSubtexturesAt | 369 | `SpriteAnimator::subtexture_key` (sprites.rs:117) | 🟡 近似 | — |
| GetLinkedTexture | 378 | — | 🔴 缺失 | 未处理 LINKS |
| Dispose | 387 | — | 🔴 缺失 | Rust 自动释放 |
| ReadAtlasData (私有) | 52 | `AtlasMeta::from_bytes`+`AtlasPage::decode` (atlas.rs:70/131) | ✅ 近似 | RLE 解码对齐 |

---

## Tiler.cs ↔ src/engine/autotiler.rs

### Class Tiler (Autotile)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Tile(bool[,], Func, Action, w, h, edges) | 34 | `Autotiler::generate` (autotiler.rs:296) | 🟡 近似 | 3×3 邻接+mask 算法重实现 |
| Tile(bool[,], mask, ...) | 87 | `Autotiler::generate` + `mask_matches` (autotiler.rs:296/268) | 🟡 近似 | — |
| Tile(bool[,], AutotileData, ...) | 140 / 145 | `Autotiler::parse`+`generate` (autotiler.rs:66/296) | 🟡 近似 | 用 ForegroundTiles.xml 代替 AutotileData |

---

## ParticleSystem.cs ↔ (无) 插件侧

### Class ParticleSystem

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Clear / ClearRect / Update / Render | 12 / 18 / 26 / 38 / 49 | — | 🔴 缺失 | 粒子系统归插件实现 |
| Simulate / Add / Emit (全部重载) | 75 / 94 / 100…156 | — | 🔴 缺失 | — |

## ParticleType.cs ↔ (无) 插件侧

### Class ParticleType

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Create (全部重载) | 79 / 98 / 126…146 | — | 🔴 缺失 | — |

## Particle.cs ↔ (无) 插件侧

### Class Particle

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| SimulateFor / Update / Render (重载) | 39 / 58 / 121 / 131 | — | 🔴 缺失 | — |

---

## Ease.cs ↔ (无) 插件侧

### Class Ease

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Linear / Sine* / Quad* / Cube* / Quint* / Expo* / Back* / Elastic* / Bounce* | 9…108 | — | 🔴 缺失 | host 无 Ease；补间由插件实现 |
| Invert / Follow / UpDown | 139 / 144 / 149 | — | 🔴 缺失 | — |

---

## Wiggler.cs ↔ (无) 插件侧

### Class Wiggler

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Create / Init / Removed / Start / Stop / StopAndClear / Update | 28 / 35 / 40 / 58 / 64 / 88 / 95 / 100 / 106 | — | 🔴 缺失 | 组件归插件 |

## SineWave.cs ↔ (无) 插件侧

### Class SineWave

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Update / ValueOffset / Randomize / Reset / StartUp / StartDown | 38 / 43 / 50 / 59 / 64 / 70 / 75 / 80 | — | 🔴 缺失 | 组件归插件 |

## StateMachine.cs ↔ (无) 插件侧

### Class StateMachine

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Added / EntityAdded / ForceState / SetCallbacks / ReflectState / Update / Log* | 78 / 90 / 99 / 108 / 152 / 160 / 168 / 190 / 198 | — | 🔴 缺失 | 状态机归插件 |

## Coroutine.cs ↔ (无) 插件侧

### Class Coroutine

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Update / Cancel / Replace | 20 / 28 / 35 / 84 / 93 | — | 🔴 缺失 | 协程归插件（Wasm 原生迭代器） |

## Tween.cs ↔ (无) 插件侧

### Class Tween

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Create / Set / Position / Init / Removed / Update / Start / Stop / Reset / Wait | 49 / 70 / 79 / 91 / 96 / 115 / 122 / 181 / 186 / 200 / 206 / 211 / 218 | — | 🔴 缺失 | 补间归插件 |

## Alarm.cs ↔ (无) 插件侧

### Class Alarm

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Create / Set / Init / Update / Removed / Start / Stop | 25 / 32 / 39 / 44 / 57 / 82 / 88 / 94 / 100 | — | 🔴 缺失 | 计时器归插件 |

---

## Renderer.cs ↔ (无) 插件侧

### Class Renderer

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Update / BeforeRender / Render / AfterRender | 7 / 11 / 15 / 19 | — | 🔴 缺失 | 渲染列表由 host 内部固定管线承担 |

## RendererList.cs ↔ (无) 插件侧

### Class RendererList

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / UpdateLists / Update / BeforeRender / Render / AfterRender / MoveToFront / Add / Remove | 15 / 23 / 43 / 51 / 63 / 75 / 87 / 93 / 98 | — | 🔴 缺失 | 无 Renderer 列表 |

## Component.cs ↔ (无) 插件侧

### Class Component

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| （无 public 方法，仅生命周期虚方法） | — | — | 🔴 缺失 | 组件状态存插件 Wasm 内存 |

## ComponentList.cs ↔ (无) 插件侧

### Class ComponentList

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / Add / Remove / GetEnumerator / ToArray / Update / Render / DebugRender / HandleGraphics* | 88 / 99 / 123 / 147 / 168 / 176 / 184 / 194 / 199 / 212 / 225 / 235 / 245 | — | 🔴 缺失 | 组件列表归插件 |

## EntityList.cs ↔ src/engine/ecs.rs (World)

### Class EntityList

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / MarkUnsorted / UpdateLists | 44 / 56 / 61 | — | 🔴 缺失 | — |
| Add / Remove (Entity/IEnumerable/params) | 125 / 134 / 143 / 151 / 159 / 167 | `World::spawn`/`despawn` (ecs.rs:92/99) | 🟡 近似 | 无批量/标签 |
| GetEnumerator / ToArray | 224 / 234 | `World::iter`/`entity_ids` (ecs.rs:111/119) | 🟡 近似 | — |
| HasVisibleEntities | 239 | `Entity::visible` (ecs.rs) | 🟡 近似 | 由渲染阶段跳过 invisible |
| Update / Render / RenderOnly / RenderExcept / DebugRender | 251 / 262 / 273 / 284 / 295 / 306 | — | 🔴 缺失 | 主循环直接遍历 World |
| HandleGraphicsReset / HandleGraphicsCreate | 314 / 322 | — | 🔴 缺失 | — |

## Tracker.cs ↔ (无) 插件侧

### Class Tracker

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Initialize / GetSubclasses / 构造 / EntityAdded / EntityRemoved / ComponentAdded / ComponentRemoved / Log* | 22 / 97 / 111 / 235 / 248 / 261 / 274 / 287 / 296 | `host::entities_by_type` (host.rs:345) | 🟡 近似 | 按 entity_type 字符串查询代替类型追踪 |

## TagLists.cs ↔ (无) 插件侧

### Class TagLists

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / MarkUnsorted / UpdateLists / EntityAdded / EntityRemoved | 15 / 25 / 31 / 48 / 61 | — | 🔴 缺失 | host 无 Tag 系统 |

---

## Collide.cs ↔ src/engine/physics.rs

### Class Collide (静态几何辅助)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Check(Entity, Entity/at) | 8 / 21 | `physics::collide_rect` (physics.rs:294) | 🟠 部分实现 | 仅 hitbox 对地块；无实体对实体 |
| Check(Entity, IEnumerable/at) | 30 / 42 | — | 🔴 缺失 | — |
| First / All | 51 / 63 / 72 / 84 / 93 / 98 | — | 🔴 缺失 | — |
| CheckPoint | 103 / 112 | `physics::collide_rect` (physics.rs:294) | 🟡 近似 | — |
| CheckLine | 121 / 130 | `physics::line_*` (physics.rs) | 🟡 近似 | 见 LineCheck |
| CheckRect | 139 / 148 | `physics::collide_rect` (physics.rs:294) | ✅ 近似 | — |
| LineCheck | 157 / 180 | `physics::segment_*` (physics.rs) | 🟡 近似 | 线段相交 |
| CircleToLine / CircleToPoint / CircleToRect | 205 / 210 / 215 / 220 | `physics::circle_to_rect` (physics.rs:348) + `circle_to_circle` (physics.rs:362) | ✅ | 圆形→AABB/圆 |
| RectToCircle | 225 / 271 | `physics::circle_to_rect` (physics.rs:348) | 🟠 近似 | 需交换参数 |
| CircleToCircle | — | `physics::circle_to_circle` (physics.rs:362) | ✅ | — |
| RectToLine | 276 / 328 | `physics::segment_*` (physics.rs) | 🟡 近似 | — |
| RectToPoint | 333 / 342 | `physics::collide_rect` (physics.rs:294) | ✅ 近似 | — |
| GetSector | 347 / 369 | — | 🔴 缺失 | — |

---

## Grid.cs ↔ src/engine/physics.rs (SolidGrid)

### Class Grid (碰撞体)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (cells/bitstring/bool[,]/VirtualMap) | 124 / 131 / 155 / 162 | `SolidGrid::empty`/`from_rows` (physics.rs:43/55) | 🟡 近似 | 由字符行构造；无 VirtualMap 构造 |
| Extend | 169 | — | 🔴 缺失 | — |
| LoadBitstring / GetBitstring | 223 / 258 | `SolidGrid::from_rows` (physics.rs:55) | 🟡 近似 | — |
| Clear / SetRect / CheckRect / CheckColumn / CheckRow | 275 / 286 / 315 / 348 / 360 | `SolidGrid::tile_id_at`/`solid_at` (physics.rs:262/252) | 🟡 近似 | 布尔查询近似 |
| Clone / Render | 372 / 377 | — | 🔴 缺失 | — |
| Collide(Vector2/Rectangle/from-to/Hitbox/Grid/Circle/ColliderList) | 409…496 | `physics::collide_rect`/`actor_move`/`collide_circle` (physics.rs:294/319/321) | 🟡 近似 | 地块碰撞；圆形通过 FFI `host_collide_circle_check` 调用 `collide_circle` |

---

## Hitbox.cs ↔ crates/ruleste-plugins-api/src/host.rs (Hitbox) + ecs.rs

### Class Hitbox

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Hitbox(w,h,x,y) | 83 | `host Hitbox::set` (host.rs:643) | 🟡 近似 | 经 FFI 写入 World hitbox |
| Intersects(Hitbox/float*) | 91 / 100 | `physics::collide_rect` (physics.rs:294) | 🟡 近似 | — |
| Clone / Render | 109 / 114 | — | 🔴 缺失 | — |
| SetFromRectangle / Set | 119 / 126 | `host Hitbox::set`/`get` (host.rs:643/651) | ✅ 近似 | — |
| GetTopEdge / Bottom / Left / Right | 133 / 140 / 147 / 154 | — | 🔴 缺失 | — |
| Collide(Point/Rect/from-to/Hitbox/Grid/Circle/ColliderList) | 161…195 | `host_collide_check` + `host_collide_circle_check` (host.rs:595/600) | 🟡 近似 | 矩形与圆形地块碰撞均已 FFI 暴露 |

---

## Collider.cs ↔ host.rs (Collision) + ecs.rs

### Class Collider

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| Added / Removed | 255 / 260 | — | 🔴 缺失 | 组件生命周期归插件 |
| Collide(Entity/Collider) | 265 / 270 | `host_collide_check` (host.rs:595) | 🟠 部分实现 | 仅地块 |
| CenterOrigin | 309 | — | 🔴 缺失 | — |
| Render | 315 | — | 🔴 缺失 | — |

---

## TileGrid.cs ↔ src/engine/autotiler.rs (TileGrid) + draw.rs (TileBox)

### Class TileGrid

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 | 28 | `Autotiler::TileGrid` (autotiler.rs:43) | 🟡 近似 | — |
| Populate / Overlay | 36 / 47 | `Autotiler::generate` (autotiler.rs:296) | 🟡 近似 | 由 SolidGrid 生成 |
| Extend | 61 | — | 🔴 缺失 | — |
| FillRect / Clear | 115 / 130 | — | 🔴 缺失 | — |
| GetClippedRenderTiles | 141 | — | 🔴 缺失 | — |
| Render / RenderAt | 170 / 175 | `host::draw_tile_box` (host.rs:287) + `draw::TileBox` | 🟡 近似 | 自动拼接盒绘制 |

---

## VirtualMap.cs ↔ (无) src/engine/physics.rs (SolidGrid 连续)

### Class VirtualMap<T>

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 / AnyInSegment / InSegment / GetSegment / SafeCheck / ToArray / Clone | 54 / 64 / 76 / 83 / 88 / 93 / 98 / 107 / 120 | — | 🔴 缺失 | Rust 用连续 `Vec<char>` 的 SolidGrid，无分段虚拟地图 |

---

## Calc.cs ↔ (无集中实现；工具散落于插件侧)

### Class Calc (静态工具)

| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| EnumLength / 字符串扩展 (StartsWith 等) | 61 / 90 / 95 / 100 | — | 🔴 缺失 | — |
| ToString / SplitLines / Count / GiveMe | 116 / 126 / 153 / 263 | — | 🔴 缺失 | — |
| PushRandom / PopRandom / Choose / Range / Facing / Chance / NextFloat / NextAngle / ShakeVector | 323…425 | — | 🔴 缺失 | 随机工具归插件 |
| ClosestTo / Shuffle* | 430 / 480 / 492…532 | — | 🔴 缺失 | — |
| Invert / HexToColor / HsvToColor / ToggleColors | 537 / 542 / 563 / 573 / 909 | — | 🔴 缺失 | 颜色工具归插件 |
| Short/LongGameplayFormat / Digits / HexToByte | 602 / 611 / 628 / 640 | — | 🔴 缺失 | — |
| Percent / SignThreshold / Min / Max / Axis / Clamp | 645 / 650 / 659 / 669 / 694 / 711 / 716 | `Camera`/插件内联 | 🟡 近似 | 通用数学在插件侧实现 |
| YoYo / Map / SineMap / ClampedMap / LerpSnap / LerpClamp | 721 / 730 / 735 / 740 / 745 / 755 | — | 🔴 缺失 | — |
| Sign / SafeNormalize / TurnRight / ReflectAngle / ClosestPointOnLine / Round / Snap / WrapAngle* | 770…848 | — | 🔴 缺失 | 向量工具归插件 |
| AngleToVector / AngleApproach / AngleLerp / Approach / AngleDiff / AbsAngleDiff / Angle | 853 / 858 / 868 / 873 / 882 / 894 / 899 / 904 | `Camera::update` 内联 (camera.rs:56) | 🟡 近似 | Approach/AngleApproach 在相机平滑内联 |

---

## 小结

- **✅ 已实现且对齐 / 近似**：图集与 SpriteBank 解析（`atlas.rs`/`spritebank.rs`）、自动拼接 3×3 算法（`autotiler.rs`）、实体注册表（`ecs.rs`）、物理碰撞与 `actor_move`/`is_grounded`（`physics.rs`）、相机目标跟随与屏幕震屏（`camera.rs`）、键盘虚拟输入与缓冲（`input.rs`）、精灵帧推进（`sprites.rs`）、插件 FFI 门面（`host.rs`）、几何/文本/粒子/震屏 FFI。
- **🔴 缺失（按架构预期）**：`Entity`/`Scene`/`Component`/`Renderer` 类及其生命周期、`Tween`/`Alarm`/`Coroutine`/`StateMachine`/`Wiggler`/`SineWave`、`Tracker`/`TagLists`、`MInput` 的手柄——均由 Wasm 插件或核心主循环承担，host 不建模这些类。
- **🟠 已落基础（host 侧）**：`Ease`（全套缓动函数，`crates/ruleste-plugins-api/src/ease.rs`）、`Particle*`（宿主粒子池，`src/engine/particles.rs` + `host_emit_particle` FFI；`ParticleType` 预设式发射缺失）、`Draw` 圆/空心矩形/文本（`host_draw_*` FFI + `renderer::draw_*`）、`Collide` 圆形碰撞（`physics::collide_circle`/`circle_to_rect`/`circle_to_circle` + `host_collide_circle_check` FFI）。

<!-- ========== SECTION c2 ========== -->

# 数据/资源解析层 (BinaryPacker/AreaData/Dialog/Font/Audio/Reader)

> 状态标记：✅ 已实现且对齐 / 🟠 部分实现 / 🟡 近似 / 🔴 缺失 / 🔴 占位（仅骨架/占位）。
> 说明聚焦解析/加载逻辑函数。行号指向所给原版文件与我方 `src/data/` 实现。

---

## `BinaryPacker.cs` ↔ `src/data/binary_packer.rs`
### class `BinaryPacker`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Element.HasAttr(name)` | BinaryPacker.cs:21 | binary_packer.rs:87 (`Element::attr` 返回 Option) | ✅ | 用 `attr()` 返回 `Option` 取代，语义等价 |
| `Element.Attr(name, default)` | BinaryPacker.cs:30 | binary_packer.rs:102 (`attr_str`) | ✅ | 字符串取值对齐 |
| `Element.AttrBool(name, default)` | BinaryPacker.cs:39 | binary_packer.rs:97 (`attr_bool`) | ✅ | 类型回退解析对齐 |
| `Element.AttrFloat(name, default)` | BinaryPacker.cs:52 | binary_packer.rs:92 (`attr_f32`) | ✅ | 用 `as_f32` 回退解析 |
| `ToBinary(filename/outdir)` | BinaryPacker.cs:78 | — | 🔴 | 仅读取方向实现，无写出 |
| `ToBinary(XmlElement, out)` | BinaryPacker.cs:100 | — | 🔴 | 无 XML→bin 编码 |
| `CreateLookupTable` / `AddLookupValue` | BinaryPacker.cs:119/142 | — | 🔴 | 编码期构造字符串表，未实现 |
| `WriteElement` | BinaryPacker.cs:151 | — | 🔴 | 编码未实现 |
| `ParseValue(value, out type, out result)` | BinaryPacker.cs:231 | binary_packer.rs:161-180 (`read_element` 内 `match ty`) | ✅ | 类型化读取对齐（含 type 7 RLE） |
| `FromBinary(filename)` | BinaryPacker.cs:270 | binary_packer.rs:124 (`MapBin::from_bytes`) / 140 (`from_file`) | ✅ | magic/`package`/字符串表/根元素读取对齐 |
| `ReadElement(reader)` | BinaryPacker.cs:287 | binary_packer.rs:147 (`read_element`) | ✅ | 递归结构、属性表、children 对齐；type 1/2 用 `Convert.ToInt32` 我方直接存 Byte/Short（见下） |
| 字符串表/类型标签 0-7 | BinaryPacker.cs:184-329 | binary_packer.rs:22 (`AttrType`) / 33 (`Attr`) | ✅ | 类型枚举与变体对齐 |

### class `RunLengthEncoding`（solids/bg 网格，type 7）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Encode(str)` | RunLengthEncoding.cs:8 | — | 🔴 | 无编码 |
| `Decode(bytes)` | RunLengthEncoding.cs:29 | binary_packer.rs:197 (`rle_decode`) | ✅ | 每 2 字节 (count, char) 展开，与原版一致 |

> 注：原版 `ReadElement` 对 type 1/2/3 用 `Convert.ToInt32` 统一成 int 存入 `object`，我方保留 `Byte`/`Short`/`Int` 区分，取值时通过 `as_*` 回退，行为等价但类型更精确。

---

## `AreaData.cs` ↔ `src/data/pack.rs`（章节元数据近似）
### class `AreaData`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Areas` 静态表 + `Load()` | AreaData.cs:94 | pack.rs:77 (`PackMeta::load`) / 102 (`scan_all`) | 🟠 | 原版硬编码 11 个 area（含 Mode/Checkpoints/AudioState 等），我方改为 `metadata.json` 驱动的 `PackMeta`/`ChapterMeta`，概念类似但数据来源与字段大不同 |
| `GetMode(area)` / `GetMode(id, mode)` | AreaData.cs:84/89 | — | 🔴 | 无等价（未实现 area/mode 索引） |
| `ReloadMountainViews()` | AreaData.cs:783 | — | 🔴 | 雪山相机视图 XML 未实现 |
| `IsPoemRemix(id)` | AreaData.cs:808 | — | 🔴 | 缺失 |
| `GetCheckpointID/GetCheckpoint/GetCheckpointName` | AreaData.cs:820/836/853 | — | 🔴 | 检查点查询缺失 |
| `GetCheckpointInventory/Dreaming/CoreMode/AudioState/ColorGrading` | AreaData.cs:867-900 | — | 🔴 | 检查点派生状态全部缺失 |
| `Unload()` | AreaData.cs:902 | — | 🔴 | 无等价 |
| `Get(scene/session/area/id)` | AreaData.cs:907-933 | — | 🔴 | 缺失 |
| `DoScreenWipe` / `HasMode` | AreaData.cs:935/947 | — | 🔴 | 转场/模式判定缺失 |

> `pack.rs` 仅覆盖“章节列表/地图路径/歌曲名”这类装载元数据，对应原版 `AreaData` 的极小子集；原版大量运行时查询与硬编码关卡表无 Rust 等价。

---

## `AreaKey.cs` ↔ （无 Rust 文件）
### struct `AreaKey`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `None`/`Default` 静态 | AreaKey.cs:9-10 | 🔴 | 缺失 | 无 `AreaKey` 类型 |
| `ChapterIndex` getter | AreaKey.cs:19 | 🔴 | 缺失 | |
| ctor `(id, mode)` | AreaKey.cs:39 | 🔴 | 缺失 | |
| `==` / `!=` 运算符 | AreaKey.cs:45/54 | 🔴 | 缺失 | |
| `Equals` / `GetHashCode` | AreaKey.cs:63/68 | 🔴 | 缺失 | |
| `ToString()` | AreaKey.cs:73 | 🔴 | 缺失 | |

## `AreaMode.cs` ↔ （无 Rust 文件）
### enum `AreaMode`
| 原版 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `Normal, BSide, CSide` | AreaMode.cs:3 | 🔴 | 缺失 | 无对应枚举（章节侧用字符串 id） |

## `AreaStats.cs` ↔ （无 Rust 文件）
### class `AreaStats`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `TotalStrawberries/TotalDeaths/TotalTimePlayed` | AreaStats.cs:18/31/44 | 🔴 | 缺失 | 存档统计聚合无实现 |
| `BestTotalDeaths/BestTotalDashes/BestTotalTime` | AreaStats.cs:57/70/83 | 🔴 | 缺失 | |
| ctor `(id)` / 私有 ctor | AreaStats.cs:96/107 | 🔴 | 缺失 | |
| `Clone()` | AreaStats.cs:117 | 🔴 | 缺失 | |
| `CleanCheckpoints()` | AreaStats.cs:131 | 🔴 | 缺失 | |

---

## `Dialog.cs` ↔ `src/data/dialog.rs`
### class `Dialog`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Load()` | Dialog.cs:23 | dialog.rs:49 (`DialogData::load`) | 🟠 | 我方按单文件解析；原版扫描 `Dialog/*.txt` 多语言并按 Settings 选语言、排序，未实现多语言集合 |
| `LoadLanguage(filename)` | Dialog.cs:57 | dialog.rs:49 | 🟠 | 仅 `FromTxt` 近似；无 `.export` 二进制分支 |
| `Unload()` | Dialog.cs:68 | — | 🔴 | 无 |
| `Has(name, lang)` | Dialog.cs:80 | dialog.rs:156 (`get`) 返回 Option | 🟠 | 用 `get().is_some()` 替代，但无语言维度 |
| `Get(name, lang)` | Dialog.cs:89 | dialog.rs:156 (`get`) | 🟠 | 缺失回退 `"XXX"` 与多语言；返回 `Option` |
| `Clean(name, lang)` | Dialog.cs:103 | — | 🟠 | 我方未区分 `Cleaned`（干净文本），`DialogRenderer` 仅剥离命令未保留原 `Cleaned` 映射 |
| `Time(ticks)` | Dialog.cs:117 | — | 🔴 | 时间格式化缺失 |
| `FileTime(ticks)` | Dialog.cs:127 | — | 🔴 | 缺失 |
| `Deaths(deaths)` | Dialog.cs:137 | — | 🔴 | 缺失 |
| `CheckCharacters()` | Dialog.cs:150 | — | 🔴 | 调试工具缺失 |
| `CheckLanguageFontCharacters(a)` | Dialog.cs:210 | — | 🔴 | 缺失 |
| `CompareLanguages(a,b,cmp)` | Dialog.cs:246 | — | 🔴 | 缺失 |

### class `Language`（Dialog 的解析核心，同文件组）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `FromTxt(path)` | Language.cs:119 | dialog.rs:57 (`parse`) | 🟠 | 我方用 `looks_like_dialog_key` 启发式区分元数据/对话键；未实现 `LANGUAGE/FONT/ORDER/ICON/SPLIT_REGEX` 元数据语义、portrait `[...]`→`{portrait ...}` 替换、`{\+ ...}` 插入展开、`{n}/{break}` 命令清洗生成 `Cleaned` |
| `FromExport(path)` | Language.cs:93 | — | 🔴 | 二进制导出格式未实现 |
| `Export(path)` | Language.cs:70 | — | 🔴 | 未实现 |
| `CanDisplay(text)` | Language.cs:57 | — | 🔴 | 字体字符覆盖检查缺失 |

---

## `ActiveFont.cs` ↔ `src/data/font.rs`（近似，格式不同）
### class `ActiveFont`（字体度量/绘制入口）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Font` / `FontSize` / `BaseSize` / `LineHeight` | ActiveFont.cs:8-14 | font.rs:20 (`PixelFontSize.line_height`) | 🟡 | 原版 `ActiveFont` 持有 `PixelFont`，`Draw` 调 `PixelFont.Get(baseSize).Draw`；我方 `draw_pixel_texts` 做同件事 |
| `Measure(char/string)` | ActiveFont.cs:16/21 | font.rs:41 (`measure`) | ✅ | `PixelFontSize::measure` 返回 `(w, h)`，`PixelFont::get` 做字号选择 |
| `WidthToNextLine` | ActiveFont.cs:26 | font.rs:55 (`width_to_next_line`) | ✅ | 对齐 |
| `HeightOf` | ActiveFont.cs:31 | font.rs:65 (`height_of`) | ✅ | 对齐 |
| `Draw(...)` 各重载 | ActiveFont.cs:36-64 | renderer.rs:600 (`draw_pixel_texts`) + :673 (`draw_pixel_line`) | 🟠 | 含 outline stroke（1px filled rect）；缺 edge outline（4px box）和 scale 缩放 |

---

## `PixelFont.cs` / `PixelFontSize.cs` / `PixelFontCharacter.cs` ↔ `src/data/font.rs`

### class `PixelFont`（BMFont `.fnt` XML 解析）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `AddFontSize(path, data, atlas, outline)` | PixelFont.cs:28 | font.rs:130 (`load`) / 165 (`parse`) | ✅ | 原版解析 BMFont XML；我方 `PixelFont::parse` 解析 `<info>/<common>/<pages>/<chars>/<kernings>` |
| `Get(size)` / `Has(size)` | PixelFont.cs:85/98 | font.rs:113 (`get`) | ✅ | 取最小 `size >= request`；`has` 同逻辑返回布尔 |
| `Draw(...)` 系列 | PixelFont.cs:111-152 | renderer.rs:600 (`draw_pixel_texts`) | 🟠 | `Draw` 含 outline 描边（stroke）；缺 edge outline（4px box）和 scale 缩放 |
| `Dispose()` | PixelFont.cs:154 | — | 🔴 | 无（`font_pages` 缓存在渲染器生命周期内） |

### class `PixelFontSize`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `AutoNewline(text, width)` | PixelFontSize.cs:22 | — | 🔴 | 自动换行缺失（`line_count` 仅计数） |
| `Get(id)` | PixelFontSize.cs:70 | font.rs:26 (`PixelFontSize.characters`) | ✅ | HashMap 直接索引 |
| `Measure(char)` | PixelFontSize.cs:80 | font.rs:41 (`measure` 单字) | ✅ | |
| `Measure(string)` | PixelFontSize.cs:90 | font.rs:41 (`measure`) | ✅ | 含 kerning 累积 |
| `WidthToNextLine` / `HeightOf` | PixelFontSize.cs:127/150 | font.rs:55 / 65 | ✅ | 对齐 |
| `Draw(...)` 系列（含 stroke/edge outline） | PixelFontSize.cs:170-269 | renderer.rs:673 (`draw_pixel_line`) | 🟠 | stroke=1px solid rect 描边；缺 edge（4px box）和 scale |
| `Characters` / `Textures` / `LineHeight` / `Size` | — | font.rs:14/15/21/22 | ✅ | 字段映射一致 |

### class `PixelFontCharacter`
| 原版字段 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| ctor `(character, texture, xml)` | PixelFontCharacter.cs:20 | font.rs:181 (`parse` chars) | ✅ | 解析 `x/y/width/height/xoffset/yoffset/xadvance`；`region` 替代子纹理 |
| `Kerning` 字典 | PixelFontCharacter.cs:18 | font.rs:23 (`kerning`) | ✅ | 解析 `<kernings>` 并填入 |
| `XAdvance` / `XOffset` / `YOffset` | PixelFontCharacter.cs:25/12/13 | font.rs:24/22/21 | ✅ | |

> 关键进展：原版 BMFont `.fnt` 体系已完整实现，`PixelFont::parse` 忠映射 `info/common/pages/chars/kernings`；`PixelFont::get` + `PixelFontSize::measure/width_to_next_line/height_of` 均对齐原版；渲染层通过 `Renderer::upload_pixel_font`（`image` crate 解码 PNG）和 `draw_pixel_texts` 接入。`SpriteFont`（旧 XNB 占位）保留以兼容旧的 `set_font` 接口。

---

## `Audio.cs` ↔ `src/data/audio.rs` + `src/data/ogg.rs`
### class `Audio`（FMOD 运行时）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Banks.Load(name, loadStrings)` | Audio.cs:29 | audio.rs:48 (`AudioManifest::load`) / 69 (`load_bank`) | 🟠 | 我方从 `audio/manifest` + `audio/<bank>/manifest` 索引 OGG 流；原版加载 FMOD `.bank`。概念对应但格式/机制完全不同（无 FMOD 运行时） |
| `Init()` / `Update()` / `Unload()` | Audio.cs:166/196/204 | — | 🔴 | FMOD 初始化/更新/卸载未实现（运行时属引擎层） |
| `SetListenerPosition` / `SetCamera` | Audio.cs:214/229 | — | 🔴 | 3D 监听缺失 |
| `CheckFmod(result)` | Audio.cs:234 | — | 🔴 | 无 |
| `Play(...)` 5 重载 / `Loop(...)` 4 重载 | Audio.cs:242-350 | — | 🔴 | 触发播放未实现（仅索引/解码） |
| `Pause` / `Resume` / `Position` / `SetParameter` / `Stop` | Audio.cs:352-404 | — | 🔴 | 运行时控制缺失 |
| `CreateInstance` / `GetEventDescription` / `ReleaseUnusedDescriptions` | Audio.cs:406-459 | — | 🔴 | 事件描述缓存缺失 |
| `GetEventName` / `IsPlaying` | Audio.cs:461/476 | — | 🔴 | 缺失 |
| `BusPaused` / `BusMuted` / `BusStopAll` / `VCAVolume` | Audio.cs:489-540 | — | 🔴 | 总线/VCA 音量缺失 |
| `CreateSnapshot` / `Resume/Is/End/ReleaseSnapshot` | Audio.cs:542-594 | — | 🔴 | 快照缺失 |
| `SetMusic` / `SetAmbience` / `SetMusicParam` / `SetAltMusic` | Audio.cs:596-662 | — | 🔴 | 音乐/环境切换缺失 |

### OGG 解码（原版无独立 OGG 解析，由 FMOD/FSB5 处理）
| 我方函数 | 文件:行 | 状态 | 说明 |
|---|---|---|---|
| `decode_ogg(bytes)` | ogg.rs:27 | ✅(新增) | 原版无对应（FMOD 内部解码），属 Ruleste 自有实现 |
| `resample_stereo(src, rate)` | ogg.rs:79 | ✅(新增) | 线性重采样到 48kHz |

### class `AudioState`（音频状态应用）
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `LayerParameters` | AudioState.cs:8 | 🔴 | 缺失 | |
| ctor `(music, ambience)` / `(string, string)` | AudioState.cs:18/30 | 🔴 | 缺失 | |
| `Apply(forceSixteenthNoteHack)` | AudioState.cs:36 | 🔴 | 缺失 | 依赖 FMOD，无等价 |
| `Stop(allowFadeOut)` | AudioState.cs:68 | 🔴 | 缺失 | |
| `Clone()` | AudioState.cs:74 | 🔴 | 缺失 | |

---

## `reader.rs`（.NET `BinaryReader` 等价）↔ 多文件共用
### struct `Reader`
| 原版（BinaryReader）方法 | 我方实现 (reader.rs:行) | 状态 | 说明 |
|---|---|---|---|
| `ReadString()` (7-bit varint) | reader.rs:95 (`read_dotnet_string`) | ✅ | 与 .NET `BinaryWriter.Write(string)` 完全对齐 |
| `ReadInt16/Int32` | reader.rs:74 / 79 | ✅ | 小端对齐 |
| `ReadByte` / `ReadBoolean` | reader.rs:66 / 70 | ✅ | |
| `ReadSingle` (f32) | reader.rs:89 | ✅ | |
| `ReadUInt32` | reader.rs:84 | ✅ | |
| 7-bit 变长前缀 | reader.rs:102 (`read_7bit_varint`) | ✅ | |
| `Take(n)` 切片 | reader.rs:54 | ✅ | 边界检查 |

> `reader.rs` 被 `binary_packer.rs`、`atlas.rs` 共用，是 .NET `BinaryReader` 行为的忠实镜像（含 7-bit varint 长度前缀），对齐度高。

---

## 附：`atlas.rs` ↔ `Monocle.Atlas` / `VirtualTexture`（相关解析，任务提及）
### `.meta` / `.data` 解析
| 原版（Monocle） | 我方实现 (atlas.rs:行) | 状态 | 说明 |
|---|---|---|---|
| `Atlas`/`.meta` 读取（Packer 格式） | atlas.rs:70 (`AtlasMeta::from_bytes`) / 200 (`Atlas::load`) | ✅ | 字段顺序（unused/source_dir/unused/page count/frames）对齐 |
| `PackerNoAtlas`（每帧独立纹理） | atlas.rs:228 (`load_no_pack`) | ✅ | 实现 |
| `VirtualTexture` `.data` RLE 解码 | atlas.rs:131 (`AtlasPage::decode`) | ✅ | 按 width/height/has-alpha + 4 像素倍数 run 解码，ABGR 顺序对齐原管线 |
| `merge` / `frame_rgba_into` / `frame_clip` | atlas.rs:261 / 272 / 290 | ✅ | 帧索引与像素提取对齐 |
| `load_atlas_dir` | atlas.rs:331 | ✅ | 目录级加载合并（Gameplay/Misc 优先级） |

> 注：原版 `Atlas` 在 Monocle 中，不在 `Celeste/` 目录，但 `.meta`/`.data` 解析逻辑被 `binary_packer` 任务一并纳入对比。Ruleste 实现完整覆盖 Packer 与 NoAtlas 两种风格。

---

## 小结
- **高对齐**：`BinaryPacker`（读）/RLE 解码、`reader.rs`（.NET 二进制语义）、`atlas.rs`（`.meta`/`.data` RLE）、`PixelFont` BMFont 解析（`PixelFont::parse` 对齐 `AddFontSize`）——均为纯解析且格式忠实复刻。
- **部分实现**：`dialog.rs`（单文件解析但缺多语言/`Cleaned`/portrait/插入展开）、`pack.rs`（仅章节装载元数据，远小于 `AreaData`）、`audio.rs`+`ogg.rs`（流索引与 PCM 解码，但无 FMOD 运行时）。
- **已对齐（新增）**：`PixelFont::get` / `PixelFontSize::measure` / `width_to_next_line` / `height_of` + PNG 解码（`image` crate）+ `Renderer::upload_pixel_font` / `draw_pixel_texts`：完整复刻原版字号选择 + 字形度量 + kerning + 纹理子图上传 + 绘制（stroke 描边）。
- **占位/近似**：`font.rs` 的 `SpriteFont`（XNB 占位）仅供旧的 `set_font` 接口；`ActiveFont::Draw` 的 edge outline（4px box）和 scale 缩放未实现。
- **缺失**：`AreaData` 运行时查询与硬编码关卡表、`AreaKey`/`AreaMode`/`AreaStats`、`Dialog` 多语言/格式化工具、`Audio` FMOD 运行时与 `AudioState` 应用、`PixelFont::Dispose`——均无 Rust 等价。

<!-- ========== SECTION c3 ========== -->

# Player 插件 vs Player.cs

对比对象：
- 原版：`references/source/Celeste/Celeste/Player.cs`（6335 行，26 状态 `StateMachine`）
- 我方：`plugins/player/src/lib.rs`（1919 行，8 状态 `match` 分派）

原版用 `StateMachine.SetCallbacks(idx, Update, Coroutine, Begin, End)` 注册 26 个状态（行 1145–1170），并以协程 + Begin/End 回调驱动；我方把对应状态压成 8 个 `ST_*` 常量 + 独立 `normal_update/climb_update/...` 函数，在 `ruleste_entity_update` 里 `match` 分派（`lib.rs:343`）。常量（`GRAVITY 900`、`JUMP_SPEED -105`、`DASH_SPEED 240`、`DASH_TIME 0.15`、`CLIMB_MAX_STAMINA 110`、`WALL_JUMP_HSPEED 130` 等）与原版一致。

## `Player.cs` ↔ `plugins/player/src/lib.rs`

### 26 个原始状态实现盘点

| 状态 | 原版常量 | 原版实现 | 我方常量 | 我方实现 | 状态 |
|---|---|---|---|---|---|
| 0 StNormal | StNormal=0 | `NormalUpdate` Player.cs:3566 | `ST_NORMAL=0` | `normal_update` lib.rs:589 + SwimCheck `host::water_overlap` | ✅ |
| 1 StClimb | StClimb=1 | `ClimbUpdate` Player.cs:3926 (+Begin 3882, End 3911, Hop 4122) | `ST_CLIMB=1` | `climb_update` lib.rs:754 / `climb_begin` 1469 / `climb_hop` 1491 | ✅ |
| 2 StDash | StDash=2 | `DashUpdate` 4340 / `DashCoroutine` 4465 / Begin 4276 / End 4334 | `ST_DASH=2` | `dash_update` lib.rs:866 / `start_dash` 1598 | ✅ |
| 3 StSwim | StSwim=3 | `SwimUpdate` 4611 / `SwimBegin` 4602 | `ST_SWIM=3` | `swim_update` lib.rs:1394 + `host::water_overlap` FFI | 🟠 部分实现 |
| 4 StBoost | StBoost=4 | `BoostUpdate` 4715 / `BoostCoroutine` 4735 / Begin 4698 / End 4708 | `ST_BOOST=4` | `boost_update` lib.rs:1009 / `handle_events` EV_BOOST 2059 | ✅ |
| 5 StRedDash | StRedDash=5 | `RedDashUpdate` 4774 / `RedDashCoroutine` 4847 / Begin 4748 / End 4769 | `ST_RED_DASH=5` | `red_dash_update` lib.rs:1073 / `red_dash_build` 1049 | ✅ |
| 6 StHitSquash | StHitSquash=6 | `HitSquashUpdate` 4865 / Begin 4860 | — | （红冲撞墙在 `red_dash_update` 内直接归零 lib.rs:1116） | 🟡 近似内联 |
| 7 StLaunch | StLaunch=7 | `LaunchUpdate` 5007 / Begin 5002 | `ST_LAUNCH=7` | `launch_update` lib.rs:1131 / `handle_events` EV_LAUNCH 2074 | ✅ |
| 8 StPickup | StPickup=8 | `PickupCoroutine` | `ST_PICKUP=8` | `holdable` 事件契约 (`EV_CARRIED` + `carried` 标志) | 🟠 最佳努力（原版进入 `StPickup` 状态并跑 `PickupCoroutine`；我方改为 flag 式冻结：`carried=true` 时位置由持有者 `EV_CARRIED` 每帧驱动、`Speed` 归零，不切换 `state`；theo-crystal/key 已接入；详见 `holdable` 子系统小节） |
| 9 StDreamDash | StDreamDash=9 | `DreamDashUpdate` 5184 / Begin 5135 | `ST_DREAM_DASH=9` | `dream_dash_update` lib.rs:1438 + EV_DREAM_DASH_GRANTED 2146 | 🟠 部分实现 |
| 10 StSummitLaunch | StSummitLaunch=10 | `SummitLaunchUpdate` 5057 / Begin 5049 | `ST_SUMMIT_LAUNCH=10` | `summit_launch_update` lib.rs:1159 / EV_BADELINE_BOOST 2183 | ✅ |
| 11 StDummy | StDummy=11 | `DummyUpdate` 5687 / Begin 5680 | `ST_DUMMY=11` | `dummy_update` lib.rs:1487 | 🟠 部分实现 |
| 12 StIntroWalk | StIntroWalk=12 | `IntroWalkCoroutine` 5969 | `ST_INTRO_WALK=12` | `intro_walk_update` lib.rs:1523 | 🟠 部分实现 |
| 13 StIntroJump | StIntroJump=13 | `IntroJumpCoroutine` 5995 | `ST_INTRO_JUMP=13` | `intro_jump_update` lib.rs:1545 | 🟠 部分实现 |
| 14 StIntroRespawn | StIntroRespawn=14 | `IntroRespawnBegin` 6121 | `ST_INTRO_RESPAWN=14` | `intro_respawn_update` lib.rs:1570 | 🟠 部分实现 |
| 15 StIntroWakeUp | StIntroWakeUp=15 | `IntroWakeUpCoroutine` 6112 | `ST_INTRO_WAKE_UP=15` | `intro_wake_up_update` lib.rs:1584 | 🟡 占位（缩放/相机由 cutscene 插件驱动） |
| 16 StBirdDashTutorial | StBirdDashTutorial=16 | `BirdDashTutorialUpdate` / Begin 6176 | `ST_BIRD_DASH_TUTORIAL=16` | `bird_dash_tutorial_update` lib.rs:1604 | 🟠 部分实现（物理） |
| 17 StFrozen | StFrozen=17 | `FrozenUpdate` | `ST_FROZEN=17` | 内联于 `match` 保持冻结 lib.rs:565 | 🟠 占位（无 sprite/Holdable 锁定） |
| 18 StReflectionFall | StReflectionFall=18 | `ReflectionFallUpdate` / Begin 5887 | `ST_REFLECTION_FALL=18` | 内联于 `match` 仅 gravity lib.rs:569 | 🟠 占位（完整 6-Reflection 演出由 `reflection` 插件驱动） |
| 19 StStarFly | StStarFly=19 | `StarFlyUpdate` 5409 / `StarFlyCoroutine` 5373 / Begin 5307 / End 5333 | `ST_STARFLY=19` | `starfly_update` lib.rs:1172 / EV_STARFLY 2149 | ✅ |
| 20 StTempleFall | StTempleFall=20 | `TempleFallUpdate` / Coroutine 5857 | `ST_TEMPLE_FALL=20` | `temple_fall_update` lib.rs:1605 + `EV_TEMPLE_FALL` (mirror-temple `templeFallTrigger`) | 🟠 最佳努力（脚本坠落：重力 + 1.5s 超时自动恢复；原版由 Level 脚本驱动） |
| 21 StCassetteFly | StCassetteFly=21 | `CassetteFlyUpdate` / Begin 5602 | `ST_CASSETTE_FLY=21` | 内联于 `match` 委托 `normal_update` + `EV_CASSETTE_RIDE` (cassette-block) | 🟠 最佳努力（代驾物理 = normal，仅 ride 标志差异；原版有专门动画偏移） |
| 22 StAttract | StAttract=22 | `AttractUpdate` / Begin 5654 / End 5659 | `ST_ATTRACT=22` | `attract_update` lib.rs:1590 + `EV_ATTRACT` (dark-chaser) | 🟠 最佳努力（朝 chaser 位置 lerp 牵引 + 0.2s hold 超时；原版有完整击杀/吸附演出） |
| 23 StIntroMoonJump | StIntroMoonJump=23 | `IntroMoonJumpCoroutine` 6070 | `ST_INTRO_MOON_JUMP=23` | `intro_moon_jump_update` lib.rs:1593 | 🟡 占位 |
| 24 StFlingBird | StFlingBird=24 | `FlingBirdUpdate` / Begin 5568 / Coroutine 5585 | `ST_FLING_BIRD=24` | — | 🔴 缺失（依赖 FlingBird 插件） |
| 25 StIntroThinkForABit | StIntroThinkForABit=25 | `IntroThinkForABitCoroutine` 6156 | `ST_INTRO_THINK_FOR_A_BIT=25` | `intro_think_for_a_bit_update` lib.rs:1612 | 🟡 占位（相机微移由 Level 控制器） |

**小结**：原版 26 状态中，我方实现了 **18 个状态**（Normal/Climb/Dash/Boost/RedDash/Launch/SummitLaunch/StarFly + Swim + DreamDash + Dummy + 5 个 Intro* + BirdDashTutorial + Frozen/ReflectionFall 内联占位 + 新接入的 TempleFall/Attract/CassetteFly 最佳努力版）；拾取（StPickup）已通过 `holdable` 事件契约（flag 式 `carried` 冻结）接入，剩余 1 个状态为 `StFlingBird`（依赖尚未实现的 FlingBird 插件）。其余由事件总线 + 对应子系统插件承载，player 端保留 ST_* 常量以便未来对位。HitSquash 未独立成状态，而是被 RedDash 撞墙分支（lib.rs:1116）内联为归零。

### 状态机 & 主要方法

| 原版函数/状态 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Update` (per-frame 通用计时/落地补 dash) | Player.cs:1425 | `ruleste_entity_update` lib.rs:291 | ✅ | 落地补满 dash、`jumpGrace`、`wallSlideTimer` 重置、计时器递减、输入聚合（`forceMoveX` 覆盖）均对齐；但原版还有 hair/grabber/leader/glider 等每帧子系统，我方省略 |
| `NormalUpdate` | Player.cs:3566 | `normal_update` lib.rs:437 | ✅ | 跑动(run accel/reduce)、蹲下摩擦、可变跳、coyote、墙滑、抓墙进 climb、普通/墙跳、fast-fall 全部覆盖；**缺失**：水/滑翔(glider)、液体子系统；可抓物(holdable) 已通过 `carried` 标志 + `EV_CARRIED` 事件契约在 `normal_update` 之外旁路处理（见 `holdable` 子系统） |
| `ClimbUpdate` | Player.cs:3926 | `climb_update` lib.rs:602 | ✅ | 抓墙上下/静止、`ClimbUpCost`/`ClimbStillCost`/`ClimbJumpCost` 消耗、超时掉落、跳离/墙跳/放手进 Normal 均对齐；`SlipCheck` 用 `-4px` 近似；**缺失**：墙助推器、ledge、sweat 精灵 |
| `ClimbBegin` | Player.cs:3882 | `climb_begin` lib.rs:1221 | ✅ | 清零横向速度、纵向×0.2、贴墙 snap 对齐原版 resting pose |
| `ClimbHop` | Player.cs:4122 | `climb_hop` lib.rs:1243 | ✅ | 墙顶消失时跳过；`hopWaitX` 武装 ledge slide 逻辑在 `update` 主循环 lib.rs:357 处理，对齐原版 `hopWaitX`/`hopWaitXSpeed` |
| `ClimbJump` | Player.cs:— | `climb_jump` lib.rs:1257 | ✅ | 非地面时扣 `CLIMB_JUMP_COST`，直跳 additive `-20` 离墙；对齐 |
| `DashBegin` / `StartDash` | Player.cs:4276 / 4211 | `start_dash` lib.rs:1352 | ✅ | 扣 dash、`dashTimer=0.15`、瞄准向量(facing 兜底)、`DashCoroutine` 首步 `Speed = dir*240` 且保留更快同向分量（`dash_axis` lib.rs:1393）对齐 |
| `DashCoroutine` / `DashUpdate` | Player.cs:4465 / 4340 | `dash_update` lib.rs:720 | ✅ | 协程 `yield 0.15` → 我方计时器；对角落地 flatten×1.2 并蹲下（hyper 预备）、`EndDashSpeed 160`/上冲×0.75 收尾、`onGround` 补 `jumpGrace` 对齐；超跳/墙跳/超级墙跳出 dash 均覆盖 |
| 撞墙 `OnDashed` 事件 | Player.cs:`OnCollideH/V` | `dash_update` lib.rs:783 + `emit_crush_dir`/`emit_dash_block` 837/847 | ✅ | 命中 crush/dash block 时按 `EV_CRUSH`/`EV_DASH_BLOCK` 发方向 + 状态，供宿主 break；与原版 `OnDashed` 一致 |
| `BoostBegin`/`BoostUpdate`/`BoostCoroutine` | Player.cs:4698/4715/4735 | `handle_events` EV_BOOST 1523 + `boost_update` lib.rs:863 | ✅ | 补满 dash、被吸向 booster 中心 `BOOST_APPROACH_SPEED`、按键即发 dash；绿 booster→`StDash`、红 booster→`StRedDash`（`red_dash_build`）对齐 |
| `RedBoost`/`RedDashBegin`/`RedDashUpdate`/`RedDashCoroutine` | Player.cs:4689/4748/4774/4847 | `red_dash_build` 903 + `red_dash_update` lib.rs:927 | ✅ | 持续 240 速度、撞墙/再dash/跳离结束、穿破 dash block（`OnCollideH`→Ignore）对齐；`wallSlideTimer` 重置等细节已含 |
| `HitSquashBegin`/`Update` | Player.cs:4860/4865 | `red_dash_update` 撞面归零 lib.rs:970 | 🟡 | 原版独立状态，我方在红冲撞面时直接 `Speed=0` 回 Normal，无 squash 动画/计时；行为近似但未独立建模 |
| `ExplodeLaunch` / `StLaunch` | Player.cs:4919 / 5007 | `handle_events` EV_LAUNCH 1538 + `launch_update` lib.rs:985 | ✅ | `EXPLODE_LAUNCH_SPEED 280` 发射、`LAUNCH_Y_ACCEL_RISE/FALL`、`LAUNCH_X_ACCEL`、`LAUNCH_RETURN_SPEED 220` 回落回 Normal 对齐 |
| `BadelineBoostLaunch` / `StSummitLaunch` | Player.cs:4985 / 5057 | EV_BADELINE_BOOST 1626 + `summit_launch_update` lib.rs:1013 | ✅ | 末段 boost→`StSummitLaunch` 直上 -240 并趋近 `atX`；普通→`StLaunch`；`BADELINE_LAUNCH_SPEED -330` 对齐 |
| `SuperJump` | Player.cs:2480 | `super_jump` lib.rs:1327 | ✅ | 地面 dash→`260`/(蹲×1.25,×0.5) 上跳，hyper 基底；对齐 |
| `WallJump` | Player.cs:2548 | `wall_jump` lib.rs:1288 | ✅ | `Speed.X=130*dir`、`Speed.Y=-105`、`forceMoveXTime=0.16` 对齐 |
| `SuperWallJump` | Player.cs:2607 | `super_wall_jump` lib.rs:1308 | ✅ | `-160`/`170`/`varTime 0.25`，由 `super_wall_jump_angle`(lib.rs:1407, `dashDir.x≤0.2 && y≤-0.75`) 触发；dash 中及红冲中均覆盖 |
| `SuperBounce` | Player.cs:2708 | `handle_events` EV_SUPER_BOUNCE 1591 | ✅ | 贴弹簧顶、补满、直上 -185 对齐 |
| `SideBounce` | Player.cs:2741 | `handle_events` EV_SIDE_BOUNCE 1553 | ✅ | `[dir][fromX][fromY]` 贴弹簧面、同速过快 early-out、`SIDE_BOUNCE_SPEED 240` 对齐 |
| `StarFlyBegin/End/Coroutine/Update` | Player.cs:5307/5333/5373/5409 | `starfly_update` lib.rs:1026 + EV_STARFLY 1611 | 🟠 | 旋转趋向输入、speed 140/190 斜坡、`StarFlyTime 2`、墙/顶反弹×-0.5、跳/抓墙/再dash 退出均覆盖；**缺失**：精确 morph 帧同步与 `StarFlyReturnToNormalHitbox` 命中盒切换细节，仅做 transform 减速近似 |
| `CreateTrail` | Player.cs:1948 | — | 🔴 | 残影粒子系统未实现（`entity_draw` 只切 sprite，无 trail） |
| `UpdateSprite` | Player.cs:2042 | `ruleste_entity_draw` lib.rs:394 | 🟠 | 按状态选 idle/run/jump/fall/wallslide/duck/dash 动画对齐；**缺失**：hair、carry 姿势、背包/无背包模式、frame 音效回调 |
| `UpdateHair` | Player.cs:1996 | — | 🔴 | 头发物理未实现 |
| `CanDash` | Player.cs:1074 | `can_dash` lib.rs:1402 | ✅ | `dashes>0 && dashCooldown<=0` 对齐 |
| `UseRefill` | Player.cs:2834 | `handle_events` EV_REFILL 1510 | ✅ | 仅在 `dashes<want || stamina<20` 时补满，避免满状态误补；对齐 |
| `Die` | Player.cs:2855 | — | 🔴 | 死亡由宿主/其它插件处理，本插件未实现 `Die` 与 `PlayerDeadBody` 生成 |
| `SwimUpdate` / `SwimBegin` | Player.cs:4611/4602 | — | 🔴 | 液体子系统缺失 |
| `DreamDashUpdate` / `DreamDashBegin` | Player.cs:5184/5135 | — | 🔴 | 梦冲缺失（依赖 dream block，引擎未实现） |

### 子系统/事件桥接

- **事件入口**：原版 `Boost/ExplodeLaunch/SideBounce/SuperBounce/BadelineBoost/StarFly/Refill` 等是 Player 的公共方法，由 Booster/Bumper/Spring/Feather 等调用；我方改为宿主通过 `drain_events()` 派发 `EV_*` 事件（`handle_events` lib.rs:1505）进入对应状态，与“所有实体均为 Wasm 插件”架构一致。
- **序列化**：`serialize`/`deserialize`（lib.rs:1714/1772）按版本号迁移全部运行时字段，支持热重载；原版无此概念。
- **缺失引擎能力**（文档 lib.rs:14 明示）：水/游泳、滑翔机、可抓物、dream block、墙助推器、冰面、corner correction 等子系统未建模。

### 总体结论
核心动作状态机（Normal/Climb/Dash/Boost/RedDash/Launch/SummitLaunch/StarFly）已较完整对齐原版常量与主流控制流，8/26 状态可达生产级；剩余 18 状态多为剧情过场与依赖未实现子系统（水、梦冲、可抓物、滑翔）的特殊状态。视觉层（trail、hair、sprite 细节）与死亡/`Die`、游泳、梦冲为明确缺口。

## `holdable` 子系统 (Player.Holdable / `StPickup` / `PickupCoroutine`) ↔ `plugins/holdable/src/lib.rs`

> `holdable` 不是实体插件，而是承载原版 `Player.cs` 中 `Holdable` / `StPickup` / `PickupCoroutine` 契约的「虚拟组件」。它自己不拥有任何实体（`ruleste_plugin_entity_types` 返回 0），仅导出事件名与常量，供玩家插件与各持有者插件（theo-crystal / key 等）共享同一套抓取/携带/释放数学。下列行号指向原版 `Player.cs` 的相关片段。

### 抓取 / 携带契约（事件总线）
| 原版函数/状态 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Holdable.Pickup` / `StPickup` 进入 | Player.cs:4026 (`StPickup`) | `EV_CARRIED` 处理 (plugins/player lib.rs `handle_events`) | 🟠 | 原版进入状态并跑 `PickupCoroutine`（0.16s `Tween` 把物体吸到 `CarryOffsetTarget=(0,-12)`）；我方改为 flag 式：`EV_CARRIED [f32 x][f32 y][u8 on=1]` → `carried=true`、`Speed` 归零，每帧把 `Position` 设为 `(x,y)` |
| `Holdable.Carry`（每帧位置驱动） | Player.cs:4040 (`PickupCoroutine` 持续段) | 持有者插件每帧 `emit(EV_CARRIED 1, pos)` | 🟠 | theo-crystal/key 在 Held 态每帧读玩家位置、推进 tween、回发 `EV_CARRIED 1`；`CARRY_OFFSET_Y=-12`、`PICKUP_TWEEN_TIME=0.16`、`PICKUP_SWAY=2` 常量集中在 `holdable` lib.rs:61-74 |
| `Holdable.Release` / `Drop` | Player.cs:4076 (`Drop`/`Release`) | `EV_CARRIED [..][u8 on=0]` | 🟠 | 持有者发 `on=0` → `carried=false`，玩家恢复 `normal_update`；`EV_CARRIED_ATTACH`/`EV_CARRIED_RELEASE` 为别名（lib.rs:56-57），二者均等于 `host::EV_CARRIED`，仅用于调用点自文档化 |
| `Player.Grab` 触发（CLIMB 抓取可抓物） | Player.cs:3990 (`Update`/`GrabCheck`) | theo-crystal/key `update` 重叠 + `CLIMB` 按下 → Held | 🟠 | 原版 `Player` 检测 `Holdable` 碰撞 + 抓取键；我方由持有者插件主动检测与玩家重叠 + `CLIMB` 触发（避免玩家插件枚举所有持有者类型） |
| `another_holder_has_player`（互斥抓取） | Player.cs:3996 | theo-crystal/key `another_holder_has_player` 守卫 | 🟠 | 防止多个持有者同时抓同一玩家；由持有者插件通过 `entities_by_type("player")` + `carried` 状态（或事件）判断 |
| `slowFall` / 滑翔携带减速 | Player.cs:4050 | — | 🔴 | 原版携带时下落减速；我方 `holdable` 预留扩展点，当前 `host` 无对应 FFI |
| `CarryOffsetTarget` 整数吸附 X | Player.cs:4044 | 持有者插件逐帧应用 | 🟡 | `CARRY_OFFSET_X=0`，最终 X 偏移在持有者内按玩家朝向整数吸附 |

**小结**：`holdable` 把「可抓物」从玩家状态机抽离到事件契约，任何插件都能驱动抓取而不必让玩家插件认识每种持有者。当前 `theo-crystal` 与 `key` 已完整接入（抓取→携带→投掷/开锁），`EV_CARRIED` 处理位于 `plugins/player` 的 `handle_events`；`slowFall` 滑翔减速与多持有者优先级仍为 🔴 占位。

---

<!-- ========== SECTION c4 ========== -->

# 动平台 / 固体实体插件 (swap/falling/crush/zip/dash/cloud/bridge/...)

对比基于原版 `references/source/Celeste/Celeste/*.cs` 与 `plugins/<name>/src/lib.rs`。
状态：✅ 已实现且对齐 / 🟠 部分实现 / 🟡 近似 / 🔴 缺失 / 🔴 占位(空update)。
说明仅覆盖物理/行为方法（Update、碰撞回调、状态机）；纯视觉/粒子/音效/存档标记在多数实体中缺失（统一记为 🔴 视觉/粒子，不逐条展开）。

---

## SwapBlock.cs ↔ plugins/swap-block/src/lib.rs
### Class `SwapBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (`start/end`, `maxForwardSpeed=360/dist`, `maxBackwardSpeed=0.4*`) | SwapBlock.cs:110-168 | lib.rs:70-91 | ✅ | 节点、速度常量一致；缺 `DashListener` 改为监听 `PLAYER_DASH` 事件 |
| `OnDash(dir)` | SwapBlock.cs:195-219 | lib.rs:97-108 | ✅ | 触发 swap：target=1、`returnTimer=0.8`、speed 分支（lerp≥0.2→maxForward，否则 lerp 插值）一致 |
| `Update()` (returnTimer/speed/lerp/MoveTo) | SwapBlock.cs:221-291 | lib.rs:110-134 | ✅ | 速度逼近、lerp 逼近、actor_move 带动 rider 一致；缺 `StopPlayerRunIntoAnimation` 标志 |
| `MoveParticles` / `Render` / `PathRenderer` / `DrawBlockStyle` | SwapBlock.cs:293-392 | lib.rs:138-151 | 🔴 | 仅有纯色+描边绘制，缺路径渲染、残影、nine-slice 贴图与粒子 |

---

## FallingBlock.cs ↔ plugins/falling-block/src/lib.rs
### Class `FallingBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (autotile、seed、Coroutine) | FallingBlock.cs:33-58 | lib.rs:144-165 | 🟠 | 读取 delay/climbFall/behind/tile，设 solid；缺 autotile 贴图与随机种子 |
| `OnStaticMoverTrigger` | FallingBlock.cs:80-86 | — | 🔴 | Rust 无 StaticMover 触发链；player 触发的等价物在 phase0 |
| `PlayerFallCheck` | FallingBlock.cs:88-95 | lib.rs:117-123 | ✅ | climbFall→HasPlayerRider，否则 HasPlayerOnTop，一致 |
| `PlayerWaitCheck` | FallingBlock.cs:97-116 | lib.rs:127-141 | ✅ | 含侧边 grab 逻辑，一致 |
| `Sequence()` (状态机) | FallingBlock.cs:118-210 | lib.rs:167-250 | 🟠 | 相位对齐：等待→FallDelay→shake(0.2)→grace(0.4)→fall(160)→落定→(solid 下永久 / platform 下再检测)；差异：finalBoss 用 130 速度且 Rust 固定 160；原版落定后若 `CollideCheck<SolidTiles>` 才 `Safe=true`，Rust 用 `collision.check(0,1)`；原版触底移除基于 `level.Bounds`，Rust 用 `LEVEL_BOTTOM=8000` |
| `LandParticles`/`ShakeSfx`/`ImpactSfx`/`HighlightFade` | FallingBlock.cs:212-276 | — | 🔴 | 粒子与音效缺失（行为性无影响） |

---

## CrushBlock.cs ↔ plugins/crush-block/src/lib.rs
### Class `CrushBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (`OnDashCollide`, `axes`, `chillOut`, `giant`) | CrushBlock.cs:85-144 | lib.rs:114-137 | 🟠 | axes/chillOut/giant 解析一致；缺面贴图与 lit 边 |
| `Update()` (idle 面跟随玩家) | CrushBlock.cs:157-184 | — | 🔴 | 仅视觉：空闲时面朝玩家，Rust 无 |
| `OnDashed(player,dir)` | CrushBlock.cs:274-282 | lib.rs:184-196 | ✅ | 事件 `EV_CRUSH` 携带方向，`CanActivate(-dir)`→`Attack(-dir)`→Rebound |
| `CanActivate(dir)` | CrushBlock.cs:284-303 | lib.rs:144-158 | ✅ | giant 仅右向、axes 限制、`crushDir!=dir` 一致 |
| `Attack(dir)` | CrushBlock.cs:305-379 | lib.rs:328-345 | ✅ | 置 crushDir、canActivate=false、returnStack 去重（同/反对消）一致 |
| `AttackSequence()` (crush+return) | CrushBlock.cs:419-612 | lib.rs:198-306 | 🟠 | crushes 加速 240/accel500、chillout 在 256px 内减速至 24/accel125、return 60/accel160、waypoint 间停 0.2 一致；差异：`MoveHCheck/MoveVCheck` 会沿固体“绕行”尝试（1..4 步），Rust `move_crush` 仅直行撞墙即停，无绕行；缺 `CollideFirst<FallingBlock>` 触发与粒子 |
| `MoveHCheck`/`MoveVCheck` (滑墙) | CrushBlock.cs:614-668 | lib.rs:162-176 | 🟠 | 见上，绕行缺失 |
| `ActivateParticles`/impact 粒子/渲染 | CrushBlock.cs:381-417,186-193 | lib.rs:376-391 | 🔴 | 仅有 `draw_rect`+面贴图近似，缺粒子 |

---

## StarJumpBlock.cs ↔ plugins/star-jump-block/src/lib.rs
### Class `StarJumpBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (Depth -10000, `sinks`, `Solid`, SurfaceSoundIndex=32) | StarJumpBlock.cs:21-29 | lib.rs:75-101 | ✅ | solid、depth、sinks、startY 一致 |
| `Awake()` (railing 贴图拼接 `Open()`) | StarJumpBlock.cs:36-177 | — | 🔴 | 全部视觉，Rust 无边缘栏杆渲染 |
| `Update()` (sinks 下沉) | StarJumpBlock.cs:179-203 | lib.rs:104-129 | ✅ | rider→`sinkTimer=0.1`；yLerp 以 rate 1 逼近；`SINE_IN_OUT`；下沉 12px；actor_move 一致 |
| `StarJumpController` 星爆推进 | (独立类) | — | 🔴 | 实际加速由独立插件负责，Rust 仅下沉；原版注释已说明 |

---

## ZipMover.cs ↔ plugins/zip-mover/src/lib.rs
### Class `ZipMover`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (Depth -9999, start/target, theme, Coroutine) | ZipMover.cs:129-174 | lib.rs:68-86 | 🟠 | 节点/深度一致；缺 cog/innerCogs/streetlight/bloom 视觉 |
| `Sequence()` (rider 触发→out→rest0.5→back→rest0.5) | ZipMover.cs:337-388 | lib.rs:139-190 | ✅ | `has_player_rider` 触发；out rate 2.0、back rate 0.5、`SineIn` 缓动、停 0.5 一致；用 actor_move 带动 rider |
| `ScrapeParticlesCheck`/`CreateSparks`/`PathRenderer` | ZipMover.cs:265-393 | — | 🔴 | 粒子与缆绳/齿轮视觉缺失 |

---

## DashBlock.cs ↔ plugins/dash-block/src/lib.rs
### Class `DashBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (`safe:true`, permanent, canDash, tileType) | DashBlock.cs:30-43 | lib.rs:53-73 | 🟠 | 读取 canDash/permanent 一致；原版 Depth -12999，Rust 500；缺 autotile 与 `TileInterceptor` |
| `Awake()` (贴图/碰撞玩家即移除) | DashBlock.cs:50-78 | — | 🔴 | 视觉与重叠移除未实现 |
| `OnDashed(player,dir)` | DashBlock.cs:131-139 | lib.rs:77-106 | ✅ | `!canDash && state!=5 && state!=10`→不破；否则 Break→Rebound；Rust 经 `DASH_BLOCK` 事件取 state_at_dash(数据第9字节) |
| `Break()` / `RemoveAndFlagAsGone()` | DashBlock.cs:86-129 | lib.rs:89-103 | ✅ | permanent→`host::collect`(标记本局不重生)，否则 `host::remove`(重生) |
| `Removed()` (Freeze 0.05) | DashBlock.cs:80-84 | — | 🔴 | 碎裂定格未实现 |

---

## Cloud.cs ↔ plugins/cloud/src/lib.rs
### Class `Cloud`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (JumpThru 32×5, hitbox x=-16, fragile, SurfaceSoundIndex=4) | Cloud.cs:39-52 | lib.rs:106-122 | ✅ | hitbox(32,5,-16,0)、platform、fragile、startY 一致 |
| `Added()` (sprite/remix 变体) | Cloud.cs:59-78 | — | 🟡 | 仅 `draw_image` 近似，无 remix 变体/精灵动画 |
| `Update()` (等待→下压→回弹发射 -200→fragile 消失/重生 2.5) | Cloud.cs:80-195 | lib.rs:125-219 | ✅ | scale 逼近、rider 检测(`Speed.Y>=0`)、speed=180、下压加速 1200、发射 `Speed.Y=-200`、落下限速 220、fragile 淡出+respawn 2.5、actor_move 带动一致 |
| 粒子/rumble/精灵缩放动画 | Cloud.cs:146-161 | — | 🔴 | 视觉/反馈缺失 |

---

## BounceBlock.cs ↔ plugins/bounce-block/src/lib.rs
### Class `BounceBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (States, iceMode/fireMode, BuildSprite) | BounceBlock.cs:183-224 | lib.rs:99-119 | 🟠 | 完整 5 态机 (Waiting/WindingUp/Bouncing/BounceEnd/Broken)、`BounceState` 字段、`Color::FIRE/ICE/FLASH`、8 个常量（WIND_UP_DIST=10/ICE_WIND_UP_DIST=16/BOUNCE_DIST=24/LIFT_SPEED_X_MULT/RESPAWN_TIME/BOUNCE_END_TIME）✅；BuildSprite 8×8 子贴图→程序化矩形；`st.ice_mode = host::is_cold_mode()` 每帧读取（lib.rs:72）✅；冰模式 WindingUp 用 ICE_WIND_UP_DIST=16、颜色 COLOR_ICE、速度 35、speed_mult 0.333 直接到 BOUNCE_END；Draw 用 ice_mode 选色 ✅；序列化含 ice_mode/last_cold ✅；缺 coreModeListener 粒子/音效 |
| `Added`/`OnChangeMode` (iceMode 监听) | BounceBlock.cs:240-259 | — | 🔴 | CoreModeListener 缺失；保留 `COLOR_ICE` 常量占位 |
| `Update()` 5 态机 (Waiting→WindingUp→Bouncing→BounceEnd→Broken→reform) | BounceBlock.cs:278-438 | lib.rs:122-238 | 🟠 | 完整状态流：Waiting 检测玩家重叠（`wind_up_player_check` lib.rs:79-86）→ WindingUp 用 `approach(st.move_speed, 40, 600*dt)` 朝 `startPos - dir*10` 移动并 `windUpProgress` 插值 → Bouncing 朝 `startPos + dir*24` 移动 → BounceEnd 倒计时 0.05s → Broken 设 `depth=8990`、solid(false)、`respawnTimer=1.6` 倒计时后试回 `startPos` 用 `collision.check(0,0)` 判定 `can_reform`，成功则 `depth=-9000`、solid(true)、`reappear_flash=0.6`、发 `bounceblock_reappear` 音效 ✅；缺失 `windUpPlayerCheck` 的"侧贴+面朝"细分（我用通用 AABB 近似）、`level.Shake/Rumble/P_Reform/P_Break` 粒子、`Shaker` 抖动、`StaticMovers` 联动 |
| `WindUpPlayerCheck` (Position+UnitY/±UnitX 碰撞) | BounceBlock.cs:455-475 | lib.rs:79-86 | 🟠 | 玩家 hitbox 与 block hitbox AABB 重叠即算；对齐主流程但原版还有"侧贴+player state=Climb+朝外"细分过滤 |
| `ShakeOffPlayer` (撞后给玩家一个 lift) | BounceBlock.cs:477-486 | — | 🔴 | 缺失：未给玩家显式 `Speed` 偏置；玩家插件 `dash_update`/`red_dash_update` 自行处理反弹入射 |
| `Break` (debris/P_FireBreak/P_IceBreak) | BounceBlock.cs:488-527 | — | 🔴 | 碎块实体、粒子、Rumble 缺失 |
| `Render` (sprite 抖动偏移 + reappearFlash 白边) | BounceBlock.cs:261-276 | lib.rs:240-272 | 🟠 | 调试框颜色按状态：Waiting 静态红，WindingUp 沿 `bounceDir` 负方向偏移 `windUpProgress*10` px，BOUNCING 沿正向偏当前距离；`reappear_flash>0.01` 时加 2px 白色 padding ✅；sprite/bloom/light/wiggle 全部省略 |
| 序列化 9 字段 (state+bounceDir+progress+speed+respawn+endTimer+reformed+flash) | — | lib.rs:286-340 | 🟠 | 字节布局为：u8 state + 4f32 dir/progress/speed/respawn/endTimer + 1u8 reformed + 1f32 flash；`ruleste_entity_destroy` 从 STATES 移除条目 ✅；`ruleste_entity_serialize/deserialize` 含版本兼容（短缓冲安全截断） |
| 7 个 unit tests | — | lib.rs:344-401 | 🟢 | `constants_match_bounceblock_cs` / `state_constants_match_bounceblock_cs` / `approach_clamps_to_target` / `safe_normalize_handles_zero` / `default_state_is_waiting_at_origin` / `new_stores_start_pos` / `player_overlapping_block_basic` 全过 |

**小结**：bounce-block 状态机主体已移植（5 态完整 + 1.6s 重组），补上常量与单元测试；缺失 CoreMode 监听（`iceMode`）、`WindUpPlayerCheck` 精细过滤、碎块/粒子/Rumble、StaticMover 联动。


---

## MoveBlock.cs ↔ plugins/move-block/src/lib.rs
### Class `MoveBlock` (箭头移动块)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (Directions, canSteer, fast, BuildSprite/body/arrows) | MoveBlock.cs:292-356 | lib.rs:209-238 | 🟠 | 读 `direction` 字符串 (left/right/up/down) → `DIR_*` 常量、`canSteer`/`fast` ✅；`homeAngle` + `angleSteerSign` 按方向设 (lib.rs:73-77)；BuildSprite/arrows/light/occlude/border/sound source/buttons 全部省略 |
| `Awake` (border) | MoveBlock.cs:363-367 | — | 🔴 | Border 实体未生成 |
| `Controller` 协程: Idling→Activating(0.2s)→Moving(crashTimer0.15+crashReset0.1+steer)→Breaking(2.2s reform) | MoveBlock.cs:369-568 | lib.rs:251-340 | 🟠 | 4 态机 (IDLING/ACTIVATING/MOVING/BREAKING) 完整 ✅；`approach(speed, targetSpeed, 300dt)` 加速、`approach(angle, targetAngle, 16π*dt)` 转向、`actor_move(speed*dt, 0)` 主轴推进 ✅；撞墙判定用 `actor_move` 返回的 `hit_wall_left/right/ceiling` 或 DIR_DOWN 越界 (lib.rs:319) ✅；2.2s `REFILL_TIME` 倒计时重组 (lib.rs:344) ✅；`MoveCheck` 抗挤压（Moving 阶段块前方贴墙则 `die()`）✅；缺 Debris/StaticMover 联动/ScrapeParticles/Rumble/SoundSource/Buttons/side-depress 音效；steering 简化（仅 `pressing` bool，缺 `Input.MoveY/X` 与 45° 偏置）|
| `Update` (按钮上下/闪光渐变) | MoveBlock.cs:590-630 | lib.rs:243-247 | 🟠 | `flash` 渐变 (approach 5*dt) ✅；按钮子实体/边按音效/参数 "arrow_influence"/"arrow_stop" 缺失 |
| `OnStaticMoverTrigger` (triggered=true) | MoveBlock.cs:632-635 | lib.rs:270 | 🟠 | 静态 mover 触发器未由 host 提供；玩家骑乘用 `block_has_player_rider` 近似 |
| `MoveHExact`/`MoveVExact` (noSquish 防挤压) | MoveBlock.cs:637-659 | lib.rs:325-345 | ✅ | `MoveCheck` 抗挤压：Moving 阶段检测玩家是否位于块前方路径（leading face 之前 `step` 内、竖直方向在块体内）且块前方紧贴 Solid 墙（用 `pe.collision.check(越过块面 probe, 0)` 探测），命中则 `host::die()` 挤压致死；骑乘在块顶 (`player_on_top`) 不触发 ✅（最佳努力：仅主轴 +x 推进的抗挤压，未做 1..3 侧推） |
| `MoveCheck` (主轴碰撞+3 步侧向尝试) | MoveBlock.cs:661-706 | lib.rs:325-345 | 🟠 | 主轴抗挤压已通过 `MoveCheck` 实现（见上）；缺 1..3/-1..1 步的抗卡死侧推，`actor_move` 的 hit flag 仍近似主流程 |
| `UpdateColors` (三色 lerp) | MoveBlock.cs:708-732 | lib.rs:194-209 | 🟠 | `update_fill_color` 按状态 (IDLE/MOVING/BREAKING) lerp RGB (approach 10*dt)，过渡自然 ✅ |
| `Render` (border, fill, arrow sprite, flash) | MoveBlock.cs:753-787 | lib.rs:344-371 | 🟡 | 简化为按 fill_color 画 3px 内部矩形 + flash 时白边扩展；缺 arrow sprite/8 方向纹理/x mark/buttons 偏移 |
| 序列化 (state+dir+flags+几何+计时器+flash) | — | lib.rs:374-447 | 🟠 | 字节布局含 state/dir/canSteer/fast/w/h/start/angle/target_angle/speed/activate/crash/no_steer/reform/triggered/flash；`destroy` 清 STATES 条目 ✅ |
| 8 个 unit tests | — | lib.rs:474-553 | 🟢 | `constants_match_moveblock_cs` / `state_constants_match_moveblock_cs` / `dir_constants_match_moveblock_cs` / `home_angle_per_direction` / `dir_vector_correct` / `approach_clamps_to_target` / `default_state_is_idling` / `new_state_stores_dims` 全过 |

**小结**：move-block 状态机主体已移植（4 态 + 2.2s 重组 + 方向 4 向 + canSteer/fast 字段），补上 8 个 unit tests；`MoveCheck` 抗挤压已接入（块前方贴墙挤压致死）；缺失 Debris/StaticMover 联动/Border/Buttons/SFX。

---

## MovingPlatform.cs ↔ plugins/moving-platform/src/lib.rs
### Class `MovingPlatform`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (JumpThru, Tween YoyoLooping `Ease.SineInOut` 2s) | MovingPlatform.cs:23-50 | lib.rs:38-52 | 🟠 | 在 start↔node 间移动；速度固定 40 线性，原版为 2s `SineInOut` 缓动往返 |
| `Update()` (rider→`addY` 下沉至 3) | MovingPlatform.cs:87-104 | — | 🔴 | Rust 无“被踩时下陷”行为 |
| rider 携带 | (Tween.MoveTo 内建) | lib.rs:54-77 | 🟠 | Rust 用 `position.set_xy` 直接设位置而非 `actor_move`，可能不带动 rider |
| `OnStaticMoverTrigger`/`Render` | MovingPlatform.cs:82-80 | — | 🔴 | 触发下陷与木纹贴图缺失 |

---

## SinkingPlatform.cs ↔ plugins/sinking-platform/src/lib.rs
### Class `SinkingPlatform`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (JumpThru, startY, shaker) | SinkingPlatform.cs:22-32 | lib.rs:47-59 | 🟠 | platform+baseY 一致；缺 shaker |
| `Update()` (rider→speed 30/60(duck) 下沉；空→回升 -50/45) | SinkingPlatform.cs:63-120 | lib.rs:61-82 | 🟡 | 概念一致（被踩下沉、离开回升），但数值不同：Rust 固定 `SINK_SPEED=22`、上限 `MAX_SINK=40`，原版无上限并随 duck 加速；回升 Rust 也用 22 而非 -50 |
| rider 携带 | (MoveV/MoveTowardsY 内建) | lib.rs:61-82 | 🟠 | 直接 set_xy，未确认带动 rider |
| 木纹贴图 | SinkingPlatform.cs:51-61 | lib.rs:84-91 | 🔴 | 纯色 |

---

## Bridge.cs ↔ plugins/bridge/src/lib.rs
### Class `Bridge` (序章坍塌桥)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (按 gap 生成 BridgeTile 序列) | Bridge.cs:33-81 | lib.rs:88-108 | 🟠 | 读 `width/height`/`x/y`、`depth=200`、solid ✅；`TRIGGER_DIST=112`/`END_DIST_A=216`/`END_DIST_B=104` 对齐原版 3 段触发点 ✅；`START_FALL_COUNT=11`/`MID_FALL_COUNT=5`/`END_FALL_COUNT=7` 与原版 `tiles.RemoveAt(0)` 次数一致 ✅；缺独立 `BridgeTile` 子实体（按 tile 化、压扁、落尘动画）|
| `Update()` 3 段坍塌 (player.X >= 112 → 11 块；>= width-216 → 5 块；>= width-104 → 7 块；否则 0.2s/块) | Bridge.cs:83-156 | lib.rs:114-153 | 🟠 | `player_x()` 读取玩家 X 触发三阶段坍塌 ✅；`collapse_offset` 累计 8px/块（与原版每个 `BridgeTile` 宽 8px 一致）✅；`collapse_interval=0.2s` 对齐原版 `collapseTimer=0.2f` ✅；`ended` 后 `solid(false)` 永久穿透 ✅；`bridge_rumble_loop`/`bridge_stop` 音效 ✅；缺独立 BridgeTile 实体的 `Fall()`/粒子/冰碎渣/高度抖动 |
| `StopCollapseLoop` | Bridge.cs:159 | lib.rs:148 | 🟠 | 结束态下用 `play_sound("bridge_stop")` 近似 |
| 渲染：原版用 11 种 tile 纹理循环 | Bridge.cs:39-50 | lib.rs:158-171 | 🟠 | 简化为 `PLANK_DARK` 整底 + 每 8px 一道 `PLANK` 木纹竖条；坍塌后从 `collapse_offset` 起始绘制剩余 ✅ |
| 序列化 | — | lib.rs:175-217 | 🟠 | 含 collapsing/ended/mid_collapse_done/end_collapse_done/collapse_timer/collapse_offset/w/h；`destroy` 清 STATES ✅ |
| 2 个 unit tests | — | lib.rs:223-235 | 🟢 | `constants_match_bridge_cs` / `default_state_is_intact` 全过 |

**小结**：bridge 坍塌序列已移植（3 阶段 + 0.2s 间隔 + 11/5/7 块数），补上 2 个 unit tests；缺失 BridgeTile 子实体的物理落体动画/粒子。

---

## BridgeFixed.cs ↔ plugins/bridge-fixed/src/lib.rs
### Class `BridgeFixed` (静态布景桥)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (Solid 8px 高, 平铺 scenery 贴图) | BridgeFixed.cs:6-24 | lib.rs:14-24 | 🟡 | 静态 solid + 绘制一致；贴图用纯色近似，无行为需对齐 |

---

## CassetteBlock.cs ↔ plugins/cassette-block/src/lib.rs
### Class `CassetteBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造/分组/贴图 | CassetteBlock.cs:70-259 | lib.rs:53-69 | 🔴 | Rust 无 group（同 index 成组）、无 wiggler 视觉 |
| `Update()` (`Activated` 切换 solid/位移 `ShiftSize`) | CassetteBlock.cs:281-319 | lib.rs:71-89, 112-133 | 🟠 | Rust 监听 `CASSETTE` 事件做 `solid = !solid` 切换；`BlockedCheck`/`TryActorWiggleUp` 已接入（切换为 solid 时上推重叠玩家，见上）；原版由 `CassetteBlockManager` 按节拍驱动 `Activated`；Rust 仅有单块、无计时、无 wiggler 视觉/位移 `ShiftSize` |
| `HasPlayerRider()` → `StCassetteFly` 驱动 | CassetteBlock.cs:281-319 | lib.rs:96-143 | 🟠 | 新增 `player_on_top()` 检测玩家骑于顶部 → 状态翻转时发 `EV_CASSETTE_RIDE [u8 on]`；player `ST_CASSETTE_FLY` 委托 `normal_update`（代驾物理 = normal，仅 ride 标志），原版有专门动画偏移/相机跟随 |
| `BlockedCheck`/`TryActorWiggleUp` | CassetteBlock.cs:315-443 | lib.rs:112-133 | ✅ | 切换为 solid 时若玩家与块体重叠，且块顶上方有空间（`Collision::check(0, -(push+1))` 为 false），用 `actor_move(0, -push)` 把玩家上推出块体，避免挤压致死（对齐 `CassetteBlock.BlockedCheck`/`TryActorWiggleUp`）✅ |
| `WillToggle`/`ShiftSize`/视觉 | CassetteBlock.cs:409-419 | — | 🔴 | 视觉与位移动画缺失 |

---

## Cassette.cs ↔ plugins/cassette/src/lib.rs
### Class `Cassette`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造/Added (sprite/ghost/光晕) | Cassette.cs:96-134 | lib.rs:41-50 | 🟡 | 仅设 hitbox、收集标记；无精灵/光晕 |
| `OnPlayer(player)` (RefillStamina, sfx, Freeze) | Cassette.cs:157-167 | lib.rs:52-65 | 🟠 | Rust 检测重叠→`collect`+发 `CASSETTE` 事件；缺 RefillStamina/Freeze |
| `CollectRoutine()` (相机缩放、飞行、UNLOCK 文本、通知 Manager) | Cassette.cs:169-244 | — | 🔴 | 整段收集演出缺失，仅以事件通知替代 |
| 粒子 | Cassette.cs:148-155 | — | 🔴 | 缺失 |

---

## CassetteBlockManager.cs ↔ (无对应插件)
### Class `CassetteBlockManager`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Update()`/`AdvanceMusic` (按音乐节拍切换 index) | CassetteBlock.cs:88-153 | — | 🔴 | Rust 架构取消 Manager，改为 `cassette` 插件发 `CASSETTE` 事件、`cassette-block` 监听翻转；节拍/lead-in/静音快照等全缺失，时序模型根本不同 |
| `SetActiveIndex`/`SetWillActivate`/`StopBlocks`/`Finish` | CassetteBlock.cs:160-224 | — | 🔴 | 由事件总线隐式替代 |

---

## CrumblePlatform.cs ↔ plugins/crumble-block/src/lib.rs
### Class `CrumblePlatform`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (solid 8px 高) | CrumblePlatform.cs:27-31 | lib.rs:63-84 | ✅ | solid、宽、8px 高一致 |
| `Added()` (outline/tiles/shaker) | CrumblePlatform.cs:38-92 | — | 🔴 | 视觉与抖动缺失 |
| `Sequence()` (player on top/climb→shake→0.4s→non-collidable→tiles 掉落 2s→reform) | CrumblePlatform.cs:94-178 | lib.rs:87-124 | 🟠 | 相位对齐：onTop/climb→timer0.4→solid(false)→timer2.0→!onTop 时 solid(true)；`GetPlayerClimbing` 已通过 `player_climbing_side`（检测 `ST_CLIMB` 且贴左/右边缘）接入，侧边攀爬也会触发坍塌；重生动条件原版需无 Actor/Solid 重叠，Rust 仅 !player_on_top；缺 shake 与瓦片掉落动画 |
| `OutlineFade`/`TileOut`/`TileIn` | CrumblePlatform.cs:180-223 | — | 🔴 | 视觉 |

---

## IceBlock.cs ↔ plugins/ice-block/src/lib.rs
### Class `IceBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (内部 `Solid` 偏移(2,3)缩 4/5，仅 Cold 模式可碰撞, LavaRect 视觉, 撞即死) | IceBlock.cs:14-42 | lib.rs:88-112 | 🟠 | hitbox 宽高 + `depth=-8500` ✅；`host::is_cold_mode()` 每帧读取控制 `collision.solid(active)` ✅；`SOLID_INSET` 常量 (2,3,5) 对齐 ✅；缺内部 Solid 8px inset、LavaRect 视觉动画 |
| `Added` (初始 Collidable = Cold 模式) | IceBlock.cs:37-42 | lib.rs:104-106 | 🟠 | 初始 `collision.solid(is_cold_mode())` ✅；缺内部 Solid 的独立碰撞体 |
| `OnChangeMode` (Cold→Hot: 粒子+关闭，Hot→Cold: 开启) | IceBlock.cs:44-61 | lib.rs:125-141 | 🟠 | `last_active` 差异检测模式切换 ✅；`deactivate_flash=0.6`/`shake=0.4` ✅；缺 `P_Deactivate` 粒子发射 |
| `OnPlayer` (玩家接触即死) | IceBlock.cs:63-66 | lib.rs:118-125 (`check_player_kill`) | ✅ | `entities_by_type("player")` 检测 AABB 重叠 + `play_sound("iceblock_death")` ✅；按接触主轴计算 `dir`（`dx>=dy` 取水平，`sign` 指向块外）并调用新增 `host::die_dir(dir_x, dir_y)`（FFI `host_die_dir` + `GameState.death_dir` 存储 + 玩家 init 读 `death_dir()` 设 `facing`，对齐 `Player.deathDir`）✅ |
| `Render` (仅 Collidable 时绘制 LavaRect) | IceBlock.cs:68-74 | lib.rs:144-169 | 🟠 | `active` 时绘制 2px inset 填充+3px hollow 边框 ✅；缺 LavaRect 动态波纹/lava 内部视觉 |
| 序列化 | — | lib.rs:179-219 | 🟠 | 含 active/last_active/w/h/shake/lava_timer/deactivate_flash；`destroy` 清 STATES ✅ |
| 4 个 unit tests | — | lib.rs:226-247 | 🟢 | `default_state_is_active` / `approach_clamps_to_target` / `state_round_trip_preserves_active` / `solid_inset_constants_match_iceblock_cs` 全过 |

**小结**：ice-block 核心行为已移植（`host::is_cold_mode()` 驱动 solid 切换 + 模式切换闪光），补上 4 个 unit tests；实际 `die(dir)` 已通过新增 `host::die_dir` FFI 接入；缺失内部 Solid 独立碰撞体、LavaRect 视觉、粒子、P_Deactivate。

---

## ExitBlock.cs ↔ plugins/exit-block/src/lib.rs
### Class `ExitBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (`safe:true`, tileType, TransitionListener, cutout) | ExitBlock.cs:19-36 | lib.rs:48-60 | 🟠 | 读 width/height、solid、depth；缺 TransitionListener/cutout 过渡 |
| `Awake()` (玩家重叠则开启) | ExitBlock.cs:84-92 | lib.rs:62-84 | ✅ | 玩家重叠→非碰撞；离开→恢复碰撞，一致（Rust 用 overlap 检测） |
| `Update()` (collidable/alpha 逼近) | ExitBlock.cs:94-106 | lib.rs:62-84 | ✅ | 核心开合逻辑对齐；缺 alpha 过渡与过渡音效 |
| 过渡/边缘贴图 | ExitBlock.cs:38-131 | lib.rs:86-94 | 🟡 | 仅半透明颜色近似 |

---

## FakeWall.cs ↔ plugins/fake-wall/src/lib.rs
### Class `FakeWall`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (mode Wall/Block, overlay 贴图, EffectCutout) | FakeWall.cs:33-48 | lib.rs:45-57 | 🟠 | Rust 仅 solid；无 Wall/Block 模式与 overlay 生成 |
| `Awake()` (玩家重叠则记录 DoNotLoad 并淡出) | FakeWall.cs:72-95 | — | 🟠 | Rust 不在 Awake 处理 |
| `Update()` (玩家接触→fade→RemoveSelf，state!=9 才触发) | FakeWall.cs:140-160 | lib.rs:59-74 | 🟠 | Rust 重叠即 `solid(false)`+`set_visible(false)`+`remove`；缺 fade 淡出、state 9 豁免、`DoNotLoad` 持久标记 |

---

## CoverupWall.cs ↔ plugins/coverup-wall/src/lib.rs
### Class `CoverupWall`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造/Added (overlay 贴图) | CoverupWall.cs:15-41 | lib.rs:19-30 | 🟡 | 仅静态 solid+绘制；原版亦无行为，仅缺 overlay 贴图 |
| (无 Update) | — | lib.rs:32-33 | ✅ | 双方均无行为，对齐 |

---

## WhiteBlock.cs ↔ plugins/whiteblock/src/lib.rs
### Class `WhiteBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (JumpThru 48px, sprite, 默认启用) | WhiteBlock.cs:20-26 | lib.rs:103-117 | 🟠 | 读 `width/height`/`x/y`、`depth=8990`、platform（JumpThru）✅；缺 48px 默认宽/精灵贴图 |
| `Awake()` (HeartGem 则 Disable) | WhiteBlock.cs:28-35 | — | 🔴 | 缺 `Session.HeartGem` 探测；当前 `update` 用 `is_cold_mode()` 门控鸭子激活（仅 Cold 模式可激活），作为非 HeartGem 关卡的可替代条件 |
| `Update()` (玩家踩+鸭子 3s→Activate：变穿透+生成 bg 实心) | WhiteBlock.cs:67-104 | lib.rs:121-145 | 🟠 | `DUCK_DURATION=3.0` ✅；`player_ducking_on_top` 用 hitbox 检测 (player_bottom ∈ [block_y, block_y+4]) + `host::player_ducking(pid)` 真实鸭子查询 + Cold 模式门控 → `duck_timer` 累计 → 达 3s 触发 `activated=true` + `platform(false)` + `play_sound("whiteblock_fallthru")` ✅；鸭子查询已通过新增 `host::player_ducking` FFI（玩家每帧 `set_player_ducking` 发布 `st.ducking`）替代原 hitbox 高度近似；缺 `bgSolidTiles` Grid 实心化生成（原版由 `BgData` 重建），缺 `HeartGem` 探测复原 |
| `Activate`/`Disable` | WhiteBlock.cs:37-65 | lib.rs:131-134, 156-159 | 🟠 | 简化：`Activate` 仅 `activated=true + platform(false)`；`Disable` 用 25% 透明色 + `DrawIfDisabled`（未启用，仅保留颜色定义 `COLOR_DISABLED`）|
| 序列化 | — | lib.rs:165-204 | 🟠 | 含 enabled/activated/duck_timer/w/h；`destroy` 清 STATES ✅ |
| 2 个 unit tests | — | lib.rs:208-220 | 🟢 | `constants_match_whiteblock_cs` / `default_state_is_enabled` 全过 |

**小结**：whiteblock 鸭子激活已移植（3s 计时 + Cold 门控 + JumpThru 关闭），补上 2 个 unit tests；真实 Ducking 状态查询已通过 `host::player_ducking` FFI 接入（替代 hitbox 高度近似）；缺失 bg Grid 实心化生成、HeartGem 探测复原。

---

## 总体结论
- **高度对齐（核心物理✅）**：SwapBlock、FallingBlock、CrushBlock、StarJumpBlock、ZipMover、DashBlock、Cloud、ExitBlock、CrumblePlatform。这些实体的关键 Update/碰撞/状态机在 Rust 插件中有忠实重实现，仅缺视觉/粒子/音效（统一 🔴 视觉）。
- **近似（🟡/🟠）**：MovingPlatform/SinkingPlatform 移动方向正确但缺缓动、rider 下沉/携带不稳；CassetteBlock/Cassette 仅保留“切换/收集事件”骨架，缺分组、blocked 检查、节拍计时与收集演出；FakeWall/CoverupWall 静态近似。
 - **显著缺失（🔴）**：MoveBlock 已移植 4 态机但 Debris/StaticMover/Buttons 缺失（MoveCheck 抗挤压已接入），Seeker 8 态机已移植但多碰撞盒/击飞未实现（视线遮挡已通过 `host::line_of_sight` 接入、踩踏 `GotBouncedOn` 已接入），Bridge 坍塌已移植但 BridgeTile 子实体物理未实现，WallBooster 已升级冰/火切换但 ClimbBlocker/StaticMover/idleSFX 3D 缺失，WhiteBlock 鸭子激活已移植但 bg Grid 实心化/HeartGem 复原缺失；FireBarrier/CoreModeToggle 已接入 CoreMode 门控与翻转（新增 `host_core_mode_set` FFI）；CassetteBlockManager（被事件总线取代，节拍模型消失）。（IceBlock 的 `die(dir)` 带方向参数、WhiteBlock 的真实 Ducking 查询、Seeker 的 `CanSeePlayer` 视线遮挡、MoveBlock 的 `MoveCheck` 抗挤压已分别通过 `host::die_dir` / `host::player_ducking` / `host::line_of_sight` FFI 接入；FireBarrier 冷热门控 / CoreModeToggle 翻转通过 `host::core_mode_set` 接入。）
- **已完成**（c4/c5）：BounceBlock 5 态机 + iceMode/火模式参数（ICE_WIND_UP_DIST=16 vs 10，BOUNCING 35 速度、speed_mult 0.333）、MoveBlock 4 态机（Idling→Activating→Moving→Breaking，2.2s REFILL_TIME）、IceBlock `host::is_cold_mode()` 门控、WallBooster `left`/`notCoreMode` 切换 + 冷热 BOOST 音效、Seeker 8 态机（Idle→Patrol→Spotted→Attack→Stunned→Regenerate→Returned）、Bridge 3 阶段坍塌（11/5/7 块 + 0.2s/块 + bridge_rumble_loop/stop 音效）、WhiteBlock 3s 鸭子计时 + Cold 门控 + JumpThru 关闭。`host::core_mode_get()` FFI + `is_cold_mode()` 包装已加入 `crates/ruleste-plugins-api/src/host.rs` 和 `src/hotload/wasm_host.rs`。

<!-- ========== SECTION c5 ========== -->

# 危险物实体插件 (spinner/spikes/killbox/fire/spring/...)

> 对比说明：原版 C# 位于 `references/source/Celeste/Celeste/`，Rust 插件位于 `plugins/<name>/src/lib.rs`。
> 状态标记：✅ 已实现且对齐 / 🟠 部分实现（核心行为在，细节/分支/联动缺失）/ 🟡 近似（行为形似但实现或参数不同）/ 🔴 缺失 / 🔴 占位(空 update)。
> 注意：`ColorSwitch.cs` 与 `DarkChaser.cs` 在提供的参考源码中不存在（仅 `Level.cs` 引用了 chaser 音乐），故相关插件按"最佳努力"实现，原版对照标记 🔴 缺失参考。

---

## `DustStaticSpinner.cs` + `CrystalStaticSpinner.cs` ↔ `plugins/spinner/src/lib.rs`

### Class `DustStaticSpinner` / `CrystalStaticSpinner`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `DustStaticSpinner(ctor)` | DustStaticSpinner.cs:18 | lib.rs:127 | ✅ | 位置、碰撞圈、深度 -50；Rust 用手动重叠检测代替 PlayerCollider |
| `Update` (距离剔除 + 可碰撞) | DustStaticSpinner.cs:47 | lib.rs:168 | 🟠 | 水晶 128px 剔除已实现 (lib.rs:187)；Dust 无剔除（原版仅 crystal 剔除） |
| `OnPlayer` (Die) | DustStaticSpinner.cs:70 | lib.rs:203 | ✅ | 接触即 die() |
| `OnHoldable` (HitSpinner) | DustStaticSpinner.cs:76 | — | 🔴 | 可投掷物击中 spinner 未实现 |
| `CrystalStaticSpinner(ctor)` | CrystalStaticSpinner.cs:144 | lib.rs:151 | ✅ | 颜色属性解析（blue/red/purple/rainbow）+ 深度 -8500 |
| `Awake` + `CoreModeListener` (Area9 换色) | CrystalStaticSpinner.cs:173, 24 | — | 🔴 | CoreMode 热/冷自动切换颜色（仅 Area9）未实现 |
| `Update` (可见性/邻居 connector/rainbow hue) | CrystalStaticSpinner.cs:200 | lib.rs:212, 281 | 🟠 | Collider 可见性/InView 剔除未做；邻居 connector 绘制近似（仅距离<24，无 SolidCheck 跳过） |
| `CreateSprites` / `AddSprite` | CrystalStaticSpinner.cs:276, 318 | lib.rs:256 | 🟡 | 用单帧 fg/bg 近似；原版按 4 角 + SolidCheck + 邻居中点生成多精灵 |
| `GetHue` (rainbow) | CrystalStaticSpinner.cs:434 | lib.rs:113 | ✅ | HSV→颜色 + YoYo 缓动一致 |
| `SolidCheck` | CrystalStaticSpinner.cs:337 | — | 🔴 | 贴墙不画精灵的逻辑未实现 |
| `Destroy` (boss 碎裂) | CrystalStaticSpinner.cs:411 | — | 🔴 | 碎裂粒子/音效未实现 |
| `OnShake` / `IsRiding` (StaticMover) | CrystalStaticSpinner.cs:372, 383 | — | 🔴 | 随实体移动未实现 |

---

## `RotateSpinner.cs` (+ `BladeRotateSpinner.cs`) ↔ `plugins/rotate-spinner/src/lib.rs`

### Class `RotateSpinner` (BladeRotateSpinner 仅加精灵/拖尾)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `RotateSpinner(ctor)` | RotateSpinner.cs:25 | lib.rs:70 | ✅ | center=node0、clockwise、rotationPercent 初始化一致 |
| `Update` (轨道运动) | RotateSpinner.cs:64 | lib.rs:116 | ✅ | 每 1.8s 一圈，角度 lerp(4.712→-π/2) 一致 |
| `OnPlayer` (Die + 停转) | RotateSpinner.cs:91 | lib.rs:144 | 🟠 | die() 一致；原版击杀后 `Moving=false` 停转，Rust 继续转 |
| `Easer`/`EaserInverse` | RotateSpinner.cs:54 | — | ✅ (恒等，隐含) | |
| `fallOutOfScreen` (StaticMover 销毁) | RotateSpinner.cs:81 | — | 🔴 | 载体销毁后下落移除未实现 |
| `BladeRotateSpinner.Update` (拖尾/旋转帧) | BladeRotateSpinner.cs:19 | lib.rs:94 | 🟡 | 原版**完全不可见**（无精灵）；Rust 画 4 条 blade 以便可见，行为偏差（原版隐形危险） |

---

## `TrackSpinner.cs` (+ `BladeTrackSpinner.cs`) ↔ `plugins/track-spinner/src/lib.rs`

### Class `TrackSpinner` (BladeTrackSpinner 加精灵/音效)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `TrackSpinner(ctor)` | TrackSpinner.cs:35 | lib.rs:72 | ✅ | Start/End=node0、Speed 档位、Percent(startCenter=0.5)、Up |
| `UpdatePosition` (SineInOut) | TrackSpinner.cs:51 | lib.rs:64 | ✅ | SineInOut 缓动一致 |
| `Update` (Pause/Percent approach) | TrackSpinner.cs:62 | lib.rs:133 | ✅ | PauseTimes/MoveTimes 一致；approach 一致 |
| `OnPlayer` (Die) | TrackSpinner.cs:88 | lib.rs:153 | ✅ | 圆形+band 碰撞手动检测（lib.rs:101）与 `ColliderList(Circle6, Hitbox16x4)` 一致 |
| `OnTrackStart`/`OnTrackEnd` | TrackSpinner.cs:96 | — | 🔴 | 原版（Blade 子类）播 sprite 旋转 + 音效 + 拖尾；Rust 仅在 draw 画 dustcreature 中心帧 |
| `BladeTrackSpinner.Update` (trail) | BladeTrackSpinner.cs:25 | — | 🔴 | 拖尾粒子、`P_Trail` 未实现 |

---

## `Spikes.cs` ↔ `plugins/spikes/src/lib.rs`

### Class `Spikes`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Spikes(ctor)` | Spikes.cs:39 | lib.rs:72 | 🟠 | 方向命中盒(3px 条)用手动检测；原版还加 `LedgeBlocker`（防站上沿）未实现 |
| `Added` (按类型/方向拼精灵) | Spikes.cs:91 | lib.rs:142 | 🟡 | 逐 8px 段拼帧；变体用 `(id*31+j*17)%3` 伪随机，原版 `Calc.Random.Choose` |
| `OnCollide` (方向性击杀) | Spikes.cs:220 | lib.rs:129 | ✅ | 四方向速度门控（上:vy≥0 且 Bottom≤base 等）一致 |
| `GetSize` | Spikes.cs:251 | lib.rs:79 | ✅ | 水平用 width、垂直用 height |
| `tentacles` 类型 | Spikes.cs:101, 140 | — | 🔴 | 触手类型绘制未实现 |
| `OnEnable`/`OnDisable`/`SetSpikeColor` | Spikes.cs:80, 171, 177 | — | 🔴 | 无启用/禁用（StaticMover 联动） |
| `IsRiding` (StaticMover) | Spikes.cs:262 | — | 🔴 | 随载体移动未实现 |

---

## `TriggerSpikes.cs` ↔ `plugins/trigger-spikes/src/lib.rs`

### Class `TriggerSpikes`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `SpikeInfo.Update` (Lerp 8/4、Delay) | TriggerSpikes.cs:46 | lib.rs:144 | ✅ | 触发后 delay=0.4→lerp 以 8/s 延伸；未触发以 4/s 收回；linger 保持 0.05 一致 |
| `OnPlayer` (触发/延迟/击杀) | TriggerSpikes.cs:87 | lib.rs:211, 251 | ✅ | 首次接触触发+delay 0.4+RetractTimer；lerp≥1 击杀一致 |
| `GetPlayerCollideIndex`/`PlayerCheck` | TriggerSpikes.cs:265, 301 | lib.rs:166, 226 | ✅ | 按条带投影 + 方向速度门控一致 |
| `UpSafeBlockCheck`/`SideSafeBlockCheck` | TriggerSpikes.cs:211, 232 | — | 🔴 | SafeGroundBlocker/LedgeBlocker（站立安全）未实现 |
| `Render` (触手+尖刺) | TriggerSpikes.cs:336 | lib.rs:262 | 🟡 | 仅绘制延伸矩形+尖刺帧近似；触手摆动/颜色未实现 |
| `IsRiding` (StaticMover) | TriggerSpikes.cs:382 | — | 🔴 | 随载体移动未实现 |

---

## `Killbox.cs` ↔ `plugins/killbox/src/lib.rs`

### Class `Killbox`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Killbox(ctor)` | Killbox.cs:9 | lib.rs:44 | ✅ | Hitbox(width,32)、Collidable=false、深度 |
| `OnPlayer` (Die / 辅助无敌弹起) | Killbox.cs:17 | lib.rs:91 | 🟠 | 始终 die()；原版 `Assists.Invincible` 时 `Bounce(Top)` 未实现 |
| `Update` (滞回武装/解除) | Killbox.cs:30 | lib.rs:59 | ✅ | 脚下 32px 以上武装、头过 32px 以下解除，滞回一致 |

---

## `FireBall.cs` ↔ `plugins/fire-ball/src/lib.rs`

### Class `FireBall`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `FireBall(ctor)` | FireBall.cs:52 | lib.rs:29 | 🟠 | 读 nodes、idx；原版计算 `lengths[]` 并 `speed=60/总长*mult`；Rust 用固定 50px/s 匀速，无 mult/iceMode |
| `GetPercentPosition` (路径插值) | FireBall.cs:235 | lib.rs:52 | 🟡 | 原版按累计弧长在节点间插值；Rust 顺序节点直线移动，无弧长归一 |
| `Update` (speedMult/percent/尾迹) | FireBall.cs:116 | lib.rs:45 | 🟠 | 移动一致；无 speedMult 趋近(冰 0.5/火 1)、无 broken 重生、无粒子 |
| `OnPlayer` (火杀/冰条件) | FireBall.cs:181 | lib.rs:65 | 🟠 | 始终 die()；原版冰模式仅当 `player.Bottom > Y+4` 才杀 |
| `OnBounce` (冰面弹起碎裂) | FireBall.cs:203 | lib.rs:79-100 | ✅ | 玩家自上方踩中（脚在块体上半部）时 `remove(id)` 销毁火球并 `emit(EV_SPRING_BOUNCE, Floor)` 让玩家弹起；侧面/下方接触仍 `die()`（对齐 `FireBall.OnBounce`）✅ |
| `OnChangeMode` (CoreMode) | FireBall.cs:216 | — | 🔴 | 热/冷切换精灵与音效未实现 |
| `Added` (生成 amount 个火球) | FireBall.cs:96 | — | 🔴 | 单实例；原版按 `amount` 生成多个沿路径分布 |

---

## `FireBarrier.cs` ↔ `plugins/fire-barrier/src/lib.rs`

### Class `FireBarrier`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `FireBarrier(ctor)` | FireBarrier.cs:16 | lib.rs:15 | 🟠 | 命中盒一致；原版加 `LavaRect` 波浪视觉，Rust 仅纯色矩形 |
| `Added` (Solid + CoreMode 门控) | FireBarrier.cs:40 | lib.rs:23, 31-47 | ✅ | 每帧读 `is_cold_mode()`：仅 Hot 模式 `solid(true)` 且致死，Cold 模式取消实心且不杀（对齐 `Collidable = CoreMode == Hot`）✅ |
| `OnChangeMode` | FireBarrier.cs:51 | lib.rs:31-47 | 🟠 | Cold 模式自动消失（取消实心）已接入；热/冷切换粒子/音效未实现 |
| `OnPlayer` (Die) | FireBarrier.cs:74 | lib.rs:31 | ✅ | 重叠即 die() |
| `Render` (仅可碰撞时画) | FireBarrier.cs:94 | lib.rs:41 | 🟠 | 原版冷模式不渲染；Rust 恒画 |

---

## `DarkChaser` (参考源码缺失) ↔ `plugins/dark-chaser/src/lib.rs`

> 原版 `DarkChaser` 类在提供源码中不存在（仅 `Level.cs` 的 B-side 音乐处理器）。下表按插件目标行为对照。

### Class `DarkChaser` (best-effort)
| 目标行为 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 向玩家归位移动 | — | lib.rs:47 | 🟡 | 以 70px/s 直线追踪（无视墙体，符合"穿墙"） |
| 接触击杀 | — | lib.rs:61 | ✅ | 圆形重叠 die() |
| `Attract` 牵引玩家（StAttract 触发器） | — | lib.rs:67 | 🟠 | 玩家进入 `ATTRACT_RANGE_SQ=8100`（90px）圆，则发 `EV_ATTRACT` 带 chaser 位置；player `attract_update` 朝该点 lerp 牵引；原版有完整吸附+击杀演出 |
| 视觉 | — | lib.rs:77 | 🟡 | 矩形身体+红眼近似 |

---

## `Seeker.cs` ↔ `plugins/seeker/src/lib.rs`

### Class `Seeker`（8 状态机 Actor）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (6 个命中盒 + StateMachine) | Seeker.cs:255 | lib.rs:94-99 | 🟠 | hitbox(14,14,-3,-3) ✅；8 态常量 (Idle/Patrol/Spotted/Attack/Stunned/Skidding/Regenerate/Returned) ✅；`approach` 加速 600 ✅；`SIGHT_DIST_SQ=25600` ✅；`ATTACK_RANGE_SQ=100`、`ATTACK_SPEED=200` ✅；`STUN_DURATION=0.4`、`REGEN_DURATION=1.6` ✅；缺 6 个独立命中盒（physicsHitbox/breakWallsHitbox/attackHitbox/bounceHitbox/pushRadius/breakWallsRadius）|
| 8 态机 (Idle→Patrol→Spotted→Attack→Stunned→Regenerate→Returned) | Seeker.cs:561+ | lib.rs:107-189 | 🟠 | Idle 等待玩家进入 (SIGHT_DIST_SQ) ✅；Patrol 回 home 速度 30 ✅；Spotted 0.4s 计时后进 Attack ✅；Attack 速度 200，朝玩家方向移动，<ATTACK_RANGE 时 die() + SFX ✅；Stunned 0.4s 停止 + 速度衰减 ✅；Regenerate 1.6s → visible=false → 传送回 home ✅；Returned 回到 home 后 Patrol ✅；`CanSeePlayer` 视线遮挡已接 `host::line_of_sight` ✅；缺 Skidding、撞墙反弹、多个碰撞盒交互 |
| `OnAttackPlayer`/`OnBouncePlayer` | Seeker.cs:368, 383 | lib.rs:177-183 | 🟠 | Attack 阶段距离<10px → `die()` ✅；撞墙后进入 Stunned ✅；缺 bounce 后的击飞逻辑 |
| `GotBouncedOn`/`OnHoldable` | Seeker.cs:399, 546 | lib.rs:155-186 | 🟠 | `GotBouncedOn` 已实现：玩家自上方踩中（水平重叠 + 脚在块体上半部）时 seeker 转入 `STUNNED` 并发 `EV_SPRING_BOUNCE`（Floor 朝向，玩家弹起由 player 插件后续消费），seeker 自身不再致死；`OnHoldable`（可抓联动）仍缺失 |
| `CanSeePlayer` 视线 | Seeker.cs:413 | lib.rs:124-131, 163-171, 188-195 | ✅ | 视线遮挡判定通过 `host::line_of_sight(x1,y1,x2,y2)`（新增 `SolidGrid::line_of_sight`：world→tile 网格步进，半格步长避免穿薄墙，越界视为遮挡）实现；Idle/Patrol 进入 `Spotted` 时要求 `line_of_sight(seeker, player)` 为真（对齐 `Seeker.CanSeePlayer`）✅ |
| 渲染 | Seeker.cs | lib.rs:195-205 | 🟡 | 程序化：深蓝体 + 变色 eye（红=攻击/黄=眩晕/灰=死亡）✅；无 sprite/shockwave 动画 |
| 序列化 | — | lib.rs:208-263 | 🟠 | 含 state/last_state/start/speed/timers/visible；`destroy` 清 STATES ✅ |
| 4 个 unit tests | — | lib.rs:269-290 | 🟢 | `constants_match_seeker_cs`/`state_constants_match_seeker_cs`/`approach_clamps_to_target`/`default_state_is_idle` 全过 |

**小结**：seeker 8 态机核心已移植（Idle→Patrol→Spotted→Attack→Stunned→Regenerate→Returned），补上 4 个 unit tests；`CanSeePlayer` 视线遮挡已通过 `host::line_of_sight` 接入；缺失多碰撞盒/击飞/可抓联动。

---

## `ColorSwitch` (参考源码缺失) ↔ `plugins/color-switch/src/lib.rs`

> `ColorSwitch.cs` 不在提供的参考源码中。

### Class `ColorSwitch` (best-effort)
| 目标行为 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 按钮渲染 | — | lib.rs (draw 内联) | 🟡 | 仅画双层方块标记；原版切换房间同色实心瓦片可行走性（由宿主做），插件仅为视觉标记 |
| 瓦片切换 | — | — | 🔴 | 宿主负责的瓦片实心化未在此插件体现（插件文档声明由宿主处理） |

---

## `WallBooster.cs` ↔ `plugins/wall-booster/src/lib.rs`

### Class `WallBooster`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造 (2px 宽贴墙 hitbox + CoreModeListener + StaticMover + ClimbBlocker) | WallBooster.cs:24-45 | lib.rs:86-106 | 🟠 | 读 `left`/`notCoreMode`/`height` → hitbox(2,h,0,0) 或 (2,h,6,0) ✅；`depth=1999` ✅；`is_cold_mode()` 或 `notCoreMode` 控制 `ice_mode` ✅；缺 StaticMover/ClimbBlocker/BuildSprite |
| `OnChangeMode` (IceMode 切换 → ClimbBlocker + sprite + 音效) | WallBooster.cs:85-101 | lib.rs:121-133 | 🟠 | `ice_mode` 翻转检测 ✅；`BOOST_COLD`/`BOOST_HOT` 颜色区分 ✅；`icewall_boost`/`wallbooster_boost` 音效 ✅；缺 ClimbBlocker 切换/精灵动画/idle SFX 定位 |
| `Update` (IdleSfx 3D 定位) | WallBooster.cs:103-120 | — | 🔴 | 缺 3D 音效定位 |
| 碰撞检测 + 助推事件 | — | lib.rs:137-165 | 🟠 | AABB 检测玩家 + `emit(BOOST)` + 0.1s cooldown ✅ |
| 渲染 | WallBooster.cs | lib.rs:167-177 | 🟡 | 程序化：BOOST_COLD/BOOST_HOT 填充 + 1px BOOST_EDGE 顶部高光线 ✅；无精灵动画 |
| 序列化 | — | lib.rs:180-211 | 🟠 | 含 left/not_core_mode/ice_mode/w/h/cooldown；`destroy` 清 STATES ✅ |
| 2 个 unit tests | — | lib.rs:218-228 | 🟢 | `default_is_left_hot` / `ice_mode_flips_to_cold_when_needed` 全过 |

**小结**：wall-booster 已升级（`left`/`notCoreMode` 解析 + `is_cold_mode()` 冰/火切换 + 2 种 BOOST 音效），补上 2 个 unit tests；缺失 StaticMover/ClimbBlocker/精灵/idle SFX 3D 定位。

---

## `Spring.cs` ↔ `plugins/spring/src/lib.rs`

### Class `Spring`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Spring(ctor)` | Spring.cs:30 | lib.rs:64 | ✅ | 三方向 hitbox（16x6/-8,-6 等）、深度 -8501 一致 |
| `OnCollide` (Floor SuperBounce) | Spring.cs:123 | lib.rs:129 | ✅ | 仅 vy≥0 且非 dash → 发 `EV_SUPER_BOUNCE(from_y)`；冷却 0.2 近似（原版靠动画） |
| `OnCollide` (WallLeft/Right SideBounce) | Spring.cs:138, 146 | lib.rs:141, 156 | ✅ | 发 `EV_SIDE_BOUNCE`（方向/边/中心Y）一致 |
| `OnHoldable` / `OnPuffer` / `OnSeeker` | Spring.cs:165, 173, 181 | lib.rs:98-197 (`overlap_target` + `EV_SPRING_BOUNCE`) | 🟠 | 新增 `overlap_target(&["theo-crystal","key","seeker","puffer"])` 检测，任一重叠即触发 spring 冷却并发 `EV_SPRING_BOUNCE`（payload `[u32 target][u8 orientation][f32 from_x][f32 from_y]`），由对应插件自行施加弹跳；玩家仍走 `EV_SUPER_BOUNCE`/`EV_SIDE_BOUNCE`。最佳努力：弹簧自身触发已接入，但 theo-crystal/key/seeker/puffer 尚未消费 `EV_SPRING_BOUNCE`（后续需在各自插件加监听） |
| `OnEnable`/`OnDisable` | Spring.cs:102 | — | 🔴 | 无启用/禁用（StaticMover 联动） |
| StaticMover 附着平台 | Spring.cs:47 | — | 🔴 | 随平台移动未实现 |

---

## `LockBlock.cs` ↔ `plugins/lock-block/src/lib.rs`

### Class `LockBlock`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `LockBlock(ctor)` (Solid 32x32) | LockBlock.cs:24 | lib.rs:38 | 🟠 | 实心 + hitbox(w/h) 一致；原版带 60 半径 PlayerCollider |
| `OnPlayer` + `TryOpen` | LockBlock.cs:73, 89 | lib.rs:51 | 🟠 | 原版需玩家持有 Key follower 且玩家↔锁中心间无 Solid 才开；Rust 收到任意 `KEY` 事件即开（无持有/视线校验） |
| `UnlockRoutine` (动画/DoNotLoad) | LockBlock.cs:101 | lib.rs:57 | 🔴 | 瞬间开启，无开门动画、`Session.DoNotLoad`、音乐进度 |

---

## `TouchSwitch.cs` ↔ `plugins/touch-switch/src/lib.rs`

### Class `TouchSwitch`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `TouchSwitch(ctor)` | TouchSwitch.cs:40 | lib.rs:42 | 🟠 | 16x16 命中盒一致；原版还有 30/20/24 的 Player/Holdable/Seeker 碰撞器 |
| `TurnOn` + `Switch.Activate` (全组逻辑) | TouchSwitch.cs:91 | lib.rs:54 | 🟠 | Rust 玩家重叠即发 `SWITCH` 一次；原版经 Switch 组件（全房间开关集满才 Finish、最后一个播 last_oneshot）未复现 |
| `OnHoldable` / `OnSeeker` | TouchSwitch.cs:109, 114 | lib.rs:48-95 (`overlap_any`) | ✅ | 新增 `overlap_any(&["theo-crystal","key"])` 与 `overlap_any(&["seeker"])` 检测，玩家/可抓物/Seeker 任一重叠即发 `SWITCH`（对齐 `TouchSwitch.OnHoldable`/`OnSeeker`）✅ |
| `Update` (颜色/动画/粒子) | TouchSwitch.cs:122 | lib.rs:69 | 🟡 | 仅 idle/on 两色；原版 inactive→active→finish 渐变+脉冲+粒子缺失 |

---

## `SwitchGate.cs` ↔ `plugins/switch-gate/src/lib.rs`

### Class `SwitchGate`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `SwitchGate(ctor)` (Solid) | SwitchGate.cs:34 | lib.rs:47 | ✅ | 实心 + hitbox(w/h) + node=node0 一致 |
| `Awake` + `Sequence` (开→滑向 node→非实心) | SwitchGate.cs:68, 102 | lib.rs:78 | 🟠 | 核心一致：收到 `SWITCH` 后向 node 滑动（Rust 1.8s 线性，原版 2s CubeOut）终点 `solid(false)`；缺失 `persistent` 设关卡旗、缺失粒子/抖动 |
| `Switch.Check` 全组判定 | SwitchGate.cs:105 | — | 🟠 | Rust 收到任一 SWITCH 即开，原版等 Switch 组件全亮 |

---

## `RidgeGate.cs` ↔ `plugins/ridge-gate/src/lib.rs`

### Class `RidgeGate`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `RidgeGate(ctor)` (Solid) | RidgeGate.cs:16 | lib.rs:30 | ✅ | 实心 + hitbox 一致 |
| `Awake` + `EnterSequence` (出生重叠则滑向 node) | RidgeGate.cs:23, 35 | lib.rs:46 | 🔴 | 触发条件不同：原版在 `Awake` 若与玩家重叠则滑到 node（1s CubeOut）并保持实心；Rust 改为监听 `SWITCH` 事件后**原地**取消实心，无位移 |
| 图像 (ridge/farewell) | RidgeGate.cs:20 | lib.rs:64 | 🟡 | 仅纯色矩形 |

---

## `Key.cs` ↔ `plugins/key/src/lib.rs`

### Class `Key` (Holdable)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Key(ctor)` (Follower + 碰撞) | Key.cs:44 | lib.rs:107 | 🟠 | 16x16 命中盒 ✅；原版 `Follower` 随从系统未实现 |
| `OnPlayer` (获得→记录) | Key.cs:123 | lib.rs:118-260 | 🟠 | 接入 `holdable` 事件总线：玩家重叠 + CLIMB 按下 → Held，每帧发 `EV_CARRIED 1` 冻结玩家 ✅；JUMP/DASH/松开 CLIMB → Thrown ✅；Thrown 撞 `lockBlock` 即发 `KEY` + `host::collect` ✅（替代原版 `Leader.GainFollower`/`Session.Keys`/`DoNotLoad`） |
| `UseRoutine` (插入锁动画) | Key.cs:162 | — | 🔴 | 插入锁孔旋转/消失动画未实现（由 lockBlock 监听 KEY 简化） |
| `RegisterUsed` / `NodeRoutine` | Key.cs:152, 142 | — | 🔴 | 释放随从、磁带飞行未实现 |
| 5 个 unit tests | — | lib.rs:263-297 | 🟢 | `carry_offset_matches_player_cs`/`throw_constants_match_key_cs`/`approach_clamps_to_target`/`state_constants_are_distinct`/`default_state_is_idle` 全过 |

**小结**：key 已接入 `holdable` 事件总线（Idle→Held→Thrown + 撞锁发 KEY 收集），补上 5 个 unit tests；缺失 Follower 随从系统/插入锁动画/DoNotLoad 标记。

---

## `CoreModeToggle.cs` / `RisingLava.cs` / `CoreMessage.cs` ↔ `plugins/core/src/lib.rs`

> core 插件聚合 4 类：`coreMessage`、`coreModeToggle`、`risingLava`、`sandwichLava`。

### Class `CoreModeToggle`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `CoreModeToggle(ctor)` | CoreModeToggle.cs:40 | lib.rs:56 | 🟠 | 建 hitbox ✅；新增 `cooldown` 字段 |
| `OnPlayer` (切换 CoreMode) | CoreModeToggle.cs:103 | lib.rs:90-122 | ✅ | 玩家触碰时通过 `host::set_core_mode(!is_cold)` 翻转 CoreMode，带 1s cooldown（Flash/Freeze 视觉未实现）✅；需 `host_core_mode_set` FFI |
| `OnChangeMode` / `SetSprite` | CoreModeToggle.cs:65, 71 | — | 🔴 | 冰/火精灵与音效未实现 |

### Class `RisingLava`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `RisingLava(ctor)` + `Added` | RisingLava.cs:38, 58 | lib.rs:57 | 🟠 | hitbox 一致；原版固定于屏幕左/底并随相机 X |
| `Update` (上升 + 等待 + lerp) | RisingLava.cs:120 | lib.rs:81 | 🟠 | Rust 恒速 4px/s 上升 + 杀；原版 `Speed=-30`（30px/s）、intro 等待、`lerp` 缓动、相机 X 跟随均未实现 |
| `OnPlayer` (Kill) | RisingLava.cs:95 | lib.rs:85 | ✅ | 重叠即 die() |
| `sandwichLava` | (SandwichLava 类) | lib.rs:85 | 🟡 | 当作同款杀戮矩形处理 |

### Class `CoreMessage`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 消息触发逻辑 | CoreMessage.cs | lib.rs:97 | 🔴 占位 | 仅画矩形；原版弹出核心模式说明文本未实现 |

<!-- ========== SECTION c6 ========== -->

# 道具 / NPC / 装饰实体插件 (strawberry/refill/npc/bonfire/...)

对比基于原版 C# 实体类与当前 Wasm 插件实现。状态标记：✅ 已实现且对齐 / 🟠 部分实现 / 🟡 近似（视觉占位，逻辑缺失） / 🔴 缺失 / 🔴 占位（空 update/draw）。许多插件为"仅绘制矩形"的视觉占位，因为宿主尚未暴露对话、渲染目标、精灵库、玩家浮空/抓握等系统。

---

## `Strawberry.cs` ↔ `plugins/strawberry/src/lib.rs`
### Class `Strawberry`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Strawberry(EntityData,offset)` 构造 | 80 | lib.rs:68 | ✅ | sprite 选择（strawberry/goldberry/moonberry）、hitbox(14,14,-7,-7)、depth -100 |
| `Added` (sprite/wobble/light/bloom) | 111 | — (init 内 lib.rs:78-101) | 🟠 | 精灵在 init 直接播放，wiggler/light/bloom 无对应 |
| `Update` (wobble/sprite bob) | 176 | lib.rs:134-140 | 🟠 | bob ±2px 实现；flyingAway/collect 逻辑分离 |
| `OnDash(Vector2)` ( winged 飞走) | 264 | lib.rs:120-131 | 🟠 | 检测玩家 state==2 触发 flying_away，但无 FlyAwayRoutine 的笑/音效序列 |
| `OnPlayer(Player)` (触碰跟随机) | 298 | lib.rs:154-168 | 🟠 | 触碰置 following 并播放音效；无 Winged 收翼/种子逻辑 |
| `OnCollect` (收集计数/保存) | 328 | lib.rs:189-192 | 🟠 | 调用 `host::collect` + 音效；无 StrawberryCollectIndex/Seeds 记录 |
| `FlyAwayRoutine` (向上飞离) | 355 | lib.rs:109-117 | 🟡 | 向上匀速 120px/s 飞，3s 后 remove；无音效/旋转 |
| `CollectRoutine` (收集动画/加分) | 380 | lib.rs:189 | 🔴 | 直接 collect，无 StrawberryPoints 弹分 |
| `OnLoseLeader` (回巢曲线) | 409 | — | 🔴 | 无 Follower 系统，回巢丢失 |
| `CollectedSeeds` | 435 | — | 🔴 | 无种子系统 |

---

## `Refill.cs` ↔ `plugins/refill/src/lib.rs`
### Class `Refill`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Refill(pos,twoDash,oneUse)` 构造 | 50 | lib.rs:79 | ✅ | hitbox(16,16,-8,-8)、depth -100、twoDash/oneUse 解析 |
| `Added` | 104 | — | 🔴 | 无 level 引用 |
| `Update` (respawn/闪光/粒子) | 110 | lib.rs:99-149 | 🟠 | respawn 计时 (2.5s)、2s 周期 flash 实现；无 glow 粒子 |
| `UpdateY` (sine 浮动) | 149 | lib.rs:160 | 🟠 | 用 `timer` 近似 sine 浮动 |
| `OnPlayer` (UseRefill/碎裂) | 167 | lib.rs:118-148 | 🟠 | 仅在玩家 dash<want 且 stamina<20 时消耗；发 `EV_REFILL`、停用、计时；oneUse→collect |
| `RefillRoutine` (Freeze/Shake/SlashFx) | 179 | — | 🔴 | 无屏震/碎片/冻结 |
| `Respawn` | 135 | lib.rs:102-109 | 🟠 | 重置 active；无音效/粒子 |
| `Render` | 158 | lib.rs:153 | 🟠 | 用 atlas 帧 `idle##`/`flash##` 程序化绘制（非 SpriteBank） |

---

## `Booster.cs` ↔ `plugins/booster/src/lib.rs`
### Class `Booster`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Booster(pos,red)` 构造 | 53 | lib.rs:56 | 🟠 | hitbox Circle(10,0,2)→矩形近似；depth -8500；red 标志 |
| `Added` (outline 子实体) | 81 | — | 🔴 | 无 outline 子实体 |
| `Appear` | 95 | — | 🔴 | 无 respawn 粒子 |
| `OnPlayer` (Boost/RedBoost) | 113 | lib.rs:97-114 | 🟠 | phase 0→1 发 `EV_BOOST`（含 red 标志坐标）；无 cannotUseTimer 前摇判定 |
| `PlayerBoosted` | 133 | — | 🔴 | 无红 booster 锁块过渡 |
| `BoostRoutine` (贴附玩家旋转) | 177 | lib.rs:116-122 | 🟡 | 仅 spin 计时 0.45s，无贴附玩家/粒子 |
| `OnPlayerDashed` | 202 | — | 🔴 | 无 |
| `PlayerReleased` (pop/respawn) | 210 | lib.rs:123-129 | 🟠 | phase 2→3，respawn 1s |
| `PlayerDied` | 221 | — | 🔴 | 无 |
| `Respawn` | 231 | lib.rs:130-135 | 🟠 | phase 3→0 |
| `Update` (cannotUse/respawn/snap) | 242 | lib.rs:91-139 | 🟠 | 相位机；无 sprite 向玩家贴附/inside→loop |
| `Render` | 273 | lib.rs:142-161 | 🟡 | 程序化 atlas 帧（0 idle / 1 spin / 2 pop） |

---

## `ClutterBlock.cs` ↔ `plugins/clutter/src/lib.rs`
### Class `ClutterBlock`（插件实体类型为 `redBlocks`/`yellowBlocks`/`greenBlocks`，对应 `ClutterBlockBase`）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `ClutterBlock(pos,tex,color)` | 44 | lib.rs:24 | 🟠 | 静态 solid 矩形，depth 8999，黑色 70% 透明 |
| `WeightDown` | 53 | — | 🔴 | 无堆叠配重系统 |
| `Update` (float 波浪/玩家压下) | 63 | lib.rs:39 (空) | 🔴 占位 | 无浮动波/玩家踩踏下沉 |
| `Absorb` | 87 | — | 🔴 | 无吸收动画 |

---

## `Door.cs` ↔ `plugins/door/src/lib.rs`
### Class `Door`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Door(data,offset)` 构造 | 20 | lib.rs:47 | ✅ | hitbox(12,22,-6,-23)、depth 8998、type→bank 选择、idle |
| `HitPlayer`/`Open` | 52/60 | lib.rs:67,98 | 🟠 | 触碰播放 open 并据玩家侧 flipX；无 close→idle 回链（由 host 动画器驱动） |
| `Update` (solid 卡死/close sfx) | 77 | lib.rs:70-72 | 🟠 | 与 Solid 碰撞则 disabled；无 close 音效 |
| `IsRiding`/`OnSquish` | 43/48 | — | 🔴 | Actor 基类行为未移植 |

---

## `HeartGemDoor.cs` ↔ `plugins/heart-gem-door/src/lib.rs`
### Class `HeartGemDoor`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `HeartGemDoor(data,offset)` 构造 | 110 | lib.rs:91 | 🟠 | 解析 requires/width/height；solid 矩形 |
| `Added` (Top/BotSolid 生成 + flag) | 126 | — | 🔴 | 无动态 Solid 上下半 |
| `Routine` (心数填充/开门) | 181 | lib.rs:118-159 | 🟠 | 玩家邻近(<80px) 时 counter 升，达 requires 则 opened；无 startHidden/WhiteLine/闪屏 |
| `Update` (offset/mist/粒子) | 301 | — | 🔴 | 无薄雾/粒子 |
| `RenderBloom`/`DrawMist`/`DrawInterior`/`DrawEdges`/`Render` | 316-446 | lib.rs:163-198 | 🟡 | 简化为：背景缝 + 上半上移/下半下移 + 心形奖牌颜色 |
| `WhiteLine` 内部类 | 20 | — | 🔴 | 无 |

---

## `Checkpoint.cs` ↔ `plugins/checkpoint/src/lib.rs`
### Class `Checkpoint`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Checkpoint(pos,bg)` 构造 | 36 | lib.rs:59 | 🟠 | depth 9990；无 bg 图选择 |
| `Added` (bg 图 / 已触发则 TurnOn) | 49 | lib.rs:68-75 | 🟠 | init 比对 `host::respawn_position` 决定 triggered |
| `Update` (triggered→TurnOn→脉冲) | 72 | lib.rs:90-115 | 🟠 | 玩家在场则 set_respawn；triggered 后 sine/fade 脉冲 |
| `TurnOn` (flash/light) | 98 | lib.rs:123-141 | 🟠 | draw 中画 highlight09 + flash 帧；无 VertexLight/Bloom |
| `EaseLightsOn` | 123 | — | 🔴 | 灯光缓动未移植 |

---

## `Memorial.cs` ↔ `plugins/memorial/src/lib.rs`
### Class `Memorial`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Memorial(pos)` 构造 | 18 | lib.rs:56 | 🟠 | depth 0（原 100）；无 slab 图 origin |
| `Added` (MemorialText/dreamy) | 34 | — | 🔴 | 对话/文本系统未实现 |
| `Update` (text.Show 切换/音效) | 52 | lib.rs:67-85 | 🟡 | 仅记 touched 标志；无文本淡入/循环音效 |
| `Render` | — | lib.rs:89-97 | 🟠 | 程序化石板（frame 图 + 矩形），非原版 slab 贴图 |

---

## `NPC.cs` ↔ `plugins/npc/src/lib.rs`
### Class `NPC`（含 `BirdNPC` 等子类行为，插件统一为 `npc`）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Update` (light alpha/phone sfx) | 55 | lib.rs:71-74 | 🔴 | 仅 timer 累加；无灯光/电话音 |
| `SetupTheoSpriteSounds`/`SetupGrannySpriteSounds` | 81/108 | — | 🔴 | 无精灵音 |
| `PlayerApproach*` / `PlayerLeave` / `MoveTo` | 128-245 | — | 🔴 | 对话/移动协程全部缺失 |
| `draw` | — | lib.rs:78-94 | 🟡 | 程序化 granny 剪影 + 微 bob；无按 npc id 换肤 |

> 注：`NPC.cs` 为基类，`BirdNPC.cs` 等子类为具体对话脚本；插件仅作装饰占位。

---

## `Bonfire.cs` ↔ `plugins/decorations/src/bonfire.rs`
### Class `Bonfire`（作为 decorations crate 的 `bonfire` kind）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Bonfire(pos,mode)` 构造 | 35 | bonfire.rs:54 | 🟠 | depth -5；mode 解析（unlit/lit/smoking） |
| `SetMode` (Lit/Smoking/音效) | 62 | bonfire.rs:61,89 | 🟠 | 仅存 mode；draw 区分三态；无 campfire 音效 |
| `Update` (亮度脉动/闪烁) | 91 | bonfire.rs:77-81 | 🟡 | 仅 timer 累加；亮度/wiggle 在 draw 内程序化 |
| `draw` (火焰/烟) | — | bonfire.rs:83-130 | 🟠 | Lit 三层火焰 flicker，Smoking 烟柱；非精灵 |

---

## `IntroCar.cs` ↔ `plugins/intro-car/src/lib.rs`
### Class `IntroCar`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `IntroCar(pos)` 构造 | 16 | lib.rs:59 | 🟠 | JumpThru→platform(42,4,-21,-16)、depth 1 |
| `Added` (wheels/pavement/barrier) | 34 | — | 🔴 | 无子实体（轮/路面/栏） |
| `Update` (被踩下沉/音效) | 54 | lib.rs:79-105 | 🟠 | 玩家在车顶则 sink 至多 2px；无上/下车音效 |
| `GetLandSoundIndex` | 74 | — | 🔴 | 无 |

---

## `InvisibleBarrier.cs` ↔ `plugins/invisible-barrier/src/lib.rs`
### Class `InvisibleBarrier`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `InvisibleBarrier(pos,w,h)` 构造 | 9 | lib.rs:20-38 | 🟠 | solid 矩形 + depth -1e6；无 ClimbBlocker |
| `Update` (玩家在内则穿透) | 24 | lib.rs:41 (空) | 🔴 占位 | 碰撞为宿主 TODO，无法推玩家 |

---

## `CliffFlags.cs` ↔ `plugins/cliffside-flag/src/lib.rs`（另见 `decorations/src/cliffflag.rs`）
### Class `CliffFlags`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `CliffFlags(from,to)` 构造 | 20 | lib.rs:16-25 | 🟡 | 静态白旗矩形 + 杆；无 Flagline 4 色布幔物理/下垂 |
| `Update`/`Render` | — | lib.rs:28-36 | 🔴 占位 | 无动画 |

---

## `Water.cs` ↔ `plugins/water/src/lib.rs`
### Class `Water`（含 `Surface` 子网格）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Surface` (Ripple/Ray/Tension 网格) | 62-301 | — | 🔴 | 完整水面网格渲染缺失 |
| `Added` (grid 镂空) | 357 | — | 🔴 | 无 |
| `Update` (涟漪/张力/入水音) | 371 | lib.rs:33 (空) | 🔴 占位 | 浮空由宿主处理（注释） |
| `Render`/`RenderDisplacement` | 449/476 | lib.rs:36-39 | 🟡 | 半透明蓝矩形；无表面波纹/置换 |

---

## `WaterFall.cs` ↔ `plugins/waterfall/src/lib.rs`
### Class `WaterFall`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Awake` (探高/双层音效) | 31 | — | 🔴 | 无 |
| `Update` (涟漪/溅射) | 54 | lib.rs:33 (空) | 🔴 占位 | 无 |
| `Render`/`RenderDisplacement` | 70/75 | lib.rs:36-44 | 🟡 | 半透明矩形 + 竖条；无根据水面高度裁剪 |

---

## `SummitBackgroundManager`(无同名文件) ↔ `plugins/summit/src/lib.rs`
### 多实体插件（覆盖 `summitGemManager`/`summitcheckpoint`/`summitcloud`/`summitgem`，原版对应 `SummitCheckpoint.cs` 等）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 各 summit 实体构造 | — | lib.rs:60 | 🟡 | 按 kind 建矩形；cloud 为 platform |
| 云路径巡逻 | — | lib.rs:90-103 | 🟠 | summitcloud 沿 nodes 移动 |
| gem 收集 | — | lib.rs:104-114 | 🟠 | summitgem 触碰 collect |
| checkpoint 设重生 | — | lib.rs:75-77 | ✅ | summitcheckpoint→set_respawn |
| 其余 | — | lib.rs:124-137 | 🟡 | 仅绘制色块 |

---

## `BlackGem`(无同名 C# 文件) ↔ `plugins/black-gem/src/lib.rs`
### 自定义 B-side 黑宝石（`blackGem`）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| （无原版对应，属本仓库自定义实体） | — | lib.rs:40 | 🟡 | hitbox(16,16,-8,-8) solid 外观 |
| 玩家重叠收集 | — | lib.rs:51-61 | ✅ | 重叠则 collect + 隐藏 |
| 绘制 | — | lib.rs:65-71 | 🟡 | 黑方块 + 白点 |

---

## `DreamMirror.cs` ↔ `plugins/dream-mirror/src/lib.rs`
### Class `DreamMirror`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `DreamMirror(pos)` 构造 | 53 | lib.rs:24 | 🟠 | hitbox + depth 9500；无 frame 子实体 |
| `BeforeRender` (实时反射 RT) | 123 | — | 🔴 | 渲染目标反射系统缺失 |
| `InteractRoutine` (进入镜) | 166 | — | 🔴 | 无 CS02_Mirror 序列 |
| `BreakRoutine` | 188 | — | 🔴 | 无破碎/Badeline 序列 |
| `Update`/`Render` | 112/281 | lib.rs:36,39 | 🔴 占位 | 仅画半透明玻璃 + 框 |

---

## `Reflection`(CS06_Reflection / ReflectionFG / BigWaterfall / ReflectionHeartStatue / ReflectionTentacles) ↔ `plugins/reflection/src/lib.rs`
### 多实体插件（`bigWaterfall`/`finalBoss`/`finalBoss*Block`/`reflectionHeartStatue`/`tentacles`）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 反射玩家逻辑（CS06_Reflection） | — | — | 🔴 | 无 |
| 大瀑布（BigWaterfall） | — | lib.rs:127 | 🟡 | 蓝色矩形 |
| finalBoss 移动块巡逻 | — | lib.rs:96-115 | 🟠 | finalBossMovingBlock 沿 nodes 移动 |
| reflectionHeartStatue/tentacles | — | lib.rs:119-133 | 🟡 | 仅色块（statue solid、tentacles 非 solid） |

---

## `OldSite`(Payphone.cs) ↔ `plugins/old-site/src/lib.rs`
### `payphone` 实体（原版 `Payphone.cs`）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 构造/交互触发 | — | lib.rs:23 | 🟡 | 装饰矩形（原版触发过场对话，未移植） |
| `Update`/`Render` | — | lib.rs:33,36 | 🔴 占位 | 仅绘制 |

---

## `ForsakenCity`(ForsakenCitySatellite / memorialTextController) ↔ `plugins/forsaken/src/lib.rs`
### 多实体插件
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `birdForsakenCityGem`（携宝石鸟） | — | lib.rs:66-76 | 🟡 | 仅绘制鸟+宝石；无过场 |
| `memorialTextController`（隐形触发） | — | lib.rs:71 | 🔴 | 不绘制，无逻辑 |

---

## `LostLevels`* ↔ `plugins/lostlevels/src/lib.rs`
### 多实体插件（16 类：eyebomb/lightning/floatySpaceBlock/glider/...）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| 危险体（eyebomb/lightning） | — | lib.rs:143-152 | 🟠 | 重叠玩家则 `die()`；无原版攻击逻辑 |
| floatySpaceBlock 巡逻 | — | lib.rs:128-142 | 🟠 | 沿 nodes 移动 + platform |
| lightningBlock/crumbleWallOnRumble | — | lib.rs:108-114 | 🟠 | solid 矩形 |
| 其余（birdPath/glider/kevins_pc/...） | — | lib.rs:162-177 | 🟡 | 仅 PROP 色块绘制，无行为 |

---

## `Hahaha.cs` ↔ `plugins/hahaha/src/lib.rs`
### Class `Hahaha`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Hahaha(pos,ifset,...)` 构造 | 59 | lib.rs:27 | 🟡 | 仅建矩形标记 |
| `Added`/`Update` (_enabled 触发笑声) | 76/85 | lib.rs:40 (空) | 🔴 占位 | 无 |
| `Render` (ha 精灵) | 128 | lib.rs:43-45 | 🔴 占位 | 仅红方块标记 |

---

## `CelestialResort`*(Trapdoor / Clutter* / FriendlyGhost / ...) ↔ `plugins/celestial-resort/src/lib.rs`
### 多实体插件
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Trapdoor.Open` (玩家在上则开) | Trapdoor.cs:27 | lib.rs:131-142 | 🟠 | trapdoor：玩家重叠→solid(false)，离开→solid(true)；无 open 音效/从下开 |
| `ClutterCabinet`/`ClutterDoor` | ClutterCabinet.cs 等 | lib.rs:180-181 | 🟡 | 木箱/道具矩形 |
| `FriendlyGhost` | FriendlyGhost.cs | lib.rs:182-186 | 🟡 | 鬼剪影 |
| `clothesline`/`blockField`/`oshirodoor`/`picoconsole`/`resortmirror` | — | lib.rs:161-196 | 🟡 | 仅装饰矩形 |
| `Update`/`Render` | — | lib.rs:125-197 | 🟠 | 仅 trapdoor 有逻辑，其余静态 |

---

## `TheoCrystal.cs` ↔ `plugins/theo-crystal/src/lib.rs`
### Class `TheoCrystal` (Holdable)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `TheoCrystal(pos)` 构造 | 47 | lib.rs:117 | 🟠 | platform 矩形 + `nodes` 巡逻 ✅ |
| `Added` (去重/教程) | 80 | — | 🔴 | 无 |
| `Update` (重力/抓握/撞墙/ temple gate) | 99 | lib.rs:140-247 | 🟠 | 3 态机 (Idle→Held→Thrown) ✅；Idle 沿 nodes 巡逻 (speed 35) ✅；玩家重叠 + CLIMB 按下 → Held ✅；Held 每帧发 `EV_CARRIED 1 (carryX,carryY)` 冻结玩家（`CARRY_OFFSET_Y=-12`）✅；JUMP/DASH/松开 CLIMB → Thrown + 发 `EV_CARRIED 0` 释放 ✅；Thrown 速度 200 + 0.3s 摩擦衰减回 Idle ✅；`another_holder_has_player` 用事件总线防止重复抓取 ✅；缺 Holdable/碰撞/死亡/Shatter/OnPickup 细粒度回调/temple gate |
| `Shatter`/`Die`/`OnPickup`/`OnRelease` | 239/458/438/444 | — | 🔴 | 全部缺失 |
| `draw` | — | lib.rs:268-276 | 🟡 | 蓝色晶体矩形 ✅ |
| 5 个 unit tests | — | lib.rs:279-313 | 🟢 | `carry_offset_matches_player_cs`/`throw_constants_match_theocrystal_cs`/`approach_clamps_to_target`/`state_constants_are_distinct`/`default_state_is_idle` 全过 |

**小结**：theo-crystal 已接入 `holdable` 事件总线（Idle→Held→Thrown 抓取/抛出状态机 + `EV_CARRIED` 冻结玩家），补上 5 个 unit tests；缺失 Holdable 碰撞/死亡/Shatter/OnPickup 细粒度回调/temple gate。

---

## `Gondola.cs` ↔ `plugins/gondola/src/lib.rs`
### Class `Gondola`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Gondola(data,offset)` 构造 | 63 | lib.rs:29 | 🟠 | Solid 64x8→platform 矩形；节点解析 |
| `Added` (rope/cliffs/back) | 89 | — | 🔴 | 无缆绳/崖壁子实体 |
| `Update` (摇摆 Rotation) | 125 | lib.rs:46-71 | 🟡 | 仅沿 nodes 巡逻（speed 40）；无摆动物理 |
| `BreakLever`/`PullSides` | 169/187 | — | 🔴 | 无 |

---

## `Decorations`* ↔ `plugins/decorations/src/lib.rs`
### 装饰聚合插件（bird/bonfire/cliffflag/cobweb/debris/flutterbird/hanginglamp/lamp/lightbeam/resortLantern/soundSource/SummitBackgroundManager/torch/towerviewer/wire）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Bonfire`（见上） | — | bonfire.rs | 🟠 | 已单独对比 |
| `Bird`/`FlutterBird`（Bird.cs） | — | bird.rs/flutterbird.rs | 🟡 | 程序化鸟/微动；无原版飞行 AI |
| `Torch`（Torch.cs） | — | torch.rs | 🟡 | 火焰矩形；无光照 |
| `Cobweb`/`Debris`/`Lamp`/`HangingLamp`/`LightBeam`/`ResortLantern`/`SoundSource`/`SummitBackgroundManager`/`Towerviewer`/`Wire` | — | 各模块 | 🔴/🟡 | 纯装饰矩形/线，无碰撞与逻辑；dispatch 见 lib.rs:113-184 |

---

## `MirrorTemple`*(TempleGate / TempleEye / DashSwitch / Seeker / ...) ↔ `plugins/mirror-temple/src/lib.rs`
### 多实体插件（14 类）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `TempleEye`/`PlayerSeeker` 追逐+致死 | — | lib.rs:150-166 | 🟠 | 向玩家移动并 `die()`；无原版 seeker AI |
| `DashSwitchH/V` 发事件 | — | lib.rs:167-173 | 🟠 | 玩家重叠发 `event::DASH_BLOCK` |
| `TempleGate` 开/关 | — | lib.rs:174-179 | 🟠 | 收到 `SWITCH` 事件则开（solid 取反）；无原版动画 |
| `TempleCrackedBlock` 被 dash 破 | — | lib.rs:180-185 | 🟠 | 收到 `DASH_BLOCK` 则 remove |
| `ConditionBlock`/`SeekerBarrier`/`SeekerStatue`/`TheoCrystalHoldingBarrier`/`TheoCrystalPedestal`/`TempleCrackedBlock` | — | lib.rs:108-118 | 🟠 | solid 矩形 |
| `TempleMirror`/`TempleMirrorPortal`/`TempleBigEyeball` | — | lib.rs:196-229 | 🟡 | 仅色块，无镜面/眼逻辑 |
| `TempleFallTrigger` → `StTempleFall` 触发 | — | lib.rs:188-198 | 🟠 | 新增：玩家重叠发 `EV_TEMPLE_FALL`（空 payload）；player `temple_fall_update` 进入脚本坠落（重力 + 1.5s 超时恢复）；原版由 Level 脚本（特定房间机关）驱动，非独立 trigger 实体 |

<!-- ========== SECTION c7 ========== -->

# 助推 / 羽毛 / 撞针实体插件 (badeline-boost / infinite-star / big-spinner)

> 说明：三个 Rust 插件均为"逻辑重写"，图形（sprite/wiggler/light/bloom）几乎全部改为
> 程序化绘制或省略，音频/粒子/震屏/位移(displacement)等副作用大多缺失。对比聚焦物理与行为。

## `BadelineBoost.cs` ↔ `plugins/badeline-boost/src/lib.rs`
### Class `BadelineBoost`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `BadelineBoost(...)` ctor | BadelineBoost.cs:48 | lib.rs:100 | 🟡 | 读取 nodes（节点链）✅；碰撞体原版 `Circle(16)`（直径 32），我方 `20×20` 盒 (lib.rs:106) 略有差异；sprite/stretch/light/bloom/wiggler 全部省略 |
| `Awake` (FakeWall→Depth) | BadelineBoost.cs:81 | — | 🔴 | 缺失：未处理 FakeWall 时改 Depth |
| `OnPlayer` | BadelineBoost.cs:90 | lib.rs:145 (Idle 碰撞检测) | 🟠 | 我方在 Idle 检测到重叠即进入 Grab，无 PlayerCollider 回调壳；未区分 `canSkip` 之外的入口 |
| `BoostRoutine` (抓取序列) | BadelineBoost.cs:95 | lib.rs:126-251 | 🟠 | 节点递进 `nodeIndex++` ✅；Grab 0.2s 插值用 `EV_CARRIED` 冻结玩家 ✅；Hold 0.1s ✅；但无 BadelineDummy、无相机 `ZoomTo`/`TimeRate=0.5`、无音频、无 `Drop`/`Dummy*` 状态、无 `endLevel`(Ch9 终结) |
| `nodeIndex++` / finalBoost | BadelineBoost.cs:99,103 | lib.rs:157,198 | ✅ | 最终节点判定对齐 |
| Dashes/Stamina 处理 | BadelineBoost.cs:144-152 | lib.rs:1631-1632 | 🟠 | 原版：若 `Inventory.Dashes>1` 置 `Dashes=1` 否则 `RefillDash`，再 `Alarm` 后 `+1`；我方统一 `dashes=MAX_DASHES`（多补满，行为不完全一致） |
| 抓取插值 `p+=dt/0.2` | BadelineBoost.cs:167-180 | lib.rs:174-190 | ✅ | 0.2s 线性插值把玩家移到 boost 上方；原版对 X/Y 分 MoveTo，我方直接 set 位置（`EV_CARRIED` 每帧驱动） |
| `player.MoveV(5f)` | BadelineBoost.cs:195 | lib.rs:191-196 | 🟠 | Hold 结束解除 `EV_CARRIED`，但原版额外 `MoveV(5)` 微移未实现 |
| 非最终节点 `BadelineBoostLaunch`+节点旅行 | BadelineBoost.cs:224-265 | lib.rs:202-228,240-245 | 🟠 | 我方发 `EV_BADELINE_BOOST(final=false)`→`ST_LAUNCH`（X 方向趋近 boost 中心 ✅）；节点旅行以 **恒定 320 px/s 线性**移动，原版为 `Tween SineInOut` 缓动且时长 `dist/320`(上限 3s) 🟠；无 stretch 拉伸绘制、无 relocate 音频、无 Rumble、无 DirectionalShake、无位移 burst |
| 最终节点 `SummitLaunch`+`Finish` | BadelineBoost.cs:266-287,369 | lib.rs:200-201,247-251 | 🟡 | 我方发 `EV_BADELINE_BOOST(final=true)`→`ST_SUMMIT_LAUNCH`（竖直 -240 ✅）；随后 `host::collect(id)` 即 `Finish` 消失 ✅；但缺失 `Engine.FreezeTimer`、`Flash`、`ResetZoom`、`TimerHidden` 等 |
| `Skip` (canSkip 跳过) | BadelineBoost.cs:290 | lib.rs:128-143 | 🟠 | 玩家在右侧 100px 外跳到下一节点 ✅；但无 stretch 动画与 relocate 音频副作用 |
| `Wiggle` | BadelineBoost.cs:334 | — | 🔴 | 缺失（纯视觉/音频） |
| `Update` (ambience/sprite 跟随) | BadelineBoost.cs:341 | lib.rs:262 (draw) | 🟠 | 我方仅在 draw 时脉冲；无 `P_Ambience` 粒子、无 sprite 朝玩家位移、无 `canSkip` 伴随逻辑（见 Skip） |

## `FlyFeather.cs` ↔ `plugins/infinite-star/src/lib.rs`
### Class `FlyFeather`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
| `FlyFeather(...)` ctor | FlyFeather.cs:46 | lib.rs:60 | 🟡 | 碰撞体 `Hitbox(20,20,-10,-10)` ✅；`shielded`/`singleUse` 读取 ✅；sprite/wiggler/bloom/light/sine/outline/shieldRadiusWiggle/moveWiggle 全部省略（改为程序化绘制） |
| `Added` (存 level) | FlyFeather.cs:77 | — | 🔴 | 缺失：我方直接走 host API，无缓存 level |
| `Update` (respawn/UpdateY) | FlyFeather.cs:83 | lib.rs:77-90 | 🟠 | respawn 倒计时→复活 ✅；`UpdateY` 的 sine 浮动 `sin*2` 在 draw 近似 (lib.rs:165) 🟡；light/bloom alpha 渐变缺失 |
| `UpdateY` (bob/moveWiggle) | FlyFeather.cs:121 | lib.rs:165 | 🟡 | 仅保留 sine 浮动，无 `moveWiggle` 推离效果 |
| `Render` (shield 圈) | FlyFeather.cs:99 | lib.rs:180-196 | 🟠 | 护盾环以多边形近似绘制 ✅；但半径 `10 - wiggle*2` 改为 `10 - sin*1.5` |
| `Respawn` | FlyFeather.cs:108 | lib.rs:82-86 | 🟠 | 复活显示+可碰撞 ✅；无 wiggler/音频/粒子 |
| `OnPlayer` (shielded PointBounce) | FlyFeather.cs:130-142 | lib.rs:102-103,131-156 | 🟡 | 原版用 `player.DashAttacking` 判定，我方用 `speed>240` 近似 (lib.rs:125) 🟡；`PointBounce` 近似 (lib.rs:131)：原版 `Speed=vec*200; Speed.X*=1.5+sign*20`，我方 `220*1.5` 且 `|vx|<100` 保底，Y 的 `-0.2≤Y≤0.4` 钳制一致 ✅；不消耗羽毛 ✅；无音频/Rumble |
| `OnPlayer` → `StartStarFly` | FlyFeather.cs:143-161 | lib.rs:104-117 | 🟠 | 发 `EV_STARFLY` ✅；`flag`（已在飞行中）决定音频的区分缺失；无音频；`singleUse` 收集/3s 复活对齐 ✅ |
| `CollectRoutine` | FlyFeather.cs:164 | lib.rs:104-117 | 🟡 | 隐藏羽翼+复活/收集逻辑对齐；但缺失 `level.Shake`、`P_Collect` 粒子、`SlashFx.Burst`、方向计算 |

## `Bumper.cs` ↔ `plugins/big-spinner/src/lib.rs`
### Class `Bumper`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
| `Bumper(...)` ctor | Bumper.cs:47 | lib.rs:75 | 🟡 | 碰撞体原版 `Circle(12)`→我方 `24×24` 盒 (lib.rs:83) 🟡；`node` 振荡 tween `CubeInOut`/`MoveCycleTime=1.8181819` 在 update 复刻 ✅；sprite/spriteEvil/light/bloom/hitWiggler 省略 |
| `Added` (CoreMode→fireMode) | Bumper.cs:95 | lib.rs:90 | 🟠 | 原版按 `Session.CoreMode==Hot` 决定 fireMode；我方读 `fireMode` 属性 (lib.rs:90)，不随关卡核心模式切换 |
| `OnChangeMode` (CoreMode 监听) | Bumper.cs:103 | — | 🔴 | 缺失：无 CoreModeListener，fireMode 静态 |
| `UpdatePosition` (sine 浮动) | Bumper.cs:110 | lib.rs:121 | 🟠 | 我方仅取 anchor，缺失 `sine.Value*3` / `sineOverTwo*2` 的悬浮抖动 |
| `Update` (respawn/ambience) | Bumper.cs:115 | lib.rs:95-122 | 🟠 | respawn 倒计时 ✅；但缺失复活时的 `sprite.Play("on")`、light/bloom 恢复、音频、`P_Ambience`/`P_FireAmbience` 粒子发射 |
| `OnPlayer` (fireMode 致死) | Bumper.cs:146-158 | lib.rs:139-140 | 🟡 | 发 `die()` ✅；缺失 `SaveData.Assists.Invincible` 豁免检查 🟡；无 hitWiggler/音频/粒子 |
| `OnPlayer` (ExplodeLaunch) | Bumper.cs:159-178 | lib.rs:141-152 | 🟠 | 发 `EV_LAUNCH`（远离圆心方向）✅；冷却 `respawnTimer=0.6` 对齐 ✅；但原版 `player.ExplodeLaunch` 带向上偏置/不纯径向，我方为纯径向归一化 🟡；缺失 `sprite.Play("hit")`、light/bloom 关闭、`DirectionalShake`、`Displacement` burst、`P_Launch` 粒子、区域 9 音频区分 |

### 玩家侧事件消费对照（`plugins/player/src/lib.rs`）
- `EV_BADELINE_BOOST` (lib.rs:1626)：非最终→`ST_LAUNCH` 带 X 趋近 (launch_update lib.rs:985)，最终→`ST_SUMMIT_LAUNCH` 竖直 -240 (lib.rs:1013)；速度 `BADELINE_LAUNCH_SPEED`（应为 -330，`StLaunch` 仅做重力回落后衰减）✅ 行为框架对齐。
- `EV_CARRIED` (lib.rs:1645)：冻结玩家于 carry 目标 ✅，对应原版 `StateMachine.State=11` + `DummyGravity=false`。
- `EV_STARFLY` (lib.rs:1611)：进入 `ST_STARFLY` (starfly_update lib.rs:1026) ✅。
- `EV_LAUNCH` (lib.rs:1538)：`ExplodeLaunch` 径向弹出，全补 dash/stamina ✅。
- `EV_REFILL` (lib.rs:1510)：羽毛护盾弹反后的补满 ✅。
- 缺失：原版 boost 序列里的 `BadelineDummy` 视觉替身、相机 zoom/`TimeRate`、Ch9 终局与 `endLevel` 注册，三部分均未见对应实现。

<!-- ========== SECTION c8 ========== -->

# 关卡 / 背景 / 音频 / 界面层 (Level/Backdrop/Audio/Main/Screens)

> 比较范围：原始 Celeste（C#）的 level/backdrop/autotiler/audio/main/screens 类，对照 Ruleste 的 Rust 实现。
> 状态：✅ 已实现且对齐 / 🟠 部分实现 / 🟡 近似 / 🔴 缺失 / 🔴 占位。

---

## `Level.cs` ↔ `src/engine/level.rs` (+ `src/main.rs` 主循环)
### Class `Level`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Begin()` | Level.cs:327 | main.rs:97 (main) | 🟡 | 原版场景初始化；Rust 在 main 中加载资源并进入循环，无场景抽象 |
| `LoadLevel(IntroTypes,bool)` | Level.cs:357 | level.rs:97 `Level::load` / `from_bin` | 🟠 | 解析房间、实体、拼接 solid/bg 网格；未实现 intro 类型、相机过场 |
| `UnloadLevel()` | Level.cs:1389 | — | 🔴 | 无显式卸载（RAII） |
| `Reload()` | Level.cs:1400 | main.rs:358 `reload_plugins` | 🟡 | 仅重载 wasm 插件，非整关重载 |
| `TransitionTo(LevelData,Vector2)` | Level.cs:1486 | main.rs:387-452 房间切换 | 🟠 | 按玩家世界坐标落入房间矩形/最近房间切换，无 wipe |
| `UnloadEntities(List<Entity>)` | Level.cs:1642 | wasm_host 房间切换 despawn | 🟠 | 由 `enter_room` 卸载上一房间实体 |
| `GetSpawnPoint(Vector2)` | Level.cs:1655 | main.rs:250-277 选最近 player 标记 | 🟠 | 仅用于 Madeline 单次生成 |
| `GetFullCameraTargetAt(Player,Vector2)` | Level.cs:1655 | camera.rs `target_at` | 🟡 | 相机目标计算近似 |
| `TeleportTo(Player,string,IntroTypes,?)` | Level.cs:1675 | — | 🔴 | 关卡传送未实现 |
| `AutoSave()` / `IsAutoSaving()` | Level.cs:1703/1711 | — | 🔴 | 无存档系统 |
| `UpdateTime()` | Level.cs:1726 | — | 🔴 | 计时器未实现 |
| `Update()` | Level.cs:1748 | main.rs:336 循环体 | 🟠 | 事件→input→wasm update→房间切换→相机→渲染 |
| `BeforeRender()`/`Render()`/`AfterRender()` | Level.cs:2010/2026/2112 | renderer.rs:491-509 | 🟠 | 背景→bg tiles→solids→实体→前景，无光照/泛光/位移 |
| `Pause(int,...)` | Level.cs:2140 | screens.rs:110 `UiState::Pause` | 🟡 | 仅 UI 状态，无 gameplay 暂停冻结细节 |
| `Shake`/`StopShake`/`DirectionalShake` | Level.cs:2616/2625/2630 | — | 🔴 | 无相机震动 |
| `Flash(Color,bool)` | Level.cs:2640 | — | 🔴 | 无闪屏 |
| `ZoomSnap`/`ZoomTo`/`ZoomBack`/`ResetZoom` | Level.cs:2651-2697 | — | 🔴 | 无相机缩放 |
| `DoScreenWipe` | Level.cs:2704 | — | 🔴 | 无擦除转场 |
| `StartCutscene`/`SkipCutscene`/`EndCutscene` | Level.cs:2873-2937 | — | 🔴 | 无过场系统 |
| `CompleteArea(...)` | Level.cs:2982 | screens.rs:337 `ChapterComplete` | 🟡 | 仅渲染结算屏，无 wipe/完成逻辑 |

---

## `LevelData.cs` ↔ `src/engine/level.rs` (`Room`)
### Class `LevelData`（单房间数据）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `LevelData(BinaryPacker.Element)` | LevelData.cs:113 | level.rs:103 `from_bin` 逐房间解析 | 🟠 | 解析 x/y/width/height/cameraOffset/实体；缺 music/ambience/wind/dark/space/层参数 |
| `CreateEntityData(BinaryPacker.Element)` | LevelData.cs:321 | level.rs:140 `attrs_to_map` | 🟠 | 转 MapData；未区分 triggers/bgDecals/fgDecals |
| `Check(Vector2)` | LevelData.cs:390 | — | 🔴 | 点在房间内判定未独立实现（main 内联矩形测试）|

> 备注：原版 `LevelData` 还维护 `Spawns/Entities/Triggers/BgDecals/FgDecals`；Rust `Room`(level.rs:23) 仅 `entities`/`decorations`（decoration 列表为硬编码子集，level.rs:65）。

---

## `Session.cs` ↔ 无等价实现
### Class `Session`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `Session(AreaKey,...)` | Session.cs:176 | — | 🔴 | 无 session/进度对象 |
| `GetFlag`/`SetFlag`/`GetLevelFlag` | Session.cs:261/266/324 | — | 🔴 | 无 flag 系统（影响 backdrop 可见性等） |
| `GetCounter`/`SetCounter`/`IncrementCounter` | Session.cs:278/290/307 | — | 🔴 | 无计数器 |
| `Strawberries`/`Keys`/`DoNotLoad` | Session.cs:41/45/43 | — | 🔴 | 无收集状态 |
| `Restart(string)` | Session.cs:234 | — | 🔴 | 无重开 |

---

## `MapData.cs` (`CreateBackdrops`/`ParseBackdrop`) ↔ `src/engine/backdrops.rs`
### `CreateBackdrops` / `ParseBackdrop`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `CreateBackdrops(Element)` | MapData.cs:318 | backdrops.rs:47 `parse` | 🟠 | 处理 `Backgrounds`/`Foregrounds` 与 `apply` 继承 |
| `ParseBackdrop(Element,Element)` | MapData.cs:345 | backdrops.rs:78 `parse_backdrop` | 🟠 | 仅 `parallax`；忽略 snow/stars/tentacles 等 shader 类型（仅 eprintln 跳过）|

---

## `Backdrop.cs` ↔ `src/engine/backdrops.rs` (`Backdrop`)
### Class `Backdrop`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Backdrop()` | Backdrop.cs:99 | backdrops.rs:13 struct | 🟡 | 字段为 position/scroll/speed/color/alpha/flip/loop/additive |
| `IsVisible(Level)` | Backdrop.cs:104 | — | 🔴 | 无 flag/dream/OnlyIn/ExcludeFrom 可见性 |
| `Update(Scene)` | Backdrop.cs:137 | backdrops.rs:37 `Backdrop::update` | 🟠 | 仅 `Position += Speed*dt`，无 Wind、fadeIn、Visible 过渡 |
| `BeforeRender`/`Render`/`Ended` | Backdrop.cs:157/161/165 | renderer.rs:153 `draw_backdrops` | 🟡 | 实际绘制在 renderer，无虚函数分派 |

---

## `Parallax.cs` ↔ `src/interface/renderer.rs` (`draw_backdrops`)
### Class `Parallax : Backdrop`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Parallax(MTexture)` | Parallax.cs:21 | backdrops.rs:131 构造 | 🟡 | 纹理经 `upload_backdrop`(renderer.rs:124) 裁出 |
| `Update(Scene)` | Parallax.cs:27 | backdrops.rs:37 / main.rs:459 漂移 | ✅ | 行为对齐（speed 漂移） |
| `Render(Scene)` | Parallax.cs:42 | renderer.rs:153 `draw_backdrops` | 🟠 | camera 锚定+scroll+loop+color/alpha+additive+flip 已对齐；缺 FadeX/FadeY/`WindMultiplier`、fadeIn 渐变 |

---

## `BackdropRenderer.cs` ↔ 无直接等价（由 `Level.backgrounds/foregrounds` + renderer 承担）
### Class `BackdropRenderer : SceneRenderer`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Backdrops` 列表 | BackdropRenderer.cs:12 | level.rs:56/58 `backgrounds`/`foregrounds` | 🟡 | 列表存放于 Level |
| `BeforeRender`/`Update`/`Render`/`Ended` | BackdropRenderer.cs:20/28/96/36 | renderer.rs:153/493/508 | 🟡 | 分别绘制前后景，无独立 renderer 类 |
| `Get<T>`/`GetEach<T>`/`Remove<T>` | BackdropRenderer.cs:44/56/126 | — | 🔴 | 无按类型查询 backdrop |

---

## `Autotiler.cs` ↔ `src/engine/autotiler.rs`
### Class `Autotiler`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Autotiler(string)` | Autotiler.cs:80 | autotiler.rs:60 `load` | 🟠 | 从 ForegroundTiles.xml 解析（roxmltree） |
| `ReadInto(TerrainType,Tileset,Xml)` | Autotiler.cs:114 | autotiler.rs:86/154 `parse_masks` + copy | 🟠 | 支持 `copy` 继承与 center/padding |
| `GenerateMap(VirtualMap,bool)` | Autotiler.cs:194/204 | autotiler.rs:296 `generate` | 🟠 | 3×3 邻接掩码匹配，按 wildcard 排序 |
| `GenerateBox(char,int,int)` | Autotiler.cs:209 | autotiler.rs:281 `generate_box` | ✅ | 单 id 实心盒，对齐 |
| `GenerateOverlay(...)` | Autotiler.cs:214 | — | 🔴 | 未实现 overlay 生成 |
| `Generate(...)` (TileHandler/CheckTile/CheckForSameLevel) | Autotiler.cs:224/288/353/341 | autotiler.rs:206/237/256 `adjacency`/`neighbor_connects`/`connects_id`/`mask_matches` | 🟠 | EdgesExtend(越界 clamp)、IGNores(`*`)、padding-vs-center(2-away) 已覆盖；缺 `Behaviour` 变体 |
| `IsEmpty(char)` | Autotiler.cs:411 | — | 🟡 | 由 `tile_id_at_local == '0'` 隐式处理 |

---

## `Audio.cs` ↔ `src/engine/audio.rs` (`AudioBus`, 软件 PCM 混音器)
### Class `Audio`（FMOD.Studio 封装）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Init()` | Audio.cs:166 | audio.rs:77 `AudioBus::new` | 🟠 | 用 SDL3 AudioStream 替代 FMOD 系统初始化 |
| `Update()` | Audio.cs:196 | audio.rs:194 `AudioBus::update` | ✅ | 每帧混音并喂设备，语义对齐 |
| `Play(string)` / 多参重载 | Audio.cs:242-291 | audio.rs:131 `play` | 🟠 | 按 FMOD/FSB5 流名播放；无 3D 位置/多 param |
| `Loop(string,...)` | Audio.cs:310-341 | audio.rs:131 `play(..,looping=true)` | 🟠 | 循环语义对齐 |
| `Pause`/`Resume`/`Position`/`SetParameter` | Audio.cs:352/360/368/389 | — | 🔴 | 无单实例暂停/位置/参数 |
| `Stop(EventInstance,bool)` | Audio.cs:397 | audio.rs:159 `stop` / `stop_all` | 🟠 | 按名停止全部匹配 voice |
| `CreateInstance`/`GetEventDescription`/`ReleaseUnusedDescriptions` | Audio.cs:406/422/443 | — | 🔴 | 无 FMOD 事件描述缓存 |
| `SetMusic`/`SetAmbience`/`SetAltMusic`/`SetMusicParam` | Audio.cs:596/619/648/640 | — | 🔴 | 无音乐/环境轨状态机 |
| `BusPaused`/`BusMuted`/`BusStopAll`/`VCAVolume` | Audio.cs:489/503/517/525 | audio.rs:174 `set_master_gain` | 🟡 | 仅全局 master gain，无总线/层 |
| `CreateSnapshot`/`ResumeSnapshot`/`...` | Audio.cs:542-587 | — | 🔴 | 无 snapshot |

---

## `AudioState.cs` ↔ 无等价
### Class `AudioState` / `AudioTrackState`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `AudioState(music,ambience)` | AudioState.cs:30 | — | 🔴 | 无音轨状态 |
| `Apply(bool)` | AudioState.cs:36 | — | 🔴 | 无层/音符 hack 应用 |
| `Stop`/`Clone` | AudioState.cs:68/74 | — | 🔴 | — |

---

## `Celeste.cs` ↔ `src/main.rs`
### Class `Celeste`（主类 / 游戏循环）
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Main(string[])` | Celeste.cs:164 | main.rs:97 `main` | 🟠 | 参数解析、资源加载、进入循环 |
| 游戏主循环 (`Run`/Engine) | Celeste.cs:164 → Engine | main.rs:336 `'running: loop` | 🟠 | 固定 60fps + sleep 限帧；SDL 事件优先处理 Quit/Esc |
| `ReloadAssets`/`ReloadLevels`/`ReloadGraphics`/`ReloadPortraits`/`ReloadDialog` | Celeste.cs:226-250 | — | 🔴 | 无热重载资源（仅插件热重载） |
| `Freeze(float)` | Celeste.cs:152 | — | 🔴 | 无全局冻结 |

---

## `SaveData.cs` ↔ 无等价
### Class `SaveData`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `Start(SaveData,int)` / `GetFilename` | SaveData.cs:206/213 | — | 🔴 | 无存档槽 |
| `AddDeath`/`AddStrawberry`/`CheckStrawberry`/`AddTime` | SaveData.cs:362/370/404/414 | — | 🔴 | 无统计 |
| `RegisterCompletion`/`RegisterHeartGem`/`RegisterCassette`/`RegisterPoemEntry`/`RegisterSummitGem` | SaveData.cs:522/420/496/502/513 | — | 🔴 | 无完成进度 |
| `SetCheckpoint`/`HasCheckpoint`/`GetCheckpoints` | SaveData.cs:566/577/600 | — | 🔴 | 无检查点 |
| `SetFlag`/`HasFlag` | SaveData.cs:628/623 | — | 🔴 | 无 flag 持久化 |
| `BeforeSave`/`AfterInitialize`/`AssistModeChecks` | SaveData.cs:341/249/319 | — | 🔴 | — |

---

## `OuiMainMenu.cs` ↔ `src/interface/screens.rs`
### Class `OuiMainMenu`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Enter`/`Leave` | OuiMainMenu.cs:156/185 | screens.rs:71 `UiState::MainMenu` 分支 | 🟡 | 状态机过渡近似，无协程补间 |
| `CreateButtons`/`OnExit`/`OnOptions`/`OnCredits` | OuiMainMenu.cs:59/296/282/289 | screens.rs:208 `MainMenu::render` + 菜单项 | 🟡 | 仅 CLIMB/Options/Credits/Exit 文本；Exit 为占位回到 Title |
| `Update`/`Render` | OuiMainMenu.cs:222/234 | screens.rs:57 `update` / `MainMenu::render` | 🟠 | 上下选择+确认逻辑对齐；无 Mountain 背景动画 |

---

## `OuiFileSelect.cs` ↔ `src/interface/screens.rs`
### Class `OuiFileSelect`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Enter`/`Leave` | OuiFileSelect.cs:26/139 | screens.rs:93 `UiState::SaveSelect` | 🟡 | 进入/离开为状态切换 |
| `SelectSlot(bool)`/`LoadThread` | OuiFileSelect.cs:178/107 | screens.rs:96-108 左右选槽 | 🟡 | 3 槽导航对齐；无实际读档线程（槽均为 Empty 占位） |
| `Update`/`Render` | OuiFileSelect.cs:204/— | screens.rs:`SaveSelect::render` | 🟡 | 仅渲染占位框 |

---

## `OuiChapterSelect.cs` ↔ `src/interface/screens.rs`
### Class `OuiChapterSelect`
| 原版函数 | 原版行号 | 我方实现 | 状态 | 说明 |
|---|---|---|---|---|
| `Enter`/`Leave`/`AdvanceToNext`/`Update`/`Render`/`EaseCamera`/`PerformCh8Unlock` | OuiChapterSelect.cs:80/132/174/221/307/324/331 | — | 🔴 | 无章节选择界面（地图名直接由命令行传入 main） |

---

## `OuiTitleScreen.cs` ↔ `src/interface/screens.rs`
### Class `OuiTitleScreen`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Enter`/`Leave` | OuiTitleScreen.cs:75/100 | screens.rs:66 `UiState::Title` 分支 | 🟡 | 状态切换 |
| `Update`/`Render` | OuiTitleScreen.cs:126/138 | screens.rs:57 `update` / `TitleScreen::render` | 🟠 | 渲染 "CELESTE" + 提示；无 Mountain 相机/菜单滑入补间 |
| `MountainTarget` / `FadeBgTo` | OuiTitleScreen.cs:11/117 | — | 🔴 | 无山体背景与淡入 |

---

## `OuiCredits.cs` ↔ `src/interface/screens.rs`
### Class `OuiCredits`
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Enter`/`Leave` | OuiCredits.cs:24/40 | screens.rs:135 `UiState::Credits` 分支 | 🟡 | 按 Cancel 返回菜单 |
| `Update`/`Render` | OuiCredits.cs:53/68 | screens.rs:135 分支 | 🟡 | 仅状态，无滚动字幕渲染 |

---

## `OuiPause.cs` ↔ `src/interface/screens.rs`
> 仓库 `references/source/Celeste/Celeste/` 中**不存在** `OuiPause.cs`（原版暂停菜单可能内联于 `Level.Pause` 或 `Oui` 体系）。以下对照 `Level.Pause` + 我方 `UiState::Pause`。

### 暂停菜单
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Level.Pause(int,bool,bool)` | Level.cs:2140 | screens.rs:110 `UiState::Pause` | 🟡 | 仅 UI 菜单（Resume/Restart/Options/Exit to Menu），无 minimap/选项子页/快速重置 |
| `Level` 内 Pause 渲染 | — | screens.rs:302 `PauseMenu::render` | 🟠 | 半透明背景+菜单项近似 |

---

## `OuiIntro.cs` / `OuiEnding.cs` ↔ 无等价
> 仓库 `references/source/Celeste/Celeste/` 中**不存在** `OuiIntro.cs` 与 `OuiEnding.cs`（原版 intro/ending 走 `LevelEnter`/`LevelExit`/cutscene）。

| 原版 | 我方实现 | 状态 | 说明 |
|---|---|---|---|
| `OuiIntro` 序章 | — | 🔴 | 无序章界面 |
| `OuiEnding` 结局 | — | 🔴 | 无结局界面 |

---

## 汇总观察
- **关卡加载**：`Level::from_bin` 对齐了房间解析、cameraOffset、jumpThru、实体/装饰拆分、房间网格拼成整关 solid/bg 网格；缺失 intro 类型、triggers/decals、音乐/风/暗房等房间元数据。
- **背景**：`backdrops.rs` + `renderer.draw_backdrops` 对齐了 parallax 的 scroll/loop/color/alpha/additive/flip 与每帧 speed 漂移；缺失 flag/dream 可见性、FadeX/Y、WindMultiplier、fadeIn。
- **Autotiler**：3×3 掩码、copy 继承、center/padding（2-away）、ignores(`*`)、EdgesExtend 均已对齐；缺 `Behaviour` 变体与 `GenerateOverlay`。
- **音频**：从 FMOD 事件系统完整替换为软件 PCM 混音器（`AudioBus`），仅覆盖按名 Play/Loop/Stop/stop_all/master_gain/update；无 3D、参数、音乐/环境轨、snapshot、总线。
- **主循环**：`main.rs` 固定 60fps 循环对齐了 事件→input→wasm→房间切换→相机→渲染→present，先 poll Quit/Esc；缺擦除/震动/缩放/过场/计时。
- **界面**：`screens.rs` 以 `UiState` 状态机近似了 Title/MainMenu/SaveSelect/Pause/Credits/ChapterComplete/Dialog 的导航与文本渲染；**完全缺失** ChapterSelect、Intro、Ending，且无 Mountain 背景、补间动画、滚动字幕、实际读档/存档。

<!-- ========== SECTION c9 ========== -->

# 补充实体插件 (dream-block / intro-crusher / plateau)

## `DreamBlock.cs` ↔ `plugins/dream-block/src/lib.rs`
### Class `DreamBlock` (继承 `Solid`)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `DreamBlock(pos,w,h,node,fastMoving,oneUse,below)` | DreamBlock.cs:60 | lib.rs:300 (`ruleste_entity_init`) | 🟠 部分 | Depth(-11000/5000)、node、fast/oneUse/below、move_duration 已对齐；但 `SurfaceSoundIndex=11` 未设置，particleTextures 改为按需 `draw_image_color` 路径加载。 |
| `DreamBlock(EntityData,offset)` | DreamBlock.cs:81 | lib.rs:300 | 🟡 近似 | 由宿主 `MapData` 注入替代；读取字段名一致（fastMoving/oneUse/below）。 |
| `Added(scene)` | DreamBlock.cs:86 | lib.rs:320 | 🟠 部分 | 原版从 `Session.Inventory.DreamDash` 读；我方改为 update 中监听 `DREAM_DASH_GRANTED` 事件（lib.rs:348）。移动 Tween 在 `Active` 阶段手动模拟（lib.rs:432）。`LightOcclude` 未实现。 |
| `Setup()` | DreamBlock.cs:120 | lib.rs:179 (`make_particles`) | ✅ 已对齐 | 粒子数量公式 `(w/8*h/8*0.7)`、Layer 分布(0:1/6,1:2/6,2:3/6)、TimeOffset、Layer 颜色表一致；用确定性 LCG 替代 `Calc.Random` 以抗热重载。 |
| `OnPlayerExit(player)` | DreamBlock.cs:147 | — | 🔴 缺失 | 未由宿主回调触发；oneUse 退出逻辑改在 `Active` 阶段用 `player_was_inside` 近似（lib.rs:471）。`Dust.Burst` 未实现。 |
| `OneUseDestroy()` | DreamBlock.cs:174 | lib.rs:477 (`DreamPhase::Breaking`) | 🟠 部分 | 我方用白填充淡出后 `host::remove`（lib.rs:481），未做 `DisableStaticMovers`。 |
| `Update()` | DreamBlock.cs:181 | lib.rs:374 (`match st.phase`) | 🟠 部分 | animTimer、wobbleEase、MoveTo 节点移动、BlockedCheck 触发已对齐；`SurfaceSoundIndex=12` 未设置。 |
| `BlockedCheck()` | DreamBlock.cs:198 | lib.rs:490 (`blocked_check`) | ✅ 已对齐 | 检查 Theo/Player 碰撞并尝试上推，返回阻塞布尔值一致。 |
| `TryActorWiggleUp(actor)` | DreamBlock.cs:213 | lib.rs:507 / :531 | ✅ 已对齐 | 1..=4px 上推逻辑一致（每 px `Collision::check`）。 |
| `Render()` | DreamBlock.cs:230 | lib.rs:545 (`ruleste_entity_draw`) | 🟠 部分 | 背景填充、粒子、四角块、白填充、抖动边框均已对齐；缺失相机视锥剔除（lib.rs:570 假设 cam=0），`whiteFill` 混合进边框线颜色未做。 |
| `PutInside(pos)` | DreamBlock.cs:284 | lib.rs:578 | ✅ 已对齐 | 宽/高取模回卷一致。 |
| `WobbleLine(...)` | DreamBlock.cs:305 | lib.rs:235 (`draw_wobble_line`) | ✅ 已对齐 | 16px 步进、双背线 + 主线、振幅 `LineAmplitude` 插值一致。 |
| `LineAmplitude(seed,index)` | DreamBlock.cs:336 | lib.rs:228 (`line_amplitude`) | ✅ 已对齐 | 公式一致（含 2π 项）。 |
| `Lerp(a,b,p)` | DreamBlock.cs:341 | lib.rs:265 (内联) | ✅ 已对齐 | 线性插值。 |
| `Activate()` | DreamBlock.cs:346 | lib.rs:375 (`DreamPhase::Activating` 阶段0-3) | 🟠 部分 | 1s 延迟→白填充 CubeIn→burst→淡出时序对齐；缺失 `Input.Rumble`、`Shaker`(Interval=0.02 随机抖动，我方用正弦近似 lib.rs:393)、`level.ParticlesFG.Emit`、`level.Shake()`。 |
| `ActivateNoRoutine()` | DreamBlock.cs:391 | lib.rs:359 (事件分支) | 🟠 部分 | 置 `playerHasDreamDash`、重生成粒子、移除 occlude 的逻辑已并入事件处理；未显式管理 `Shaker`。 |
| `FootstepRipple(pos)` | DreamBlock.cs:407 | — | 🔴 缺失 | 位移渲染器 Burst 未实现。 |

## `IntroCrusher.cs` ↔ `plugins/intro-crusher/src/lib.rs`
### Class `IntroCrusher` (继承 `Solid`)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `IntroCrusher(pos,w,h,node)` | IntroCrusher.cs:20 | lib.rs:116 (`ruleste_entity_init`) | 🟠 部分 | Depth(-10501)、start/end、solid 对齐；`SurfaceSoundIndex=4` 未设置；`shakeSfx SoundSource` 改为 `play_sound` 即时播放。 |
| `IntroCrusher(EntityData,offset)` | IntroCrusher.cs:31 | lib.rs:116 | 🟡 近似 | 宿主注入；`data.Nodes[0]` 对应 `spawn.get_node(0)`（lib.rs:123）。 |
| `Added(scene)` | IntroCrusher.cs:36 | lib.rs:125 / :140 | 🟠 部分 | 原版据 `Session.GetLevelFlag("1"/"0b")` 直接定位于 end；我方用 `SETTLED` HashSet（lib.rs:59）在 settle 后持久化，重生在 init 直接定位于 node（lib.rs:128）。语义近似但非同一机制。 |
| `Update()` | IntroCrusher.cs:49 | lib.rs:147 (`ruleste_entity_update`) | 🟠 部分 | 触发区间(px∈[x+30,x+w+8])、逃脱判定(px≥x+w-8‖px<x+28)、1.2s 抖动、CubeIn 下落、落地相位流转均已对齐；`tilegrid.Position=shake` 改为 draw 时偏移（lib.rs:213）。 |
| `Sequence()` | IntroCrusher.cs:55 | lib.rs:151 (`match st.phase`) | 🟠 部分 | 阶段0等待→阶段1抖动(1.2s)→阶段2下落(CubeIn, FALL_RATE=2 对应 `Calc.Approach(t,1,2dt)`)→阶段3落定 一致；缺失 `Input.Rumble`、`level.Particles(Emit 落尘/land dust)`、`level.Shake()`、`shakingSfx.Param("release")`、落地后的 0.25s `Shaker`。 |
| （辅助）`player_x` | — | lib.rs:85 | 🟡 近似 | 取玩家中心 x（含 hitbox 偏移），用于触发/逃脱判定。 |
| （辅助）`crush_player_if_overlapped` | — | lib.rs:97 | 🔴 占位 | 原版无显式击杀调用（由物理碰撞处理）；我方主动 `host::die()`，行为近似但属额外实现。 |
| （辅助）`shake_offset` | — | lib.rs:199 | 🟡 近似 | 用 fract 伪随机 ±2px 替代 Monocle `Shaker`。 |
| `draw` | — | lib.rs:208 | ✅ 已对齐 | `draw_tile_box('3', ...)` 对应 `GFX.FGAutotiler.GenerateBox('3',...)`，抖动偏移仅在 phase1 应用。 |

## `Plateau.cs` ↔ `plugins/plateau/src/lib.rs`
### Class `Plateau` (继承 `Solid`)
| 原版函数 | 原版行号 | 我方实现 (文件:行) | 状态 | 说明 |
|---|---|---|---|---|
| `Plateau(e,offset)` | Plateau.cs:12 | lib.rs:33 (`ruleste_entity_init`) | 🟠 部分 | `Solid(104,4)` + `Collider.Left+=8`（hitbox(104,4,8,0) lib.rs:42）、`solid` 对齐；缺失 `SurfaceSoundIndex=23`、`LightOcclude`、`EnableAssistModeChecks=false`。Depth 原版沿用 Solid 默认(0)，我方设为 300（lib.rs:44），与原版不一致。 |
| （渲染）`Image(fallplateau)` | Plateau.cs:16 | lib.rs:51 (`ruleste_entity_draw`) | 🟡 近似 | 原版绘制 `scenery/fallplateau` 贴图；我方仅用 1px 苔绿 + 3px 石灰色块近似（MOSS/STONE），非真实纹理。 |
| `Update()` | — | lib.rs:48 (空) | ✅ 已对齐 | 原版无逻辑，静态实体。 |

### 总体备注
- 三插件均实现 `ruleste_noop_destroy`/`ruleste_noop_serialize`，与原版含 `RemoveSelf`/序列化字段的基本行为一致（复杂序列化如 `node`/状态未在存档层复原）。
- 共通缺失项：**`SurfaceSoundIndex` 未设置**（三个插件均无）、**`LightOcclude` 未实现**（DreamBlock disabled、Plateau）、**`Input.Rumble`/`Level.Shake`/粒子发射**在 dream-block 与 intro-crusher 的演出序列中未还原。
- 事件/标志驱动：dream-block 用 `DREAM_DASH_GRANTED` 事件替代 `Session.Inventory.DreamDash`；intro-crusher 用进程内 `SETTLED` 集合替代 `Session` 关卡标志，需注意跨会话/热重载一致性。
