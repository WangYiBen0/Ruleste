# Ruleste 与原版 Celeste 完整审计与差异对照总表

> 审计基准：原版源码 `references/source/Celeste/`（Monocle 引擎 + Celeste 逻辑）；我方 `src/`、`crates/ruleste-plugin-api/`、`plugins/*`。
> 说明：随着全部 47 个实体插件的代码框架已于近期全部补齐（见 commit `f4e953d`），本总表合并了 `ORIGINAL-DIFFS.md` 与 `UNEXPECTED-DIFF.md`，用于系统性追踪我方实现与原版 Celeste 的数值、物理、机制与表现差异。

---

## 一、47 插件与原版类映射总表

| 我方插件 | 地图类型 | 原版类 | 原版文件 | 状态 |
|---|---|---|---|---|
| player | — | Player | Player.cs | 已实现（基础状态） |
| booster | booster | Booster | Booster.cs | 已实现 |
| checkpoint | checkpoint | Checkpoint | Checkpoint.cs | 已实现 |
| cliffflag | cliffflag | CliffFlags | CliffFlags.cs | 已实现 |
| cloud | cloud | Cloud | Cloud.cs | 已实现 |
| clutter | red/yellow/greenBlocks | ClutterBlockBase + ClutterBlock + Generator | ClutterBlock*.cs | 已实现 |
| cobweb | cobweb | Cobweb | Cobweb.cs | 已实现 |
| crushblock | crushBlock | CrushBlock | CrushBlock.cs | 已实现 |
| debris | floatingDebris/foregroundDebris | FloatingDebris/ForegroundDebris | FloatingDebris.cs 等 | 已实现 |
| door | door | Door | Door.cs | 已实现 |
| dreamblock | dreamBlock | DreamBlock | DreamBlock.cs | 已实现 |
| fallingblock | fallingBlock | FallingBlock | FallingBlock.cs | 已实现 |
| hanginglamp | hanginglamp | HangingLamp | HangingLamp.cs | 已实现 |
| introcrusher | introCrusher | IntroCrusher | IntroCrusher.cs | 已实现 |
| lamp | lamp | Lamp(position, broken) | Lamp.cs | 已实现 |
| lightbeam | lightbeam | LightBeam | LightBeam.cs | 已实现 |
| refill | refill | Refill | Refill.cs | 已实现 |
| resortLantern | resortLantern | ResortLantern | ResortLantern.cs | 已实现 |
| rotatespinner | rotateSpinner | Star/Dust/BladeRotateSpinner | RotateSpinner.cs | 已实现 |
| soundsource | soundSource | SoundSourceEntity | SoundSourceEntity.cs | 已实现（占位） |
| spikes | spikesUp/Down/Left/Right | Spikes | Spikes.cs | 已实现 |
| spinner | spinner | DustStaticSpinner 或 CrystalStaticSpinner | Level.cs:748-770 | 已实现 |
| spring | spring | Spring | Spring.cs | 已实现 |
| strawberry | strawberry | Strawberry | Strawberry.cs | 已实现 |
| torch | torch | Torch | Torch.cs | 已实现 |
| wire | wire | Wire | Wire.cs | 已实现 |
| zipmover | zipMover | ZipMover | ZipMover.cs | 已实现 |
| swapblock | swapBlock | SwapBlock | SwapBlock.cs | 已实现 |
| dashblock | dashBlock | DashBlock | DashBlock.cs | 已实现 |
| starjumpblock | starJumpBlock | StarJumpBlock | StarJumpBlock.cs | 已实现 |
| triggerspikes | triggerSpikes* | TriggerSpikes | TriggerSpikes.cs | 已实现 |
| memorial | memorial | Memorial | Memorial.cs | 已实现 |
| bonfire | bonfire | Bonfire | Bonfire.cs | 🌱 初版 |
| flutterbird | flutterbird | FlutterBird | FlutterBird.cs | 🌱 初版 |
| bird | bird | BirdNPC | BirdNPC.cs | 🌱 初版 |
| npc | npc | NPC 基类 + 分派 | NPC.cs | 🌱 初版 |
| heartgemdoor | heartGemDoor | HeartGemDoor | HeartGemDoor.cs | 🌱 初版 |
| introcar | introCar | IntroCar + IntroCarBarrier | IntroCar.cs | 🌱 初版 |
| killbox | killbox | Killbox | Killbox.cs | 已实现 |
| plateau | plateau | Plateau(Solid 104×4) | Plateau.cs | 已实现 |
| invisiblebarrier | invisibleBarrier | InvisibleBarrier(Solid+ClimbBlocker) | InvisibleBarrier.cs | 已实现 |
| summitbackground | SummitBackgroundManager | AscendManager | AscendManager.cs | 🌱 初版 |
| towerviewer | towerviewer | Lookout | Lookout.cs | 🌱 初版 |
| trackspinner | trackSpinner | Star/Dust/BladeTrackSpinner | TrackSpinner.cs | 已实现 |
| infinitestar | infiniteStar | FlyFeather | FlyFeather.cs | 已实现 |
| badelineboost | badelineBoost | BadelineBoost | BadelineBoost.cs | 已实现 |
| bigspinner | bigSpinner | Bumper | Bumper.cs | 已实现 |

