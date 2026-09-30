# Audio.cs / AudioState.cs / Buses / Snapshots / VCAs vs Ruleste AudioBus 对比分析

> 源码对照
> - C# 原版：`references/source/Celeste/Celeste/Audio.cs`（680 行）、`AudioState.cs`（82 行）、`AudioTrackState.cs`（111 行）、`Buses.cs`、`Snapshots.cs`、`VCAs.cs`
> - Rust 实现：`crates/ruleste-core/src/data/audio.rs`（manifest，174 行）、`crates/ruleste-core/src/engine/audio.rs`（SDL3 混音器，304 行）、`crates/ruleste-core/src/hotload/wasm_host.rs`（`AudioBus{requests}` 中间层）、`crates/ruleste-plugins-api`（`play_sound`）
>
> 顶层结论：Ruleste 用**纯软件 SDL3 混音器**替换 FMOD Studio，没有事件/总线/快照/VCA，用 **stream name** 直接寻址（`event:/...` 映射是 ROADMAP 遗留研究项）。因此大部分 API 是 🔴/🟠，但播放链路有明确等价物。

## 一、全局状态

| C# `Audio` 静态字段 | Rust 对应 | 状态 |
|--------------------|-----------|------|
| `Banks`（Master/Music/Sfxs/UI/Dlc*） | `AudioManifest.banks`（名字列表） | 🟠 |
| FMOD Studio `system` | SDL3 `AudioDevice` + `AudioStreamOwner` | 🟠 架构替换 |
| `currentCamera` + `SetCamera` | 宿主 `GameState.camera` | 🟡 |
| `CurrentMusic` | `AudioBus` 无“当前音乐”概念 | 🔴 |
| `currentMusicEvent / currentAmbientEvent / currentAltMusicEvent` | 缺失 | 🔴 |
| `musicUnderwater` + snapshot | 缺失 | 🔴 |
| `ready`（FMOD 初始化） | `AudioBus::is_available()` | ✅ |

## 二、音量/总线/VCA

| C# | Rust | 状态 |
|----|------|------|
| `MusicVolume`（vca:/music） | 无（只有全局 master_gain） | 🔴 |
| `SfxVolume`（gameplay+ui） | 无分总线 | 🔴 |
| `PauseMusic`（bus:/music） | 缺失（无按 bus 暂停） | 🔴 |
| `PauseGameplaySfx` | 缺失 | 🔴 |
| `PauseUISfx` | 缺失 | 🔴 |
| `BusPaused/BusMuted/BusStopAll` | 缺失（只有 `stop_all` 全停） | 🔴 |
| `VCAVolume` | 无独立 VCA | 🔴 |
| `SetMasterGain` | ✅ 全局 gain | ✅ （Rust 特有近似） |

## 三、事件实例 API

| C# | Rust | 状态 |
|----|------|------|
| `Play(path)` | 插件 `host_play_sound -> engine.audio.play(name)` | ✅ 概念等价（含 volume/pan/pitch/loop） |
| `Play(path, param, value)` | 缺失（无事件参数） | 🔴 |
| `Play(path, position)`（3D） | 缺失（无 3D/pan 定位） | 🔴 |
| `Loop(path)` | `host_play_sound` 的 `looping` 参数 | ✅ 已暴露 |
| `Pause(instance)` / `Resume(instance)` | 缺失（无句柄） | 🔴 |
| `Position(instance, pos)` | 缺失（无 3D 属性） | 🔴 |
| `SetParameter(instance, param, value)` | 缺失 | 🔴 |
| `Stop(instance, allowFadeOut)` | `engine.audio.stop(name)`（无淡出） | 🟡 |
| `CreateInstance/GetEventDescription` | `AudioManifest::get`（按 stream 名） | 🟠 |
| `IsPlaying(instance)` | 缺失 | 🔴 |
| `GetEventName(instance)` | 缺失（名字即 key） | 🔴 |

## 四、音乐/氛围

| C# | Rust | 状态 |
|----|------|------|
| `SetMusic(path, startPlaying)` | `play(name, looping=true)` 手动管理 | 🟡 |
| `SetAmbience(path)` | 同上 | 🟡 |
| `SetMusicParam(path, value)` | 缺失（无事件参数/层） | 🔴 |
| `SetAltMusic(path)` + `mainDown` snapshot | 缺失 | 🔴 |
| `AudioState` / `AudioTrackState`（事件 + `MEP` 参数 + `Layer`/`Progress`） | 缺失 | 🔴 |

## 五、Snapshot

`CreateSnapshot / ResumeSnapshot / IsSnapshotRunning / EndSnapshot / ReleaseSnapshot` → 全 🔴。常量（pause_menu、underwater、dialogue、dash_assist 等）无对应。

## 六、Rust 实现要点

`crates/ruleste-core/src/engine/audio.rs`：以 48kHz/f32/stereo 打通 SDL3 `AudioStream`，常功率 pan，硬裁剪，懒解码 OGG（Arc 缓存），支持 repeating，headless 时 inert。数据侧 `data/audio.rs` 解析转换工具的 `manifest`（顶层 bank 清单 + 每 bank 子清单），流名全局唯一。

**插件音效链路（已打通）**：`wasm_host.rs` 的 `AudioBus{ requests }` 由主循环消费——`main.rs` 每帧 `wasm_host.game_state().audio.drain()` 取出 `AudioRequest`，交给 `engine::audio::AudioBus::play_plugin_requests` 进 SDL 混音器，不再静默丢失。

**event → stream 映射（已打通）**：`engine::audio::AudioBus::resolve` 内联了一份 `event:/...` → `game_<NN>_<sound>` 的映射表（取自原版 `SFX.cs`，运行时不依赖 `references/`），并按 `_NN` / `_N` 两种后缀探测同名编号变体，覆盖转换工具命名差异；未命中的名字计入 `AudioStats::unknown`。表覆盖当前插件实际触发的音效，不是全量 `SFX.cs`。

## 七、差异根因

1. **无 FMOD**：本项目约束不引入 FMOD 运行时；SDL3 纯软件替代使其天然没有 Studio 事件/总线/快照/VCA。
2. **寻址模型**：FMOD `event:/...`（带 param/层）→ license 无关的 `stream name`；映射文件（`*.strings.bank`）无法机械解析，当前只能接受 stream-name 主键。
3. **宿主边界**：插件 FFI 目前只有 `play_sound(name)`，没有 volume/pan/pitch/loop/position/param。

## 八、建议优先级

| 优先级 | 项目 | 原因 |
|--------|------|------|
| ~~P0~~ | ~~把 `wasm_host.game_state().audio.drain()` 接进主循环的 SDL `audio.play`~~ | ✅ 已完成（`main.rs` 每帧 drain → `play_plugin_requests`） |
| ~~P1~~ | ~~`host_play_sound` 增加 volume/pan/pitch/loop 参数~~ | ✅ 已完成（`AudioRequest` 携带 volume/pan/pitch/loop 直通混音器） |
| ~~P1~~ | ~~event→stream 映射~~ | ✅ 已完成（`AudioBus::resolve` 内联映射表 + 编号变体探测）；后续可扩成外部映射文件 |
| P2 | `AudioState`/`AudioTrackState` 音乐/氛围状态机 | 章节转换、层参数 |
| P2 | 总线/VCA/snapshot 简化近似（bus 暂停/音量组） | 菜单/暂停静音 |
| P3 | 3D/positional 及参数 | 当前玩法不依赖 |
