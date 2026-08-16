# Ruleste 与原版 Celeste 逐文件对照差异审计（二次复查）

> 审计基准：原版源码 `references/source/Celeste/`（Monocle 引擎 + Celeste 逻辑）；我方 `src/`、`crates/ruleste-plugin-api/`、`plugins/*`。
> 二次复查于 2026-08-16 完成，覆盖全部插件 + player + 引擎/数据层，四路并行逐文件与对应 C# 类核对，关键结论均亲自复核。
> 行号引用格式：`原版文件:行`、`我方文件:行`。
> 标注：✅ 相对上次审计已修复 / 🔴 仍错或仍缺 / 🟠 部分 / 🟡 近似。

---

## 〇、全局结论（相对上次审计）

1. **类型映射错误基本收敛**：infiniteStar→FlyFeather、bigSpinner→Bumper、badelineboost→BadelineBoost 三个"行为完全错位"的实体已重构为正确语义（见第一部分）。`SummitBackgroundManager`→AscendManager、`towerviewer`→Lookout 仍是纯占位。
2. **上次标红的机制硬伤大多已修**：swapblock（任意 dash 触发+0.8s 回程）、fallingblock（Safe 永久固体+climbFall）、starjumpblock（sinks 下沉平台）、zipmover（骑乘触发+非对称 SineIn）、dashblock（state 门控+permanent）、triggerspikes（接触触发+0.4s 延迟+永不收回+深度-50+滞留 0.05s）、refill 满态守卫、spikes 上向守卫、spring SideBounce/SuperBounce、killbox 滞回门控、cloud hitbox 修正。
3. **新发现的硬伤（本轮重点）**：
   - 🔴 **player 墙滑**：`wall_slide_timer` 每帧在滑墙时被重置为满（player/lib.rs:529），导致滑墙永不衰减、速度恒钉在 20px/s 起步档。→ **已修复（本批）**：删掉每帧重置，改为滑墙期间自然衰减、落地重置 1.2、计时耗尽即结束；速度公式保留 `MAX_FALL+(20-MAX_FALL)*t/1.2`。
   - 🔴 **player dash 撞墙**：撞墙即断 dash + 非原版反弹（lib.rs:769-815），原版撞墙不结束 dash。→ **已修复（本批）**：撞墙仅清零该轴速度（对齐 OnCollideH/V），dash 到 0.15s 计时耗尽才自然结束。
   - 🔴 **player ClimbBegin 回体力**：`climb_begin` 回满体力（lib.rs:1211），原版 ClimbBegin 不回体力（Player.cs:3882-3909）。→ **已修复（本批）**：移除显式回体力。
   - 🔴 ~~fallingblock 穿平台缺失~~：**复核后为误报** —— 原版 `MoveVCollideSolids`→`MoveVExactCollideSolids` 在 `moveV>0` 时也会停在 JumpThru 上（Platform.cs:499-506），与我方 physics 停在 one-way/动态平台一致，行为已相符，无需改动。
   - 🔴 **dreamblock 完全无运动**：仅静态 solid，节点 Yoyo/激活/oneUse/梦冲穿行全缺。
    - ✅ **rotate-spinner 已可见**（draw 已实现）。
   - 🔴 **Crystal spinner 整体缺失**。
   - 🔴 **heartgemdoor / invisiblebarrier 纯占位**。→ invisiblebarrier **已修复（本批）**：改为 `collision.solid(true)`（原版即 `Solid`，碰撞有效；重叠时临时 Collidable=false 的过渡技巧未做）。
   - 🟠 **plateau** 右端短 8px + 非 Solid。→ **已修复（本批）**：改回 `104×4 Solid`、collider 右移 8（[x+8,x+112)），与原版 `Collider.Left += 8` 一致。
4. **系统性缺口无进展**：死亡流程+方向、粒子/残影/位移、持物、会话旗标、柱状音效参数、Freeze/相机/rumble —— 依赖宿主架构能力，属 P3。

---

## 一、类型映射（原名 → 原版真实类）

