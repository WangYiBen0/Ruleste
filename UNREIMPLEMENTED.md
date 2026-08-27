# UNREIMPLEMENTED.md — 未重实现部分清单

> 基准：原版 `references/source/Celeste/`，共 595 个 `Celeste/*.cs` 文件。
> 对照：本仓库 `src/`（引擎/数据/界面）+ `plugins/*`（74 个 Wasm 实体插件）。
> 规模：按命名可直接映射的仅约 76 个，重实现游戏逻辑约占原版 13%（不含编辑器 / Pico8 / 3D 山顶等目标外内容）。
> 标注：🔴 缺失 / 🟡 近似 / ⬜ 空壳（无绘制或无逻辑）。

---

## 一、完全未启动的子系统（无 Rust 对应）

| 子系统 | 原始文件（代表） | 说明 |
|---|---|---|
| **菜单 / UI** | `Oui*.cs`（MainMenu / FileSelect / ChapterSelect / Journal / Options / Assist / Credits / Title）、`TextMenu`、`ButtonUI`、`MenuButton`、`LanguageSelectUI`、`KeyboardConfigUI` | 仅 `screens.rs` 极简占位；章选 / 介绍 / 结局 / 日记 / 设置均缺 |
| **存档 / 会话 / 旗标** | `SaveData.cs`、`Session.cs`、`Settings.cs`、`Assists.cs`、`PlayerInventory.cs`、`DeathData.cs`、`EntityID.cs`、`Stats.cs` | 无运行时存档 / 旗标 / 辅助 / 背包 API（阻断 checkpoint 存档、heartgemdoor 计数等） |
| **对话 / 过场** | `Dialog.cs`、`Textbox.cs`、`MiniTextbox.cs`、`MemorialText.cs`、`TalkComponent.cs`、全部 `CS*_*.cs`（数百过场脚本）、`Poem.cs`、`Postcard.cs` | 无对话框 / 打字机 / 过场系统 |
| **粒子 / 残影 / 位移** | `ParticleTypes.cs`、`ParticleRenderer.cs`、`TrailManager.cs`、`SlashFx.cs`、`DeathEffect.cs`、`SpeedRing.cs`、`Dust*.cs`、`FallEffects.cs` | 全缺（阻断 collect 演出、trail、命中特效） |
| **后处理 / 渲染管线** | `BloomRenderer.cs`、`CustomBloom.cs`、`DisplacementRenderer.cs`、`LightingRenderer.cs`、`Godrays.cs`、`GaussianBlur.cs`、`Glitch.cs`、`Distort.cs`、`HiresRenderer.cs`、`VertexLight.cs`、`ReflectionFG.cs` | 无泛光 / 位移 / 光照 / 镜像 |
| **山顶 3D（Overworld）** | `Overworld*.cs`、`Mountain*.cs`、`Maddy3D.cs`、`ObjModel.cs`、`Snow3D.cs`、`Planets.cs`、`Skybox.cs`、`StarsBG.cs` | 完全未做 |
| **音频运行时** | `Audio.cs`、`AudioState.cs`、`Snapshots.cs`、`VCAs.cs`、`Buses.cs`、`SFX.cs`、`MusicTrigger.cs`、`AmbienceParamTrigger.cs`、`SoundSourceEntity.cs` | 仅 `audio.rs` 有 SDL 混音器；无 3D / 参数 / 快照 / 音乐状态 |
| **编辑器 / Pico8** | `Celeste.Editor/*`、`Celeste.Pico8/*` | 超出重实现目标，预期不移植 |

---

## 二、缺失的游戏实体（原版有类，无对应插件）

### 危险 / 障碍
- `Puffer` / `PufferCollider`（河豚）
- `Lightning` / `LightningBreakerBox` / `LightningStrike` / `LightningRenderer`
- `LavaRect` / `RisingLava`（core 插件仅有简化 risingLava）/ `SandwichLava`
- `WindController` / `WindMover` / `WindSnowFG`
- `TempleEye` / `TempleBigEyebrow` / `TempleGate` / `TempleMirror` / `TempleCrackedBlock` / `TemplePortalTorch`
- `SeekerStatue` / `SeekerBarrier` / `SeekerCollider` / `SeekerEffectsController`
- `NegaBlock`、`GlassBlock` / `GlassBlockBg`、`GoldenBlock`
- `FakeHeart` / `DreamHeartGem` / `HeartGem`、`CrystalDebris` / `CrystalColor`
- `Trapdoor`、`FloatySpaceBlock` / `RotatingPlatform` / `MovingPlatformLine` / `SinkingPlatformLine`

### 玩家附属 / 持物系统
- `PlayerHair`、`PlayerDeadBody`
- `PlayerSprite` / `PlayerSpriteMode`、`PlayerCollider`、`PlayerDashAssist`
- `Holdable` / `HoldableCollider` / `Leader` / `Follower`（持物 / 拖尾，阻断 strawberry / theo / reflect 等）
- `TrailManager`、`SlashFx`

### NPC / 剧情实体
- `FlutterBird`（原 ORIGINAL-DIFFS 点名但无插件）
- `Cobweb`、`Lamp` / `HangingLamp` / `ResortLantern`（均无插件）
- `BirdPath` / `BirdPathTrigger`
- `AngryOshiro`、`BadelineDummy` / `BadelineOldsite` / `BadelineAutoAnimator`
- `Oshiro*` 系列、`Granny` / `Theo` NPC 脚本（`NPC0*`）
- `MoonCreature`、`FlingBird` / `FlingBirdIntro`、`Glider`（鹰）

