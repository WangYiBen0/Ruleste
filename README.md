# Ruleste

用 Rust + SDL3 从零复现《Celeste》的非官方重实现。核心为“微内核 + Wasm 插件”架构：墙体以外的所有实体（玩家、跳板、草莓、机关……）都作为 `wasm32-unknown-unknown` 目标编译的插件运行，支持热加载。引擎不绑定任何游戏框架，只用 SDL3 处理图形/音频/输入。

> **免责声明**：Ruleste 不分发任何原版资产，也不依赖 FMOD 运行时。`resources/Celeste/` 仅用于自检，需自带原版 `Content/` 并通过转换工具生成；`references/` 在 `.gitignore` 中，仅供开发参考。

## 环境

唯一支持的开发环境入口是 Nix flake，保证跨平台工具链一致：

```sh
nix develop        # 进入带 fenix 工具链 + wasm32-unknown-unknown rust-std 的 shell
```

需要：Rust（fenix 固定版本）、`wasm32-unknown-unknown` target（rust-std）、SDL3。非 Nix 用户需自行安装 `sdl3`（Linux 设好 `PKG_CONFIG_PATH`，Windows 用 vcpkg，macOS 用 Homebrew）。

## 一键构建

仓库根目录的 `./build.sh` 会编译宿主二进制 **和** 全部 32 个 Wasm 插件，并把 `.wasm` 暂存到宿主可执行文件旁的 `plugins/` 目录（即游戏默认插件目录）：

```sh
$ nix develop
[nix-shell]$ ./build.sh            # 默认 debug
[nix-shell]$ ./build.sh --release  # 优化构建
```

`build.sh` 只构建不运行；产物：
- 宿主：`target/debug/ruleste`（或 `target/release/ruleste`）
- 插件：`target/<profile>/plugins/ruleste_*.wasm`

## 运行

```sh
[nix-shell]$ cargo run -- \
    maps/Celeste/0-Intro.bin \
    resources/Celeste/Celeste/textures/Atlases/Gameplay.meta \
    resources/Celeste/Celeste/textures/Sprites.xml \
    resources/Celeste/Celeste/textures/ForegroundTiles.xml \
    resources/Celeste/Celeste/audio
```

位置参数：`<map> <atlas-meta> <sprites-xml> <tiles-xml> [audio-dir]`，最后一项缺省为 `resources/Celeste/Celeste/audio`。插件目录由 `--plugin-path=<dir>` 覆盖，默认 `<exe_dir>/plugins/`（fallback 到 CWD 的 `plugins/`）：

```sh
cargo run -- <map> <atlas> <sprites> <tiles> <audio> --plugin-path=./my-plugins/
```

运行时开关：
- `RULESTE_DEBUG=1` — 产出插件调试日志（例如 player 状态机切换：`player {id} state -> DASH (2)`）
- `RULESTE_DUMP_FRAME=<path.ppm>` + `RULESTE_DUMP_FRAME_AT=<n>` — 第 `n` 帧把渲染缓冲 dump 成 PPM，用于像素级自检
- `RULESTE_PLAY_SFX=<stream_name>` — 启动时用 AudioBus 播一个样本自检（听声验证音频链路）

## 架构

```
src/
  data/       资源解析器（.bin 地图、.meta 图集、.data 纹理、Sprites.xml、音频 manifest/OGG、pack metadata.json、字体、存档……）
  engine/     引擎层（ECS、事件总线、输入 + jump buffer、物理 SolidGrid、自动拼接、相机、draw 命令、AudioBus 混音器）
  hotload/    Wasm 宿主 + mtime 热重载监视器
  interface/  SDL3 渲染器：320×180 逻辑分辨率、纹理上传、按 depth 排序、letterbox 缩放
plugins/      Wasm 实体插件（每个一种/一组实体，wasm32 cdylib）
crates/
  ruleste-plugin-api/  FFI 头、宏、宿主函数声明（host_input_*、host_draw_*、host_play_sound、host_emit…）
maps/         关卡包（按 pack_name 分目录）
resources/    资源包（按 pack_name/namespace 分目录；不参与分发）
```

仓库指引见 `AGENTS.md`，推进路线图见 `ROADMAP.md`。

### 微内核 + Wasm 插件

宿主用 wasmtime 加载 `.wasm`，按关卡实体类型**懒加载**（`load_plugins_for`）：0-Intro 只实例化 decorations/introcrusher/player 3 个插件，而不是全部 32 个。实体导出 `entity_init / entity_update(id, dt) / entity_draw`；组件（位置、速度、碰撞盒、精灵动画、输入）通过宿主 FFI 读写。事件总线统一 ID（`PLAYER_DASH=0`、`PLAYER_JUMP=1`、`PLAYER_DEATH=2`、`REFILL=3`、`BOOST=4`、`CRUSH=5`），插件用 `host_emit` 广播。

