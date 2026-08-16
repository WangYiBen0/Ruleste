# AGENTS.md - Ruleste 仓库指引

## 项目目标
Ruleste 是《Celeste》游戏的非官方 Rust 重实现，采用 SDL3 从零构建，不依赖游戏框架。
核心架构为“微内核 + Wasm 插件”，所有游戏实体（非墙壁）均作为 Wasm 插件运行，支持热加载。
目前项目处于早期，目标是完整复现原版游戏行为，同时提供灵活的 mod 能力。

## 技术栈与环境
- 语言：Rust
- 图形/音频/输入：SDL3 + fmod-oxide
- 构建系统：Cargo + Nix flakes（`flake.nix` 提供完全可复现的开发环境）
- 代码规范：必须通过 `cargo fmt` 和 `cargo clippy`（无警告）
- Git 提交：遵循 [Conventional Commits](https://www.conventionalcommits.org/)

## 快速开始
```bash
# 进入开发环境（Linux/macOS）
nix develop

# 构建
cargo build

# 运行游戏
cargo run
```

### 平台支持
- **Linux**：SDL3 自动选择 X11 或 Wayland（可通过 `SDL_VIDEO_DRIVER=x11` 或 `wayland` 强制指定）
- **macOS**：SDL3 使用 Cocoa 窗口，Homebrew 安装 `brew install sdl3`，或通过 Nix 开发环境
- **Windows**：SDL3 使用 Win32 窗口，需安装 vcpkg 并 `vcpkg install sdl3`，或使用 `build-from-source` feature 自动构建

## 架构与目录结构
- `src/` — Rust 核心实现
  - `src/data/` — 资源解析器（.bin 地图、.meta 图集、.data 纹理、对话、字体、存档）
  - `src/engine/` — 引擎层：ECS、事件总线、输入、自动拼接、物理，类似原版的 Monocle 引擎
  - `src/interface/` — 渲染层：标题页面、主菜单、存档选择菜单、暂停菜单、画面
  - `src/hotload/` — Mtime 热重载监视器（地图、对话、Wasm 插件）
- `plugins/` — Wasm 实体插件（`wasm32-unknown-unknown` 目标）
  - 例如 `plugins/player/` 玩家插件，`plugins/booster/` 助推器插件
- `maps/` — 关卡包，按 pack 分目录（例如 `maps/Celeste/*.bin`）
- `resources/` — 资源包，按 pack/namespace 分目录：
  - 例如 `resources/Celeste/Celeste/` 为原版 Celeste pack（pack=Celeste、namespace=Celeste）
  - 子目录：`pack.png`、`metadata.json`、`textures/`、`texts/`、`font/`、`audio/`（OGG + manifest）、`mountain/`、`tutorials/`、`effects/`
  - 官方内容不参与分发，提供 `convert-from-celeste-contents.sh` 把原版 `Content/` 转成 pack 布局；FMOD `.bank` 由 `tools/bank-to-ogg.sh` 抽成 `audio/`
- `references/` — 参考，只读，gitignored
  - 详见 `references/README.md`

## 关键约束与注意事项
1. **禁止依赖 `references/`**：只能作为开发参考，代码、构建过程、二进制文件不能与之有任何关联。
2. **字符串编码**：所有 `.bin` 地图和 `.meta` 图集使用 .NET 7-bit varint 长度前缀，不是固定 `u32`。
3. **Wasm 插件机制**：所有非墙体实体均为 Wasm 插件。宿主通过 FFI 提供 `get_position`、`set_component` 等组件接口和事件总线。插件导出 `entity_init`, `entity_update(entity_id, dt)`, `entity_draw`。
4. **插件安全性**：热加载/重载 Wasm 时注意状态恢复，避免悬垂指针和不一致问题。
5. **插件互操作**：插件之间可能产生联动，比如 `throwables` 插件可拓展 `player` 插件的操作，而 `theo`、`jellyfish` 插件亦可拓展 `throwables`。
6. **SDL3 环境**：非 Nix 环境下需确保 `PKG_CONFIG_PATH` 包含 `sdl3.pc` 路径（Windows 使用 vcpkg）。
7. **SDL3 像素格式（本构建的坑）**：软件渲染器下，`SDL_PIXELFORMAT_RGBA8888` 纹理的内存字节序实际是 `A,B,G,R`（与标准 SDL 约定相反）。上传 `(r,g,b,a)` 字节序数据（如 `atlas.rs` 解码的 `page.rgba`）时，纹理必须声明为 `ABGR8888` 才会正确渲染；声明 `RGBA8888` 会导致颜色经变换错乱（如泥土色 (143,86,59) 显示成 (143,33,48)）。此外 `SDL_RenderReadPixels` 返回 `ARGB8888`（内存字节序 `B,G,R,A`）表面，直接按 RGB 读取会得到 R/B 互换的错误颜色。新增像素相关功能时用 `RULESTE_DUMP_FRAME`（PPM dump）验证。
8. **窗口关闭**：`Input::pump` 只处理按键，不消费 `Quit`/`Escape`；主循环先 poll 事件检查退出，再把其余事件喂给 `Input`。
9. **窗口尺寸**：渲染固定于内部 320×180 逻辑分辨率，通过 `set_logical_size(..., LETTERBOX)` 交给 SDL 缩放，窗口必须 `resizable()`。原因：Niri 等平铺合成器会把"固定尺寸"窗口自动浮动；可缩放窗口才会被平铺（已验证：tile 936×1144，内容 16:9 letterbox 居中，`RULESTE_DUMP_FRAME` dump 尺寸随窗口变化）。不要在 core 里手动 `set_scale` 后调整窗口大小，应由 SDL 逻辑呈现处理。

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