### 平台 / 机关 / 触发
- `DashSwitch`、`DashCollision` / `DashCollisionResults`（swap-block 用 `PLAYER_DASH` 近似）
- `ClimbBlocker` / `LedgeBlocker` / `SafeGroundBlocker`、`StaticMover`（下沉平台依赖）
- `Lookout`（towerviewer 插件空占位）、`AscendManager`（summitbackground 插件空占位）
- `SummitGem` / `SummitGemManager` / `SummitVignette`、`CoreStarsFG` / `CoreVignette`
- `StarJumpController`、`BlackholeBG` / `BlackholeStrengthTrigger`
- `IntroCarBarrier` / `IntroPavement` / `IntroVignette`
- `Spawn` / `SpawnManager`、`Decal` / `DecalData`
- `Flagline`（cliffside-flag 仅覆盖 CliffFlags）、`ResortMirror` / `MirrorSurface` / `MirrorFG`
- `WaterSurface` / `WaterInteraction`（water 插件部分）
- **FinalBoss 整章**：`FinalBoss` / `FinalBossBeam` / `FinalBossMovingBlock` / `FinalBossShot` / `FinalBossStarfield` 全部缺失

### 大量 Trigger（约 70+）
`CameraLocker` / `CameraOffsetTrigger` / `CameraTargetTrigger`、`AltMusicTrigger`、`BloomFadeTrigger`、`ChangeRespawnTrigger`、`CheckpointBlockerTrigger`、`CreditsTrigger`、`CrumbleWallOnRumble`、`EventTrigger`、`GoldBerryCollectTrigger`、`LightFadeTrigger`、`MusicFadeTrigger`、`NoRefillTrigger`、`OshiroTrigger`、`RumbleTrigger`、`SpawnFacingTrigger`、`StopBoostTrigger`、`WindTrigger` / `WindAttackTrigger` 等——触发系统本身未实现。

---

## 三、已移植实体内、系统性缺失行为（跨所有插件）

- **视觉 / 音频 / 粒子**：精灵动画帧、Wiggler / 光晕 / Bloom、SlashFx、音效全部缺。
- **相机 / 反馈**：Shake / rumble / Freeze / 位移爆发缺（宿主无 API）。
- **会话旗标与存档**：所有依赖 `Session` / `SaveData` / `Flag` 的行为不可表达。
- **死亡体系**：`Player.Die(dir)` 无方向、`PlayerDeadBody`、assist 豁免缺。
- **持物 / 拖尾 / 毛发**：`Holdable` / `Leader` / `Follower` / `PlayerHair` 缺，阻断羽毛持物、theo、reflect 等联动。

---

## 四、占位手动绘图实体（仅矩形 / 线条 / 无绘制，无真实精灵）

> 扫描方法：对每个 `plugins/*/src/*.rs` 检测是否调用 `draw_image` / `draw_sprite` / `draw_atlas` / `draw_texture`（真实精灵帧）。以下为**未调用真实精灵绘制**的插件——其 `draw` 仅用 `draw_rect` / `fill_rect` / `draw_line` 程序化绘制，或完全空壳，缺失原版图集美术。

### 4.1 仅矩形 / 填充绘制（rect / fill）—— 程序化色块占位
badeline-boost、big-spinner*、black-gem、bounce-block、bridge、bridge-fixed、cassette、cassette-block、celestial-resort、cliffside-flag、clutter、color-switch、core、coverup-wall、crumble-block、dark-chaser、dash-block、door⬜、dream-mirror、exit-block、fake-wall、falling-block、fire-ball、fire-barrier、forsaken、gondola、hahaha、heart-gem-door、ice-block、intro-car、intro-crusher⬜、invisible-barrier⬜、key、killbox⬜、lock-block、lostlevels、mirror-temple、move-block、moving-platform、npc、old-site、plateau、reflection、ridge-gate、seeker、sinking-platform、spring⬜、star-jump-block、strawberry⬜、summit、swap-block、switch-gate、theo-crystal、touch-switch、wall-booster、water、waterfall、whiteblock、zip-mover

> `*` big-spinner 同时用 line（刀片）+ rect。
> `⬜` 表示 `draw` 完全为空（无任何绘制）：door、intro-crusher、invisible-barrier、killbox、player、spring、strawberry。

### 4.2 仅线条绘制（line / draw_lines）—— 程序化几何占位
- `big-spinner`（刀片线框）、`rotate-spinner`（四根刀片）、`infinite-star`（兼 rect）

### 4.3 完全无绘制（EMPTY draw）—— 隐形 / 空壳
- `door`、`intro-crusher`、`invisible-barrier`、`killbox`、`player`、`spring`、`strawberry`
- 另：`summit`（SummitBackgroundManager）、`towerviewer`（Lookout）的 update/draw 均为空占位（见第二节）。

### 4.4 已使用真实精灵（参考对照，非占位）
以下插件已接 `draw_image` / 图集帧，可视作正确渲染：
booster、checkpoint、cloud、crush-block、decorations、dream-block、memorial、refill、spikes、spinner、track-spinner、trigger-spikes

---

## 五、规模参考
- 原始 `Celeste/Celeste/*.cs`：**595** 文件。
- 直接映射到插件 / `src`：约 **76** 个。
- 重实现游戏逻辑占比：约 **13%**（不含编辑器 / Pico8 / 3D 山顶等目标外内容）。
- 仅程序化绘制（无真实精灵）的插件：**64 / 74**。
- 完全空 `draw` 的插件：**7**（door、intro-crusher、invisible-barrier、killbox、player、spring、strawberry）。