| 地图类型 | 原版分派 | 上次判定 | 本次状态 |
|---|---|---|---|
| `infiniteStar` | `new FlyFeather`（Level.cs:492-493） | ❌ 错位 | ✅ **已重构为 FlyFeather**：20×20@(-10,-10) hitbox、`shielded` 非冲刺→PointBounce(220,+方向修正,RefillDash)、`!singleUse` 3s 重生、`EV_STARFLY`（infinite-star/src/lib.rs）。对照 FlyFeather.cs 全数 ✓（缺粒子/音效/light/bloom） |
| `bigSpinner` | `new Bumper`（Level.cs:792-793） | ❌ 错位 | ✅ **已重构为 Bumper**：Circle≈AABB24、触碰→`EV_LAUNCH`(280,方向)、0.6s 冷却、`fireMode`→die、node 往返 1.818s cube-in-out、depth -50（big-spinner/src/lib.rs）。对照 Bumper.cs ✓（缺 sprite/粒子/音效） |
| `badelineboost` | `new BadelineBoost`（Level.cs:633-635） | ❌ 机制错误 | ✅ **已重构为 BadelineBoost 轨道**：抓取→`EV_CARRIED` 搬运→`(0,-330)` 发射→末节点 SummitLaunch、320px/s 旅行（badeline-boost/src/lib.rs）。对照 BadelineBoost.cs 主干 ✓ |
| `SummitBackgroundManager` | `new AscendManager` | ❌ 占位 | 🔴 仍占位（summitbackground.rs update/draw 空） |
| `towerviewer` | `new Lookout` | ❌ 占位 | 🔴 仍占位（towerviewer.rs update/draw 空） |

---

## 二、Player 插件 vs Player.cs（6335 行）——本轮重审

### 状态机：已实现 8/26，EV 主要对齐

| # | 原版行为（行号） | 我方当前行为（行号） | 状态 |
|---|---|---|---|
| 1 | `wallSlideTimer-=dt`，仅滑墙时衰减（1555-1559）；落地重置 1.2（1575）；速度 `Lerp(160,20,timer/1.2)`（3766） | ✅ 已对齐：首次进入滑墙沿用落地 1.2，滑墙期间递减至 0，速度 20→160 渐快，耗竭即停（529 重置已移除） | ✅ |
| 2 | 地面补 dash 需 `dashRefillCooldown<=0 && onGround && 脚下Solid/JumpThru && !Spikes && !NoRefills && Dashes<Max`（1598-1611,2824-2832） | `grounded && cooldown<=0`→dashes=MAX（300-303）；cooldown 门槛已修，**缺 Spikes/NoRefills 检查** | 🟠 |
| 3 | dash 冷却只在 DashBegin 设一次（4286） | 只在 start_dash/red_dash_build 设置 | ✅ 已修复 |
| 4 | dash 协程 `yield return null` 一帧后才冲（4467-4484） | 当帧即速 240 | 🟡 快约 1 帧 |
| 5 | 撞墙**不**结束 dash，只清零 Speed（3135-3228,3394-3414） | ✅ 撞墙仅清零该轴速（769-809），0.15s 后自然结束；不再反弹 | ✅ |
| 6 | 动量保留仅 X 轴（4480-4484） | X、Y 两轴 | 🟡 近 |
| 7 | 超墙跳需 `DashAttacking && SuperWallJumpAngleCheck`（3811,3826） | 角度判定有，但 ST_NORMAL 内**缺 `dash_attack_timer>0` 门槛**（562-579）；5px/尖刺探测缺 | 🟡 可能误触发 |
| 8 | Boost 0.25s 蓄力 + 80px/s 吸附 + 按输入立即冲出（4715-4746） | EV_BOOST → boost_timer=0.25 + 80px/s 吸附 + 立即冲出（851-885） | ✅ 已修复 |
| 9 | RedBoost：RedDash 撞墙→`StHitSquash(6)`（3410-3413,4748-4767） | 撞墙回 ST_NORMAL（960-962），**无 StHitSquash** | 🟠 |
| 10 | SuperBounce -185 + RefillDash/RefillStamina（2708-2739） | 数值全对，缺 `AutoJump`（宿主无此系统） | 🟠 |
| 11 | ClimbBegin **不回体力**（3882-3909） | ✅ 已修：`climb_begin` 不再回满（1211 已移除） | ✅ |
| 12 | 下落中 `Speed.Y>0 && CanUnDuck && !onGround && jumpGrace<=0` 自动站立（1795-1798） | 无下落自动站立 | 🔴 缺失 |
| 13 | WallJump `forceMoveX=dir, 0.16s` 输入覆盖（2562-2569） | 用 `low_friction_stop=0.16` 全停近似，且时值≠0.15 | 🟠 机制错误 |
| 14 | 26 状态 | 实现 8 个：Normal/Climb/Dash/Boost/RedDash/Launch/SummitLaunch/StarFly（135-142）；缺 Swim/HitSquash/Pickup/DreamDash 及全部 Intro | 🟠 |
| 15 | Die/PlayerDeadBody、Trail/粒子/位移、持物 Holding（1046,2855-3005） | 全缺；死亡仅 EV 无方向；无粒子体系；羽毛 EV_STARFLY 直切无持物 | 🔴 仍缺失 |