## 测试

```sh
[nix-shell]$ cargo test                                       # 集成测试跑 wasm 交互链
[nix-shell]$ RULESTE_PLUGIN_PATH=./target/debug/plugins cargo test  # 显式指定插件目录
```

`tests/` 里的集成测试需 wasm 先构建（`./build.sh` 会一并编译），否则测试自动 skip。

## 代码规范

- `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 必须通过
- 提交信息用 [Conventional Commits](https://www.conventionalcommits.org/)
- 实体行为严禁硬编码进核心；一律落到 `plugins/` 下的 wasm 插件

## License

MIT。资产不属于本项目；`Celeste` 是 Matt Makes Games / Maddy Thorson 的商标，本仓库与原作者无隶属关系。

## resources 资产格式

`resources/` 与 `maps/` 是两套顶层包（pack）布局。`maps/<pack>/` 放关卡；`resources/<pack>/<namespace>/` 放与之配套的非关卡资源——`<namespace>` 是内容命名空间（`Celeste`=原版，`SJ2021`/`SJ2010`=Strawberry Jam 等 mod 命名空间）。结构如下：

```
maps/
└── <pack_name>/              # 关卡包（.bin 引擎直读）

resources/
└── <pack_name>/
    └── <namespace>/          # 命名空间：Celeste / SJ2021 / ...
        ├── pack.png          # 包图标（mod 选择菜单用）
        ├── metadata.json     # 元数据（标题、作者、版本、容量……）
        ├── textures/         # 纹理：Atlases/*.meta+.data、Sprites.xml、*Tiles.xml、ColorGrading/
        ├── texts/            # 对话 .txt、Icons/
        ├── font/             # BMFont / XNA .spritefont
        ├── audio/            # 音频（由 bank→ogg 转换工具产出）
        │   ├── manifest          # 顶层索引：引用每个 bank 子目录
        │   └── <bank>/
        │       ├── manifest      # 子清单
        │       └── <stream_name>.ogg
        ├── mountain/         # 山模型
        ├── tutorials/        # 教程
        └── effects/          # 着色器 / .xnb
