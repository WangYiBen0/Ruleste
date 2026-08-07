# Ruleste ROADMAP

用 Rust + SDL3 从零复现《Celeste》，架构为“微内核 + Wasm 插件”。本文件是推进路线图；每完成一个阶段后更新对应状态与验证项。

- 代码规范：`cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 必须通过
- 提交信息：Conventional Commits
- 约束：`references/` 仅用于阅读，代码/构建/产物不得依赖
- 实体逻辑一律以 `plugins/` 下的 Wasm 插件实现，核心不得硬编码玩法

## Phase 0 — 项目脚手架 ✅
- [x] `flake.nix`：fenix 固定工具链 + wasm32-unknown-unknown rust-std，`nix develop` 可复现环境（x86_64-linux、aarch64-linux、x86_64-darwin、aarch64-darwin）
- [x] Cargo workspace：`crates/ruleste-plugin-api`、`plugins/player`、`plugins/spring`、根 crate `ruleste`
- [x] 依赖选型：sdl3 0.18（Unix 用 pkg-config、Windows 用 vcpkg、unsafe_textures）、wasmtime 47、roxmltree、bytemuck、anyhow
- [x] `.envrc`（use flake）、.gitignore、Git 仓库初始化
- 验证：`nix develop -c cargo build` 全 workspace 零警告

## Phase 1 — 数据解析 ✅（真实资产回归已完成）
- [x] `.NET` BinaryReader（7-bit varint dotnet string）`src/data/reader.rs`
- [x] `.bin` 地图 BinaryPacker（magic/查表/属性 0–7/RLE innerText）`src/data/binary_packer.rs`
- [x] `.meta` 图集 + `.data` RLE 页纹理解码 `src/data/atlas.rs`
- [x] `Sprites.xml` SpriteBank `src/data/spritebank.rs`
- [x] 用真实资产（`references/Celeste/Content/`）做解析自检：`0-Intro.bin` → 9 实体/4 类型；`Gameplay.meta` → 6824 帧 + RGBA 提取；`Sprites.xml` → player 动画（`src/bin/inspect.rs`）
- [ ] 补充 `.data` 纹理、`Dialog`、字体、存档等其余资源解析
- [ ] 将原版 `Content/` 转换为 `map/`、`resources/` 的工具（官方内容不参与分发）

## Phase 2 — 引擎核心 ✅（待联调）
- [x] ECS `World`/`Entity`（position/speed/hitbox/depth/sprite 状态）`src/engine/ecs.rs`
- [x] 虚拟输入：默认键位镜像 `Settings.SetDefaultKeyboardControls`，边沿检测 `src/engine/input.rs`
- [x] 物理：`SolidGrid`（TILE=8）、`actor_move` 碰撞结算（GROUND/WALL/CEILING 标志）、`entity_collide` `src/engine/physics.rs`
- [x] 关卡：`.bin` → `Level`（solids/bg/实体列表、x/y 补齐）`src/engine/level.rs`
- [x] SpriteAnimator：动画帧推进、loop/goto、子图零填充寻址 `src/engine/sprites.rs`
- [ ] 自动拼接（autotiler）——类似 Monocle 的贴图拼接，替换当前调试色块
- [ ] 相机与房间滚动（跟随玩家、边界）

## Phase 3 — Wasm 宿主与内建插件 🔶 进行中
- [x] Wasm 宿主：`load_plugins`、实体 `spawn/update/draw/despawn`、完整 FFI（position/speed/hitbox/sprite/input/collision/sound/emit）`src/hotload/wasm_host.rs`
- [x] 缓冲协议：插件自导出 `ruleste_alloc/dealloc`，宿主不再猜堆布局
- [x] 热重载：mtime 监听 + `serialize/deserialize` 状态迁移 `src/hotload/watcher.rs`
- [x] 插件 ABI：`ruleste_plugin_meta/entity_types/entity_*`、宏 `ruleste_meta!`/`ruleste_entity_types!`/`ruleste_noop_*` `crates/ruleste-plugin-api`
- [x] 内建 `player` 插件：走动/跑、跳跃（落地缓冲 + 可变跳 + 贴墙跳）、蹲伏、贴墙滑、冲刺；动画名按 `PlayerSprite.cs`；状态可序列化 `plugins/player`
- [x] 实体-实体交互：`host_entities_by_type`/`host_drain_events`/`host_entity_alive` FFI + Spring 插件验证跨插件交互
- [ ] 其余内建插件：`booster`、`dream_block`、`crystal`、`spikes`、`crush` 等（按关卡实体优先级排序）
- [x] 地图实体 → 插件的自动实例化：按 `ruleste_plugin_entity_types` 派发，未覆盖类型告警
- [x] 插件安全：宿主对插件 panic/越界/OOM 的隔离与报错
  - `spawn_entity`: `call_init` trap → 回滚 ECS，日志告警
  - `reload_plugin` serialize/restore 路径：辅助函数 trap → 跳过该实体，继续热重载

## Phase 4 — 渲染与画面 🔶 进行中
- [x] SDL3 渲染器：320×180 逻辑分辨率 ×4 缩放、纹理上传、按 depth 排序绘制、翻转/相机 `src/interface/renderer.rs`
- [ ] 场景框架：标题页 → 主菜单 → 存档选择 → 关卡 → 暂停/死亡/完成
- [ ] 地图渲染：solids 自动拼接、bg 层、水/雾/滤镜等后处理
- [ ] 实体绘制：插件 `ruleste_entity_draw` 驱动的 SpriteBank 帧渲染
- [ ] 音频：FMOD 事件替换为 SDL 音频，`AudioBus` 请求接入

## Phase 5 — Everest 键盘输入
- [ ] 解析 `Celeste.everest.yaml` 的键位/辅助配置
- [ ] 键位覆盖默认绑定；支持 `Pause`、`Confirm`、`Backspace` 等回调语义
- [ ] 与 `src/engine/input.rs` 的绑定模型合并，插件侧 `host_input_*` 无需改动

## Phase 6 — 游戏内地图编辑器
- [ ] 图块拾取/放置/删除，solids/bg 编辑，实时热重载预览
- [ ] 实体放置与参数面板（复用 `MapData` 序列化协议）
- [ ] 保存为 `.bin`（复用 BinaryPacker 编码器）与 `.txt`/XML 互转
- [ ] 进入/退出编辑器状态机，Undo/Redo

## Phase 7 — MiaoNet 联机
- [ ] 调研 `references/MiaoNet` 协议（只读参考），设计状态同步/帧同步取舍
- [ ] 网络层与宿主事件桥接（`GameEvent`/`host_emit` 网络化）
- [ ] 宿主预测 + 回滚；插件状态序列化协议复用
- [ ] 加入/退出房间、延迟补偿、断线恢复

## Phase 8 — 打磨与完整复现
- [ ] 成就/草莓/核心宝石/数字收集，存档读写
- [ ] 金果冻/竹篮、吹风机、风场等机制补齐
- [ ] 对话系统与过场、变奏曲、红心等章节机制
- [ ] 性能：wasmtime 配置、渲染合批、热路径剖析
- [ ] 打包与 CI（nix flake check、wasm 插件构建流水线）
