# Particle / ParticleType / ParticleSystem 对比分析：Monocle (C#) vs Ruleste (Rust)

> 源码对照
> - C# 原版：`references/source/Celeste/Monocle/Particle.cs`（140 行）、`ParticleType.cs`（202 行）、`ParticleSystem.cs`（164 行）、`ParticleEmitter.cs`（95 行）＋ Celeste 侧 `ParticleTypes.cs`（1393 行）
> - Rust 实现：`crates/ruleste-core/src/engine/particles.rs`（145 行）＋ `host_emit_particle` FFI（`crates/ruleste-plugins-api/src/host.rs`）＋ `src/interface/renderer.rs`（作为矩形绘制）

## 架构差异

原版是 Monocle 引擎的完整粒子子系统：`ParticleType`（参数模板）＋ ring-buffer `ParticleSystem`（最多 N 个，槽位覆盖）＋ `ParticleEmitter`（定时发射器）＋ Celeste 侧 1393 行 `ParticleTypes`（预定义全部音效/视觉粒子）。粒子可用图集 `MTexture`（含旋转、Spin、方向角、摩擦、加速度、多 ColorMode/FadeMode）渲染。

Ruleste 把粒子做成**宿主侧一行单例池**：插件用 `host_emit_particle` 提交 `(pos, vel, acc, life, color, size)`，宿主积分并渲染成**填充矩形**。没有模板/Chooser/方向/旋转/摩擦/Spin，也没有 `ParticleEmitter`。

## 一、`ParticleType` 字段逐项

| C# `ParticleType` 字段 | Rust 对应 | 状态 |
|------------------------|-----------|------|
| `Source` / `SourceChooser`（MTexture） | 固定方块（无纹理） | 🔴 |
| `Color` / `Color2` | 单 `color: Color`（无第二色） | 🔴 |
| `ColorMode`（Static/Choose/Blink/Fade） | 仅线性 alpha 淡出 | 🔴 |
| `FadeMode`（None/Linear/Late/InAndOut） | 恒 Linear（按 life 比例降 alpha） | 🟡 |
| `SpeedMin / SpeedMax` | 单 `vx,vy`（无随机范围） | 🟡 |
| `SpeedMultiplier` | 缺失（指数衰减速度） | 🔴 |
| `Acceleration` | `ax,ay` | ✅ |
| `Friction` | 缺失 | 🔴 |
| `Direction / DirectionRange` | 缺失（插件自行算初速） | 🔴 |
| `LifeMin / LifeMax` | 单 `life` | 🟡 |
| `Size / SizeRange` | 单 `size`（半宽） | 🟡 |
| `SpinMin / SpinMax / SpinFlippedChance` | 缺失 | 🔴 |
| `RotationMode`（None/Random/SameAsDirection） | 缺失（方块无旋转） | 🔴 |
| `ScaleOut` | 缺失（大小恒等） | 🔴 |
| `UseActualDeltaTime` | 缺失（统一 dt） | 🔴 |

## 二、`Particle` 字段

| C# | Rust | 状态 |
|----|------|------|
| `Type` / `Track` / `Source` | 无（无模板、无跟随实体） | 🔴 |
| `Position / Speed` | `x/y/vx/vy` | ✅ |
| `Active` | `life > 0` 隐含 | ✅ |
| `Color / StartColor` | `color` | 🟡 |
| `Life / StartLife` | `life` / `max_life` | ✅ |
| `Size / StartSize` | 单 `size` | 🟡 |
| `Rotation / Spin` | 缺失 | 🔴 |
| `ColorSwitch` | 缺失 | 🔴 |

`Particle.Update`：C# 先按 `FadeMode/ColorMode` 算色，再积分速度/摩擦力/`SpeedMultiplier`，并按 `ScaleOut` 缩放大小。
Rust `Particle::update`：只 `v += a*dt; p += v*dt; life -= dt`。之后 `alpha()` 线性淡出。**旋转/大小变化/摩擦/第二色完全缺**。

## 三、`ParticleSystem`