**要点**：相比上次审计，本次已修 **墙滑计时 / dash 撞墙 / ClimbBegin 回体力** 三处手感硬伤；其余 EV 交互（refill/boost/launch/bounce/starfly/badeline）语义已与原版大体重合。

---

## 三、核心引擎层 vs Monocle/Celeste

| 我方 | 原版 | 关键差异 |
|---|---|---|
| physics.rs | Actor/Solid/Grid/JumpThru | 越界格=实心（原版越界=false）；无 movementCounter 像素余数；无 LiftSpeed 0.16s 缓存；无 onCollide 回调/挤压；JumpThru 用顶面区间近似；**actor_move 会落在 one-way/动态平台上**（→fallingblock 穿平台缺失的根因，physics.rs:373-388） |
| ecs.rs | Entity/Component/Scene/Tag/Tracker | 无组件系统、无 Tag、无 Tracker、无 Scene 栈；同深度排序依赖 HashMap 迭代序；无 Collidable/Active |
| camera.rs | Camera + Player.CameraTarget | 无 dash 快速镜头/ShakeVector/过渡 lerp |
| input.rs | MInput/VirtualButton | 无 gamepad；buffer 仅 Jump/Dash；PAUSE 键被 main.rs 先消费 |
| draw.rs | Draw | 旋转约定相反；缺 Circle/Text/Outline/SineTexture/线宽 |
| sprites.rs | Sprite/SpriteBank | `copy=` 未实现；Justify/Center/Position 未实现（origin 恒 0,0）；`frames` 的 `a*b` 重复与降序区间不支持 |
| autotiler.rs | Autotiler | 变体确定性哈希 vs `Calc.Random.Choose`；缺 sprites overlay/AnimatedTiles |
| level.rs | Level/LevelData | 无 Session/Bounds/过渡/Wind/Water/DarkRoom/Freeze；jumpThru 烘焙进网格；多区域不支持 |
| wasm_host.rs | — | 更新顺序 HashMap 迭代序（非确定）；事件异步 vs 原版同步调用 |
| main.rs | Celeste.Main | 可变 dt（上限 0.1）vs FixedTimeStep；无场景栈/菜单/暂停/后处理/存档 |
| **renderer.rs** | Monocle 实体列表 | 🔴 **深度排序方向相反**：原版 EntityList `CompareDepth = b.actualDepth - a.actualDepth` 降序=小 depth 后画=在前（EntityList.cs:26）；我方 `sort_by_key(e.depth)` 升序=小 depth 先画=在后（renderer.rs:300）。→ 全部插件 depth 值实际与原版**镜像**，需整体乘 -1 才对；目前部分值恰好"负负得正"（如 bonfire 2000 vs -5 经镜像后视觉对，npc -2000 vs 1000 也对）纯属巧合 |
| **wasm_host 绘制合成** | SpriteBatch | 🔴 插件 draw hook 几何（lines/rects/images/tile_boxes）在 `draw_entities`（按深度排序的精灵）**之后**统一合成（main.rs:329-333），不参与深度排序 → 任何程序化实体永远盖在精灵实体之上，与 depth 无关 |

---

## 四、数据/资源解析层