```

加载规则约定：引擎按 `<namespace>` 下的子目录各取所需（`textures`、`texts`、`font`、`audio`、`effects`、`mountain`、`tutorials`）；`pack.png` 与 `metadata.json` 由界面层（主菜单 / mod 选择）消费。原版 FMOD `.bank` 私有二进制 **不读**，转换工具把音频抽到同级 `audio/` 引擎可读副本。

`metadata.json` schema（`src/data/pack.rs`，`PackMeta`）：`{pack, namespace, title, version, author, homepage, description, chapters: [{id, name, map, song}]}`；`chapters` 缺省时引擎回退扫描 `maps/<pack>/*.bin`。`convert-from-celeste-contents.sh` 会顺带生成一份（`chapters` 从 maps 目录自动列出，`name`/`song` 留空待作者补全）。可用 `./target/debug/pack-info ` 快速列举所有 pack 的章节。

### 解析状态总览

| 子目录 | 格式 | 引擎是否查 | 状态 |
|---|---|---|---|
| `maps/<pack>/*.bin` | `.NET` 7-bit varint 地图 | 是 | ✅ |
| `textures/Atlases/*.meta` + 同名 `.data` | `Monocle.Atlas` 清单 + RLE RGBA 纹理 | 是 | ✅ |
| `textures/Sprites.xml`, `Portraits.xml` … | `Monocle.SpriteBank` XML | 是 | ✅ 已解析 player 动画 |
| `textures/ForegroundTiles.xml`, `BackgroundTiles.xml` | 自动拼接规则 | 是 | ✅ |
| `textures/ColorGrading/*.png` | 后期调色表 | 是 | ⏳ |
| `audio/<bank>/*.ogg` + manifest | Ogg Vorbis + 文本清单 | 是 | ✅ AudioBus 解码 + 混音 |
| `texts/*.txt` + `texts/Icons/` | 对话文本 / 图标 | 是 | ⏳ |
| `font/*` (.bmfc / .spritefont) | BMFont / XNA 字体 | 是 | ⏳ |
| `effects/*.xnb` | 着色器 | 是 | ⏳ |
| `mountain/`, `tutorials/` | 山模型 / 教程 | 是 | ⏳ |
| `pack.png` + `metadata.json` | 包元数据 | 否（界面层用） | ⚠️ PackMeta 解析已交付（`pack-info` 工具） |
| 原 `FMOD/Desktop/*.bank`（转换工具的输入，**不放进 resources**） | FMOD 私有二进制 | 否 | ❌ 见下文 |

### 关于 FMOD `.bank`——不直接读

`*.bank` 是 FMOD Studio 的 **私有二进制容器**（RIFF 包装的 FEV/FMT / FSB5 Custom-Vorbis）。它需要 FMOD 运行时解码，而本项目“不依赖游戏框架”的约束明确排除——`ROADMAP.md` 也写着“音频：FMOD 事件替换为 SDL 音频，`AudioBus` 请求接入”。所以 **Ruleste 不读 `.bank`**；转换工具（见下）把音频抽进 `resources/<pack>/<namespace>/audio/`，引擎只读 OGG。`.bank` 本体留在 distro 外，不进 `resources/`。

### 音频包（`audio/`）

由 `tools/bank-to-ogg.sh` 在本地一次性生成：

```
resources/<pack>/<namespace>/audio/
  manifest                       # 顶层索引，引用每个 bank 子目录
  <bank>/
    manifest                     # 子清单（每个 bank 一份）
    <stream_name>.ogg            # 每个 subsong 一个 OGG
```

`<bank>` 是 bank 文件 stem 经路径安全化（空格→下划线）：`sfx`、`music`、`Master_Bank`、`dlc_sfx`、`dlc_music`、`ui`。

`manifest` 行格式（制表符分隔）：

```
# bank=<display name>
<sample_name>\t<bank display>\t<file path>
# 例：
game_gen_diamond_touch_01	sfx	sfx/game_gen_diamond_touch_01.ogg
ui_world_whoosh_0400ms_forward	Master Bank	Master_Bank/ui_world_whoosh_0400ms_forward.ogg
```

顶层 `manifest` 懒引用各子清单：

```
# Ruleste audio manifest — generated by tools/bank-to-ogg.sh
# Format: 'sample_name'  'bank'  'file'
Master Bank	(bank)	Master_Bank/
sfx	(bank)	sfx/
music	(bank)	music/
...
```

加载约定：引擎循环打开每份子清单建表（`AudioManifest`，`src/data/audio.rs`），把 `sample_name` 解析到 `<file path>`（相对 `audio/` 根），供 `AudioBus`（`src/engine/audio.rs`）按名播放。`.ogg` 懒解码（首次播放读盘、解码成 48kHz 立体声 f32、Arc 缓存），支持投音 pan、循环、常功率混音。**注意**：子清单文件名从旧设计的 `audio.manifest` 改成 `manifest`（与上方结构图、文档一致）。

### 转换工具：`tools/bank-to-ogg.sh`

把一批 FMOD `.bank`（不含 `*.strings.bank`）整批转成 OGG + `manifest`。底层用 **vgmstream** 解 FSB5/Custom-Vorbis，再用 **ffmpeg** 编码成 Ogg Vorbis（VBR q4）；二者都从 nixpkgs 拉取，不污染默认开发 shell：

```sh
nix shell nixpkgs#vgmstream nixpkgs#ffmpeg -c \
    tools/bank-to-ogg.sh <bank-dir> resources/Celeste/Celeste/audio
# 例：
nix shell nixpkgs#vgmstream nixpkgs#ffmpeg -c \
    tools/bank-to-ogg.sh references/Celeste/Content/FMOD/Desktop \
                              resources/Celeste/Celeste/audio
```

验证过 6 个 bank → **4968 个样本**（sfx=3463、music=647、dlc_sfx=659、ui=150、dlc_music=34、Master_Bank=15）。

- FSB5 解码是「私有格式」这活，vgmstream 长 30k LoC、米级别嵌入 setup-table；本项目把它当作 **dev 工具**（nix shell 外部拉），不进引擎依赖；`build.sh` / 主 crate 都不会引用 vgmstream。
- 音频寻址以 **stream name 为主键**（`sample_name`，如 `game_gen_diamond_touch_01`），即 manifest 第一列、OGG 文件名。原版 `event:/...` 路径与 stream-name 的映射藏在 `*.strings.bank`（FMOD 私有反序列化格式）里，解析那条连接是 ROADMAP「音频」项剩下的工作。
- 宿主的 `AudioBus`（`src/hotload/wasm_host.rs`）接收插件 `host_play_sound("event:/...")` 请求；后续在加载阶段把 `event:/...` 路径映射到 manifest 的 `sample_name` 即可（POI：玩家插件声道已就位）。
- `RULESTE_PLAY_SFX=<stream_name>` 可在启动时播一个样本，快速验证音频链路（需要音频设备；无声环境 AudioBus 自动降级为 inert，不影响运行）。

## 贡献

- 改动落到功能分支或补丁，PR 附设计动机说明。
- 改 Wasm 宿主 API 时同步更新插件示例，保证 ABI 兼容。
- 永不引用 `references/` 里的文件。