| C# | Rust `ParticleSystem` | 状态 |
|----|----------------------|------|
| 固定容量 ring buffer（`maxParticles`） | `Vec` 动态扩容 | 🟡 |
| `Clear()` | `clear()` | ✅ |
| `ClearRect(rect, inside)` | 缺失 | 🔴 |
| `Update()` | `update(dt)` | ✅ |
| `Render()` / `Render(alpha)` | `append_to_rects`（推入矩形列表） | 🟠 |
| `Simulate(duration, interval, emitter)` | 缺失 | 🔴 |
| `Add(particle)` | `emit(Particle)` | ✅ |
| `Emit(type, position[, dir/color/amount/range/track])` 重载族 | `emit_particle(x,y,vx,vy,ax,ay,life,color,size)` | 🟡 |
| 槽位覆盖（覆盖最旧） | 无限增长直到 `Update` 清理 | 🟡 |

## 四、`ParticleEmitter`

| C# | Rust | 状态 |
|----|------|------|
| `Interval` 定时 `Emit` | 缺失（插件每帧自行调用 `emit_particle`） | 🔴 |
| `Range`（矩形随机散布） | 缺失 | 🔴 |
| `Amount` | 缺失（插件手动多次调用） | 🔴 |
| `Track`（跟随实体） | 缺失 | 🔴 |
| `SimulateCycle()` / `Simulate(duration)`（预生成粒子） | 缺失 | 🔴 |

## 五、宿主侧接口

`host_emit_particle(x,y,vx,vy,ax,ay,life,r,g,b,a,size)`（`crates/ruleste-plugins-api` 的 `emit_particle`）已经能让插件发基础直线粒子。宿主在 `WasmHost::update` 后推进粒子，在 `draw` 末尾把它们转成 `draw::Rect`（含 alpha）。对比原版 `ParticleSystem.Emit(type, amount, position, range, direction)`：

- 缺 `amount`：插件要自己循环调用
- 缺 `range`（矩形随机位）：插件可在宿主 rng 上自己算
- 缺 `track`（实体跟随）：没有
- 缺 `direction`（角度 + SpeedMin/Max 随机）：插件自己算 `vx,vy`

**渲染差异**：Ruleste 粒子永远是实心方块且无旋转；原版用 `MTexture`（通常 4×4 像素斑点）并支持 `Rotation/Spin/Size` 缩放。视觉保真有差距，尤其毛发、dust、debris 类粒子。

## 六、`ParticleTypes.cs`（Celeste 侧 1393 行）

原版预定义了几十个静态模板（`Dust`、`Booster`、`Wall`、`DreamBlock`、`Fall`、`Spark`、`Crystal`、`Fire`、`Snow` 等）。Ruleste **没有**对应 `ParticleType` 表：插件直接硬编码 `emit_particle` 参数。好处是插件独立、无需共享模板；代价是无统一视觉常量、无法复用原版的颜色/大小/速度组合。

## 七、总结

| 维度 | 状态 | 说明 |
|------|------|------|
| 积分（pos/vel/acc/life） | ✅ | 核心物理一致 |
| Alpha 淡出 | 🟡 | 只有线性；Late/InAndOut 缺 |
| 模板/Chooser | 🔴 | 无 `ParticleType` |
| 方向/随机速度/摩擦/速度倍率 | 🔴 | 插件自己算 |
| 旋转/Spin/缩放/图集贴图 | 🔴 | 仅方块 |
| 发射器（间隔/范围/数量/跟随） | 🔴 | 插件手动循环 |
| 预定义粒子库 | 🔴 | 无 |

### 建议补全优先级

| 优先级 | 项目 | 原因 |
|--------|------|------|
| P1 | 纹理 + 旋转/缩放 | 很多核心反馈（dust、death、crystal）需要 |
| P1 | `amount`/`range`/`direction` 便捷参数 | 插件侧大量重复代码 |
| P2 | `ParticleType` 共享模板（Rust 侧 const 表） | 对齐原版 `ParticleTypes.cs` |
| P2 | `FadeMode` Late/InAndOut | 视觉差异明显 |
| P3 | `ParticleEmitter` 定时/跟随 | 等模板化之后再说 |