| 我方 | 原版 | 关键差异 |
|---|---|---|
| binary_packer.rs | BinaryPacker.cs | ✓ 读序/tag 一致；byte/short 未归一 int32；解析失败回退默认值 |
| atlas.rs | Atlas.cs | LINKS 尾段未解析；`load_no_pack` 拍平路径；字典大小写敏感；**当前已能加载并合并全部 atlas（load_atlas_dir）**，插件 `draw_image` 可解析 `objects/flyFeather/*`、`objects/heartdoor/*` 等真实帧 |
| reader.rs | BinaryReader | ✓ varint/UTF-8 一致 |
| spritebank.rs | SpriteBank/SpriteData | 🔴 `copy=` 未实现；Justify/Center/Position 未实现（56 个精灵 origin 0,0）；`frames` `a*b`/降序区间不支持 |
| pack.rs | AreaData.cs + Content/ | 🔴 与真实资源树对不上：期望 `resources/<pack>/<namespace>/`，实际 `resources/Celeste/Celeste/`（pack=Celeste、namespace=Celeste，与 AGENTS.md 文档一致）；章节级信息全缺 |

---

## 五、插件簇逐条差异（本轮重审结论）

### A. 动平台簇

| 实体 | 原版行为（行号） | 我方当前行为（行号） | 状态 |
|---|---|---|---|
| swap-block | 全局 DashListener + 0.8s 回程 + 0.4×回速 + `Lerp(max/3,max,lerp/0.2)` | 全部实现（swap-block:97-122） | ✅ |
| falling-block | 落地→Safe 永久固体（209）；climbFall 侧触宽限（97-149）；**穿过** DashBlock（`thruDashBlocks:true`）；同样会停在 JumpThru（Platform.cs:499-506）；出界 `Bounds+16`；StaticMover 触发 | 会停 one-way/动态平台——**与原版一致（复核纠正）**；climbFall/宽限/hitbox ✓；出界 y>8000 近似 Bounds+16；`thruDashBlocks` 与 StaticMover 触发 `Triggered` 未做 | 🟠 |
| crush-block | OnDashCollide 任何朝向；可中途改向（274-302）；chillout accel125；碾碎 FallingBlock（493-497） | dash 撞墙 emit EV_CRUSH ✓；下 dash 踊顶 emit EV_CRUSH ✓；可中途改向 ✓；chillout accel125 ✓ | ✅ |
| star-jump-block | sinks 下沉 12px 正弦回弹，无升空 | ✅ 已按 sinks 实现，depth -10000 | ✅ |
| zip-mover | 骑乘者启动；去2s/回0.5s SineIn；Depth -9999 | ✅ 全对齐；缺启动 shake | ✅ |
| dash-block | 仅 dash 态触发、super jump 不碎、canDash state 5/10、permanent、Depth -12999 | ✅ 仅 dash 态+门控+permanent；Depth 500（未改 -12999）；无 Freeze(0.05) | 🟡 |
| cloud | JumpThru(32,5) Collider.X=-16；上升钳制 -220；回程 600/弹射 1200 | ✅ hitbox 32×5@(-16,0)、-220、600/1200；弹射缺 `rider.Speed.Y>=0` 校验 | ✅ |
| spring | WallLeft/Right+SideBounce+Floor `Speed.Y>=0`+SuperBounce(-185,回复,state9 梦境不弹) | ✅ 全实现含 EV_SIDE_BOUNCE/EV_SUPER_BOUNCE | ✅ |
| dreamblock | 节点 Yoyo 运动、激活、oneUse、梦冲穿行、BlockedCheck（86-228,346-405） | 🔴 仅静态 solid slab（dream-block:94-144），无任何运动 | 🔴 |
| intro-crusher | 触发/逃逸区 + 1.2s CubeIn 下落 + 会话旗标 `1`/`0b` | 触发区/下落 ✓；**无会话旗标**（每局重压）；挤压=命中即 die 而非 push/OnSquish | 🟡 |
| plateau | `Solid(104×4)` Collider.Left+=8 → [x+8,x+112) | ✅ Solid 104×4@(+8,0)，右缘 8px 缺口已修 | ✅ |
| flutterbird | 离散贝塞尔跳跃 + 玩家接近(<48px)飞离 + 群体连锁（55-115） | 🔴 纯正弦摆线（flutterbird.rs:67-74） | 🔴 |
| bird | 10 种 mode 状态机 + 旗标流程（BirdNPC.cs） | 🔴 仅 idle + FlyAway 上飘近似 | 🔴 |

### B. 危险物簇