---

## 二、核心差异详述（按影响分级）

### 🔴 P0（严重破坏游戏流程与核心机制）

| 实体/系统 | 核心差异 | 影响 |
|---|---|---|
| ✅ **crushBlock (Kevin)** | 1. 原版静止时只有 6px 顶部带 `playerCollider`；挤压时才切 `staticCollider` 全实心。Ruleste 一直是 `solid(true)` 全实心。2. ~~只响应**横向** dash（`EV_CRUSH` 只在 `hit_wall_left/right` 触发），竖向 Kevin 无法激活~~ (已修复：player 插件现已在天花板碰撞时也发送 `EV_CRUSH`，crushBlock 按 `axes` 决定挤压方向)。3. 无 `returnStack` 多段返回，直线回原点。4. 巨型 chillout Kevin：原版撞击后**永久卡死**，Ruleste 会 `canActivate=true` 重置。5. 缺 `Dangerous` 挤压瞬杀、粒子、音效、shake、ruler over ledge 跨越。 | 玩家可穿过侧面、竖向 Kevin 失效、回路飞行、巨型 Kevin 可反复利用、不挤死玩家 |
| ✅ **zipMover** | 1. ~~原版**等待骑乘者** (`HasPlayerRider`) 才启动；Ruleste 持续往返~~ (已修复：新增 `player_on_top` 等待玩家踩上才启动)。2. ~~原版缓动 `SineIn`（快出快回）；Ruleste 用 `SineInOut` 两头慢~~ (已修复：采用 `0.5 - 0.5*cos` 的 SineIn 缓动)。3. ~~原版 0.5s/2s+0.5s 暂停；Ruleste 用单一 `CYCLE_TIME=3.4s` 无暂停~~ (已修复：在终点暂停 2s 后原路返回)。4. `actor_move` 会被中间的实心瓦片卡住。 | ZipMover 行为完全不同，可能卡死、无视骑乘 |
| ✅ **starJumpBlock** | 1. ~~原版是 `Solid` 四面实心；Ruleste 用 `platform(true)` 单向可穿~~ (已修复：改为 `solid(true)` 四面实心)。2. Ruleste **自造**底部触发 `-320` 弹射，绕过 `StarJumpController`。3. 弹射速度 `-320` 是魔法数，原版由控制器逐渐加速。 | 每个星跳块都成了自动弹簧，跳过整个星飞流程 |
| ✅ **fallBlock** | 1. ~~无 `climbFall`：原版侧面攀爬也能触发；Ruleste 只检测顶部~~ (已修复：碰撞检测扩展为包含侧面接触)。2. 幻影 0.4s `delay`（原版立即 shake）。3. 无 shake/SFX/粒子，无 `TileInterceptor`（草莓随块落）。4. **最关键**：落在跳跳平台/移动平台上会**永久卡死**（原版 `while CollideCheck<Platform> yield 0.1` 继续落）。 | 落在平台上永久卡死、侧面触发失效、无视觉反馈 |
| **trackSpinner** | 1. Collider 原版 `ColliderList(Circle(6), Hitbox(16,4,-8,-3))`（中心圆+横向带）；Ruleste 12×12 AABB 角落多判 70%。2. 原版 `dt/MoveTimes[speed]` 三档速度 + `PauseTimer` 停顿；Ruleste 线性 60px/s 无停顿。3. 缺 `startCenter`、`Ease.SineInOut`、速度枚举。 | 击杀区更大、速度/停顿全错、庙宇刀片不可见 |
| **introCrusher** | 1. 原版 `safe: true`（挤压时允许玩家重叠不压扁）；Ruleste `solid(true)` 会把玩家压入地板致死。2. 无 Session-flag 短路，每次重生都重新落。3. 无 dust/SFX/shake。 | 石板压死玩家、重复触发、无视觉反馈 |
| ✅ **plateau** | ~~原版 `Solid`（四面实心）+ `Safe=true`；Ruleste `platform(true)` 可从下方穿越~~ (已修复：改为 `solid(true)` 四面实心)。 | 岭桥可从下方穿越，破坏 6/7/8 关卡设计 |
| **swapBlock** | 1. 原版有 `PathRenderer`（虚线轨迹）、`redAlpha` 红绿渐变、ghost 拖尾、`moveSfx`/`returnSfx`、主题(Moon/红)。2. Ruleste 只画黄矩形，无音效/路径/主题/位移/粒子。 | 视觉/听觉完全缺失，交换块无指示 |
| **booster** | 1. 红加速器 `RedBoost`（冻结-再冲刺）缺失。2. `BubbleReturn` 时序与原版不符。3. 无 `Outline`、光晕、镜像、wiggler。 | 红加速器失效、视觉/音频缺失 |
| **spring** | 1. 无壁弹 `SideBounce`。2. 无 `staticMover` 跟随平台、无 `TriggerPlatform` 连锁弹簧。3. 无 bounce sound、无 cooldown 机制。 | 壁弹手感错误、连锁弹簧失效 |

