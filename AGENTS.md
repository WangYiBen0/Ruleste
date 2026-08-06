# AGENTS.md - Ruleste 仓库指引

## 项目目标
Ruleste 是《Celeste》游戏的非官方 Rust 重实现，采用 SDL3 从零构建，不依赖游戏框架。
核心架构为“微内核 + Wasm 插件”，所有游戏实体（非墙壁）均作为 Wasm 插件运行，支持热加载。
目前项目处于早期，目标是完整复现原版游戏行为，同时提供灵活的 mod 能力。

## 技术栈与环境
- 语言：Rust
- 图形/音频/输入：SDL3
- 构建系统：Cargo + Nix flakes（`flake.nix` 提供完全可复现的开发环境）
- 代码规范：必须通过 `cargo fmt` 和 `cargo clippy`（无警告）
- Git 提交：遵循 [Conventional Commits](https://www.conventionalcommits.org/)

## 快速开始
```bash
# 进入开发环境
nix develop

# 构建
cargo build

# 运行游戏（需 X11/Wayland）
cargo run
```

## 架构与目录结构
- `src/` — Rust 核心实现
  - `src/data/` — 资源解析器（.bin 地图、.meta 图集、.data 纹理、对话、字体、存档）
  - `src/engine/` — 引擎层：ECS、事件总线、输入、自动拼接、物理，类似原版的 Monocle 引擎
  - `src/interface/` — 渲染层：标题页面、主菜单、存档选择菜单、暂停菜单、画面
  - `src/hotload/` — Mtime 热重载监视器（地图、对话、Wasm 插件）
- `plugins/` — Wasm 实体插件（`wasm32-unknown-unknown` 目标）
  - 例如 `plugins/player/` 玩家插件，`plugins/booster/` 助推器插件
- `map/` — 地图
- `resources/` — 资源包，包含纹理包、字体包、语言包、音效包等
  - 例如 `{map,resources}/Celeste/` 官方地图与资源包，但是不参与分发，而是提供一个将原版 `Content/` 转换为 `map/resources` 的工具
- `references/` — 参考，只读，gitignored
  - 详见 `references/README.md`

## 关键约束与注意事项
1. **禁止依赖 `references/`**：只能作为开发参考，代码、构建过程、二进制文件不能与之有任何关联。
2. **字符串编码**：所有 `.bin` 地图和 `.meta` 图集使用 .NET 7-bit varint 长度前缀，不是固定 `u32`。
3. **Wasm 插件机制**：所有非墙体实体均为 Wasm 插件。宿主通过 FFI 提供 `get_position`、`set_component` 等组件接口和事件总线。插件导出 `entity_init`, `entity_update(entity_id, dt)`, `entity_draw`。
4. **插件安全性**：热加载/重载 Wasm 时注意状态恢复，避免悬垂指针和不一致问题。
5. **插件互操作**：插件之间可能产生联动，比如 `throwables` 插件可拓展 `player` 插件的操作，而 `theo`、`jellyfish` 插件亦可拓展 `throwables`。
6. **SDL3 环境**：非 Nix 环境下需确保 `PKG_CONFIG_PATH` 包含 `sdl3.pc` 路径。

## AI 代理职责
本仓库期望 AI 协助以下工作：
- 编写与重构 Rust 代码（严格遵循上述架构和约束）
- 撰写与更新文档（模块注释、架构说明、README 等）
- 修复 bug 和性能优化

### 行为守则
- **永远不将游戏逻辑硬编码到核心**，所有实体行为必须通过插件实现。
- 代码须通过 `cargo fmt` 和 `cargo clippy -- -D warnings`。
- 提交信息使用 Conventional Commits 格式。
- 修改 Wasm 宿主 API 时，需同步更新插件示例，确保接口兼容。
- 不得引用或依赖 `references/` 目录下的任何文件。
- 开发环境一律通过 `nix develop` 进入，不得假设系统全局安装了特定库。
- 提出修改时，最好以功能分支或补丁形式呈现，并说明设计动机及对插件接口的影响。