| 实体 | 原版行为（行号） | 我方当前行为（行号） | 状态 |
|---|---|---|---|
| spinner | 区域分派：区3/7d→Dust、区5红/区6紫/区10彩虹/其余蓝 Crystal、区9 CoreMode（Level.cs:748-770） | 🔴 **仅 DustStaticSpinner**（spinner:105-133）；Crystal（蓝/红/紫/彩虹）整体缺；无 128px 邻近 culling/LedgeBlocker/眼球 | 🔴 |
    | rotate-spinner | 区10 Star / 区3 Dust / 其余 Blade，均有精灵（Level.cs:720-733；RotateSpinner.cs:64-97） | ✅ **draw 已实现**（rotate-spinner:94），渲染四根刀片，可见 | ✅ |
| track-spinner | 两点间 SineInOut 往返 + PauseTimes + speed 枚举 + startCenter | ✅ 行为已修（track-spinner:81-98,133-157）；缺面积分派（恒尘埃样式）；死亡不停 Moving | 🟡 |
| spikes | Up 向 `Speed.Y>=0 && player.Bottom<=base.Bottom`（224-229）+ 四向门控 + 逐格随机贴图 + Die(dir) + Depth -1 | ✅ 守卫与门控已在（spikes:128-136）、depth -1 ✓；缺随机贴图（恒 `_00`）、死亡方向、tentacles 类型 | 🟡 |
| trigger-spikes | 接触触发 0.4s + 8/s 伸出 + 永不收回 + 滞留 0.05s + Depth -50 + `Die(outwards)`（TriggerSpikes.cs:50-98,173） | ✅ 全部对齐：0.4s/8/s/不收回/0.05s 滞留（trigger-spikes:134-162,211-260）、depth -50 ✓；缺死亡方向/音效；格子索引截断差 1 | ✅ |
| killbox | 高 32、初始 inert、`Bottom<Top-32` 布防、`Top>Bottom+32` 卸防（35-43） | ✅ 32/滞回/初始 inert 全对齐（killbox:21-23,67-85）；缺 assist 无敌弹跳 | ✅ |
| lightbeam | flag 门控、遮挡渐显 alpha、正弦摇曳（38-93） | 🟠 半成品：深度✓实心✓，无 flag/遮挡/摇曳 | 🟠 |
| torch | Session flag 持久、VertexLight/Bloom、1s tween（84-91） | 🟠 手动 overlap 点亮+动画帧 ✓；无 flag 持久、无光源 | 🟠 |
| lamp | 破碎变体 + BloomPoint | 🟠 恒完好帧+锚点偏移；无破碎/泛光 | 🟠 |
| hanginglamp | 完整链条摆动物理 + 撞击音（57-106） | 🟠 静态直线链，无摆动/撞击/光源 | 🟠 |
| wire | 控制点随风摆（32-35） | 🟠 固定 24px 下坠+16 段；无风 | 🟠 |
| cobweb | area 配色、锚点实心校验、离枝随波 | 🟠 固定灰+控制点定值 | 🟠 |
| resort_lantern | 发光循环+Wiggler+撞击音 | 🟡 较完整，仅缺光源脉动 | 🟡 |
| soundSource | `Awake` 即播 FMOD（SoundSourceEntity.cs:22-26） | 🔴 仍占位（soundsource.rs:40-45），音频后端未落地 | 🔴 |

### C. 道具/NPC/互动簇