---

### 🟠 P1（明显体验差异）

| 实体/系统 | 差异摘要 |
|---|---|
| **cloud** | 1. 用 `platform(true)` 而非 `JumpThru(safe:false)`。2. 无 `cloudRemix`、消失粒子、无 `RespawnParticles`、无 rumble。3. 顶面高度有偏差。 |
| **infiniteStar** | 虽有插件初版，但缺乏 `St.InfiniteStamina` 状态联动、pickup SFX、rumble 与 bubble 粒子。 |
| **invisibleBarrier** | 原版只在 `Player.OnCollideH/V` 中阻挡 dash；当前实现可能存在阻挡范围差异。 |
| **heartGemDoor** | 缺乏完整交互（80px 内计数器吸合、开门动画、双 Solid、50 粒子与雾）。 |
| **memorial** | 无多语言文本控制器、无 SpriteBank 动画，仅读取基础属性。 |
| **npc** | 目前多为占位，缺失 29 路 NPC 分派、对话、心形图标、过场协程。 |
| **clutter** | 无 `ClutterBlockManager` 生命周期、无开门联动、碎裂无 touch dust。 |
| **spikes / triggerSpikes** | 上向缺 headroom 守卫，triggerspikes 距离感应与自动收回机制与原版不符。 |
| **decorations / 粒子与音效** | 所有游戏内粒子系统（`P_*`）、FMOD 音频事件（`event:/game/...`）、光照遮挡（`LightOcclude`）与镜面反射全系缺失。 |

---

### 🟡 P2（细节、视觉与架构差异）

1. **事件总线 vs 直接调用**：原版由玩家侧守卫（如满状态不消耗 Refill、状态机约束），我方多为“插件单方面判定 + 玩家无条件执行”。
2. **时序与物理原语**：缺少原版 Coroutine（协程精准等待）、Tween、Wiggler、Shaker、StaticMover、Tracker 系统，目前主要靠手写 `f32` 计时器。
3. **深度语义**：渲染层级与原版正负号约定或有出入，部分实体的 `depth` 值需要逐一核对校正。
4. **资源解析细节**：Spritebank 的 `copy=`、`Justify`、`frames` 的 `*` 重复语法在解析器中尚未完全覆盖。

---

## 三、统计概览

| 类别 | 实体数 | 状态概述 |
|---|---|---|
| 移动实心体 | 7 | 插件均已创建，但 Collider 语义、触发/返回、缓动与震动需精修 |
| 交互/道具 | 8 | 基础逻辑具备，触发、音效、粒子及视觉状态机待对齐 |
| 收集/门/危险 | 9 | 核心拾取与尖刺具备，部分高级形态（如金色草莓、无限羽毛状态）待完善 |
| 杂项/背景 | 6 | 生命周期、联动、对话系统为后续重点 |
| 装饰实体 | 16 | 几何与纹理渲染正常，全系粒子、光照、音效、遮挡待补齐 |

**总结**：全部 47 个地图实体均已落实插件代码框架（`plugins/*`），核心通关流程已能加载运行。当前的重点工作在于**物理细节对齐、玩家状态机扩充、以及粒子与音效系统的引入**。

---

## 四、后续优化优先级

1. **P0 块实体碰撞与行为对齐**：
   - ✅ `plateau` 已改为四面实心 `solid`。
   - ✅ `starJumpBlock` 已改为四面实心 `solid`。
   - ✅ `zipMover` 已实现骑乘触发、目标点暂停与原路返回。
   - ✅ `fallingBlock` 已实现侧面攀爬触发 `climbFall`。
   - ✅ `crushBlock` 已支持竖向（天花板）碰撞触发。
   - 🔲 `introCrusher` 待实现 `safe: true` 挤压不死与会话旗标短路。
   - 🔲 `swapBlock` 待实现路径轨迹、红绿渐变、主题与音效。
2. **Player 状态机扩展**：逐步实现原版 26 个状态中的关键缺失状态（如羽毛飞行 `StarFly`、游泳、红冲等）。
3. **粒子与音效系统**：建立统一的宿主 FFI 接口（`host_play_sound`、`host_emit_particle`），逐插件补全视听反馈。
4. **NPC 与对话系统**：引入 `Dialog` 解析与对话框渲染，恢复关卡中的剧情与NPC互动。
5. **存档与会话旗标 (Session / Checkpoint)**：完善存档、DoNotLoad、Session 旗标持久化语义。