| 实体 | 原版行为（行号） | 我方当前行为（行号） | 状态 |
|---|---|---|---|
| strawberry | Follower 拖尾/moon/ghost/seeds/golden 条件/collect 演出（200-353） | 🟠 触碰→跟随→safe-ground 收集；缺 winged/ghost/moon/seeds/演出；golden 逻辑不完整 | 🟠 |
| refill | `UseRefill` 满态不消耗（Player.cs:2834-2848） | ✅ 玩家侧守卫 `dashes<want||stamina<20` 已修（player:1500-1511）；插件侧 want 写死 1 | ✅ |
| booster | OnPlayer 立即 Boost + sprite 视觉吸附（113-131,257-266） | ✅ 0.25s 蓄力+80px/s 吸附+RedBoost 在 player 侧；插件只发事件+自转 | ✅ |
| debris | 玩家推动 pushOut / OnExplode / parallax（FloatingDebris.cs:62-76） | 🟠 纯矩形冒烟，无碰撞/推动/旋转；无 parallax | 🟠 |
| clutter | enabled 黑白 70/30% + Deactivate + 碎块/Generator | 🟠 恒 enabled+黑178；enabled/Deactivate/字典全忽略 | 🟠 |
| door | Open close 重启分支 + Sfx + LightOcclude | 🟡 主干 open 在；缺 close 重播/音效/LightOcclude | 🟡 |
| heart-gem-door | 双 Solid + 80px 内计数吸合 + 开门 32px + flag `opened_heartgemdoor_`（136-299） | 🔴 **update 仍空**（heart-gem-door:82），纯渲染占位 | 🔴 |
| checkpoint | SaveData 存档 + blocker + SpawnOffset + EaseLightsOn | 🟡 触发/登记 respawn/flash ✓；无存档语义/SpawnOffset | 🟡 |
| memorial | MemorialText 台词/渐显/音乐（34-81） | 🟠 仅触碰布尔；无对话/fade；depth 0 vs 100 | 🟠 |
| npc | Depth=**1000** + 29 路分派（NPC.cs:44；Level.cs:1082-1180） | 🟠 读 npc attr 即弃，全部同一剪影；depth **-2000**（镜像后≈原版 1000 的"后方"） | 🟠 |
| bonfire | Depth=**-5** + unlit/lit/smoking 三态（38,62-89） | 🔴 depth **2000**（镜像后恰好≈-5 在前，纯巧合）；无三态 | 🔴 |
| intro-car | **继承 JumpThru 可站立** + 受重下沉回弹 + 附属（IntroCar.cs:6,54-72） | 🔴 纯绘制不可站立；depth 1000 vs 1 | 🔴 |
| invisible-barrier | Solid 可碰撞 + 玩家压住自动收回 + ClimbBlocker（24-35） | `collision.solid(true)` 已生效可碰撞（✅）；重叠时 Collidable=false、ClimbBlocker 未做 | 🟠 |
| cliffflag | Flagline 四色/8999 深度（8-27） | 🟡 颜色/深度 ✓；线色略亮；钉距未随机 | 🟡 |

### D. 深度值核对表（我方 vs 原版含义）

> ⚠️ 重要：我方 renderer 升序排序与原版降序**方向相反**（第三节），因此"数值大小"语义不通用。下表列的是视觉层级结论（谁在玩家前面/后面），而非数值直接相等。已按 renderer 实测排序规则推导。

| 实体 | 我方 depth | 原版 depth | 视觉结论（玩家=0） |
|---|---|---|---|
| bonfire | 2000 | -5 | 我方=玩家前 ✓（镜像巧合），原版=玩家前 |
| npc | -2000 | 1000 | 我方=玩家后 ✓（镜像巧合），原版=玩家后 |
| spikes | -1 | -1 | 同向，无碍 |
| triggerspikes | -50 | -50 | 同向，无碍 |
| spinner | -50 | -50 | 同向 |
| zipmover | 500 | -9999 | **意义相反**（我方>0=玩家前，原版<0=玩家前）→ 视觉层级错误 |
| swapblock | 500 | -9999 | 同上 🔴 |
| dashblock | 500 | -12999 | 同上 🔴 |
| starjumpblock | -10000 | -10000 | 同向 ✓ |
| plateau | 300 | Solid 默认 -9000 | 我方=玩家前 ✓（Solid 原版亦在前） |
| cloud | 同上 | 0（默认 Solid 层） | 🟡 |
| heartgemdoor | 500 | 0 | 我方=玩家前，原版=玩家层 → 🟡 |
| memorial | 0 | 100 | 我方=玩家层，原版=玩家后 → 🟡 |
| introcar | 1000 | 1 | 我方=玩家前，原版≈玩家层 → 🟡 |
| killbox | 0 | 0 | ✓（但不可见纯色） |
| cliffflag | 8999 | 8999 | ✓ |

---

## 六、事件总线 vs 直接调用（贯穿全部交互）

| 原版 | 我方 | 现状 |
|---|---|---|
| `player.UseRefill` 玩家侧满态守卫 | EV_REFILL + 玩家侧守卫 | ✅ 已对 |
| `player.Boost/RedBoost`（状态 4/5 + 0.25s 蓄力） | EV_BOOST + 蓄力/红冲 | ✅ |
| `player.SuperBounce` -185 + 资源 | EV_SUPER_BOUNCE | ✅ |
| `player.Die(dir)` 方向 + assist 豁免 | `host::die()` 无方向 | 🔴 死亡方向/assist 缺失 |
| `player.StartStarFly`（状态 19） | EV_STARFLY + ST_STARFLY | ✅ |
| `player.BadelineBoostLaunch`（状态 7） | EV_BADELINE_BOOST + ST_LAUNCH/ST_SUMMIT | ✅ |
| `player.ExplodeLaunch`（Bumper） | EV_LAUNCH + ST_LAUNCH（280） | ✅ 主干；缺 AutoJump/Y≤50→-150 修正 |
| `player.SideBounce` | EV_SIDE_BOUNCE | ✅ |
| DashListener（任意 dash 触发 swapblock） | PLAYER_DASH 事件 | ✅ |
| OnDashCollide（dashblock/crushblock） | dash 期间 emit EV_DASH_BLOCK/EV_CRUSH | 🟠 crush 缺下 dash 踩顶 |
| **玩家侧守卫整体** | 事件+玩家侧 | **复发风险低，已大部覆盖** |

---

## 七、架构级差异汇总

1. **实体交互模型**：原版"实体持有玩家引用、调用玩家方法（玩家侧带守卫）" vs 我方"事件总线 + 实体自扫 overlap + 直改 speed"。事件语义覆盖率已显著提升（见第六节），但 `die()` 无方向、assist 豁免、Freeze、粒子仍阻断一部分还原。
2. **时序原语**：无引擎级 shake/rumble/Freeze/粒子/session flag API，凡依赖这些的行为不可表达（trail、collect 演出、crush 挤压、checkpoint 存档、heartgemdoor 计数等）。
3. **深度语义**：🔴 **renderer 升序 vs 原版降序**，且插件 draw hook 几何在深度排序的精灵之后统一合成（main.rs:329-333）——深度体系双重重错，目前部分值靠镜像巧合正确。
4. **`host_collect` 误用**：已修正大部分（fallingblock 保留、swapblock 不再删除），dashblock permanent 语义正确。
5. **宿主能力缺口**：按类型实体碰撞查询部分存在（`entities_by_type`）；MoveHExact/MoveVExact、hurtbox 分离、模拟杆输入、rumble/Freeze/相机/粒子/音效参数、SaveData/Assists/Inventory、会话旗标仍缺失。
6. **更新顺序**：HashMap 迭代序更新（原版添加序），非确定。

---

## 八、建议修复优先级（本次重审更新）

**P0（手感级硬伤，可在插件层修复）**：
- ✅ ~~player 墙滑~~：已修（滑墙衰减+计时耗尽结束，page 3 顶部）。
- ✅ ~~player dash 撞墙~~：已修（清零轴速而非反弹，计时 0.15s 自然结束）。
- ✅ ~~player ClimbBegin 回体力~~：已修（不再回满）。
- ✅ ~~invisiblebarrier 无碰撞~~：已修（solid(false→true)）。
- ✅ ~~plateau 右缘短 8px + 非 Solid~~：已修（104×4 Solid@+8）。
- 🟠 剩余：dreamblock 节点运动（P1 再排）；Crystal spinner 缺失。

**P1**：
- crushblock：已实现下 dash 踩顶 emit EV_CRUSH；可中途改向；chillout accel125。
- rotate-spinner：已实现可见 draw；扇区分派可后置。
- dreamblock：节点 Yoyo + 激活 + oneUse（不依赖梦冲系统）。
- introcrusher：会话旗标。
- heartgemdoor：依赖 heart 库存/save，先补双 Solid 墙扇。
- introcar：补 JumpThru 可站立。

**P2（还原度补全）**：Crystal spinner、spikes 随机贴图、死亡方向、lightbeam/torch/hanginglamp/wire/cobweb 动画与光源、strawberry 变体、npc 分派、bonfire 三态。

**P3（架构）**：深度排序统一（renderer 改降序 + 全插件深度复核）、插件 draw 参与深度排序、实体稳定排序、碰撞回调、会话旗标与存档 API、粒子/Shake/rumble/Freeze。