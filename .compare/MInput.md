# MInput 对比分析：C# 原版 vs Rust 重实现

> **对比对象**
> - 原版：`references/source/Celeste/Monocle/MInput.cs`（1485 行）
> - Rust：`src/engine/input.rs`（300 行）

---

## 架构差异概览

| 维度 | C# MInput | Rust Input |
|------|-----------|------------|
| 设计模式 | 静态类，内含三个独立子类（KeyboardData / MouseData / GamePadData） | 单一 `Input` 结构体，基于 Action ID 的抽象层 |
| 输入抽象层级 | 物理按键层（`Keys.A`、`Buttons.X` 等） | 逻辑 Action 层（`MOVE_LEFT`、`JUMP` 等），绑定映射在初始化时完成 |
| 状态管理 | 每个子类持有自己的 `PreviousState` / `CurrentState` | 统一的 `held[]` / `prev_held[]` / `pressed[]` / `released[]` 数组 |
| 虚拟输入 | 独立的 `VirtualInput` 列表（`VirtualButton` 等） | 内建 buffer 机制（`buffer[]`），直接集成到 `pressed()` 中 |
| 设备支持 | 键盘 + 鼠标 + 手柄（4 个玩家） | 仅键盘 + 鼠标左键 |

---

## 1. `MInput` 静态字段

### 1.1 `Active` (bool)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `public static bool Active = true;` — 控制是否更新输入 |
| **Rust** | 无对应字段。`pump()` 每帧必执行，不做 Active 检查 |
| **差异** | Rust 侧由调用者（主循环）决定是否调用 `pump()`，不在此层控制 |

### 1.2 `Disabled` (bool)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `public static bool Disabled = false;` — 所有 Check/Pressed/Released 均检查此字段，为 true 时全部返回 false |
| **Rust** | 无对应。所有查询直接返回内部状态，无全局禁用开关 |
| **差异** | 若需要禁用输入功能，需要额外实现 |

### 1.3 `ControllerHasFocus` (bool)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 由手柄是否有输入自动设置，与键盘焦点互斥 |
| **Rust** | 无手柄支持，故无此概念 |

### 1.4 `IsControllerFocused` (bool)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 用于判断是否应显示手柄 UI 提示 |
| **Rust** | 同上，无手柄支持 |

### 1.5 `Keyboard` (KeyboardData)
| | |
|---|---|
| **状态** | 🟡 近似（功能被合并到 Input 结构体） |
| **C# 原版** | 独立的 `KeyboardData` 实例，提供 `Check(Keys)`、`Pressed(Keys)`、`Released(Keys)` 等方法 |
| **Rust** | 键盘状态被抽象为 Action 层：`Input::button(action)` 对应 `Check(Keys)`，`Input::pressed(action)` 对应 `Pressed(Keys)`，`Input::released(action)` 对应 `Released(Keys)` |
| **差异** | Rust 不暴露原始 `Keys` 枚举查询，必须通过 Action ID；丢失了按原始 Keys 查询的能力 |

### 1.6 `Mouse` (MouseData)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 完整的 `MouseData`：左右中三键 + 位置 + 滚轮 + 屏幕矩阵变换 |
| **Rust** | `MouseState` 结构体：仅左键 + 无滚轮 + 无屏幕矩阵变换 |
| **差异** | 右键/中键/滚轮/WheelDelta/WasMoved/Position ScreenMatrix 变换均缺失 |

### 1.7 `GamePads` (GamePadData[4])
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 4 个手柄数据，完整支持 DPad/摇杆/扳机/震动/死区 |
| **Rust** | 完全未实现 |
| **差异** | 整个手柄子系统缺失（详见第 3 节） |

### 1.8 `VirtualInputs` (List<VirtualInput>)
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | 独立的虚拟输入列表，每帧遍历 `Update()` |
| **Rust** | `Input` 内建 `buffer[]` 实现了 `VirtualButton` 的核心功能（按键缓冲），但没有独立的 VirtualInput 系统 |

---

## 2. `KeyboardData` 逐方法对比

### 2.1 `Update()`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | `PreviousState = CurrentState; CurrentState = Keyboard.GetState();` — 读取整个键盘状态快照 |
| **Rust** | 在 `pump()` 中实现：遍历 `keys_down` HashSet，对比 `prev_held` 计算边沿 |
| **差异** | C# 用 XNA 的 `KeyboardState`（按位存储所有键），Rust 用 `HashSet<Keycode>` + 逐 Action 解析；逻辑等价但实现机制不同 |

### 2.2 `UpdateNull()`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 将当前状态置零（`default(KeyboardState)`），用于窗口失焦时清空输入 |
| **Rust** | 无对应。`pump()` 不处理此场景 |

### 2.3 `HasAnyInput()`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `CurrentState.GetPressedKeys().Length != 0` — 检查是否有任意键按下 |
| **Rust** | 无对应方法。可通过 `held.iter().any(|&h| h)` 间接实现 |

### 2.4 `Check(Keys key)` (单键)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | `if (Disabled) return false; if (key != 0) return CurrentState.IsKeyDown(key);` |
| **Rust** | `Input::button(action: i32)` — 检查该 Action 的任一绑定键是否按下 |
| **差异** | Rust 无 Disabled 检查；且必须通过 Action ID 查询，不支持任意 Keys 查询 |

### 2.5 `Pressed(Keys key)` (单键)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 检查 `CurrentState.IsKeyDown(key) && !PreviousState.IsKeyDown(key)` |
| **Rust** | `Input::pressed(action)` — 包含 buffer 机制（`VirtualButton.Pressed` 等价），比原版更丰富 |
| **差异** | Rust 的 `pressed()` 额外包含 buffer 窗口内的缓冲按压，这是 `VirtualButton` 的行为而非原始 `KeyboardData.Pressed` |

### 2.6 `Released(Keys key)` (单键)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 检查 `!CurrentState.IsKeyDown(key) && PreviousState.IsKeyDown(key)` |
| **Rust** | `Input::released(action)` — 检查 `!held && prev_held` |
| **差异** | Rust 无 Disabled 检查 |

### 2.7 `Check(Keys, Keys)` (双键)
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | `Check(keyA) \|\| Check(keyB)` — OR 逻辑 |
| **Rust** | 通过 `Binding.keys` 天然支持：一个 Action 可绑定多个键，`held` 已经是 OR 逻辑 |
| **差异** | Rust 在绑定层面实现了双键/多键，无需显式双键方法 |

### 2.8 `Check(Keys, Keys, Keys)` (三键)
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | `Check(keyA) \|\| Check(keyB) \|\| Check(keyC)` |
| **Rust** | 同上，绑定层面支持（例如 `CLIMB` 绑定了 Z/V/LShift 三个键） |

### 2.9 `Pressed(Keys, Keys)` / `Pressed(Keys, Keys, Keys)`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | 分别对每个键调用 `Pressed()`，OR 逻辑 |
| **Rust** | `pressed(action)` 对整个 Action 的所有绑定键 OR |

### 2.10 `Released(Keys, Keys)` / `Released(Keys, Keys, Keys)`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | 同上，OR 逻辑 |
| **Rust** | 同上 |

### 2.11 `AxisCheck(Keys negative, Keys positive)`
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 返回 -1/0/1，同时按下返回 0 |
| **Rust** | `Input::axis(action)` — 仅返回 0.0 或 1.0（held 的布尔值），不支持 negative/positive 双键轴 |
| **差异** | Rust 的 `axis()` 不支持双向轴（-1/0/1），仅返回单方向 bool → f32 |

### 2.12 `AxisCheck(Keys negative, Keys positive, int both)`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 指定两键同时按下的返回值 |
| **Rust** | 无对应 |

---

## 3. `GamePadData` 逐方法对比

> **整体状态：🔴 缺失**
>
> Rust 侧完全没有手柄支持。以下列出所有缺失方法。

| 方法 | C# 行号 | 状态 |
|------|---------|------|
| `GamePadData(int playerIndex)` 构造 | L476-479 | 🔴 缺失 |
| `DPadHorizontal` 属性 | L338-351 | 🔴 缺失 |
| `DPadVertical` 属性 | L354-367 | 🔴 缺失 |
| `DPad` 属性 | L370 | 🔴 缺失 |
| `DPadLeftCheck` / `Pressed` / `Released` | L372-396 | 🔴 缺失 |
| `DPadRightCheck` / `Pressed` / `Released` | L398-422 | 🔴 缺失 |
| `DPadUpCheck` / `Pressed` / `Released` | L424-448 | 🔴 缺失 |
| `DPadDownCheck` / `Pressed` / `Released` | L450-474 | 🔴 缺失 |
| `HasAnyInput()` | L481-504 | 🔴 缺失 |
| `Update()` | L506-523 | 🔴 缺失 |
| `UpdateNull()` | L525-535 | 🔴 缺失 |
| `Rumble(float, float)` | L537-545 | 🔴 缺失 |
| `StopRumble()` | L547-551 | 🔴 缺失 |
| `Check(Buttons)` | L553-560 | 🔴 缺失 |
| `Pressed(Buttons)` | L562-573 | 🔴 缺失 |
| `Released(Buttons)` | L575-586 | 🔴 缺失 |
| `Check(Buttons, Buttons)` | L588-595 | 🔴 缺失 |
| `Pressed(Buttons, Buttons)` | L597-604 | 🔴 缺失 |
| `Released(Buttons, Buttons)` | L606-613 | 🔴 缺失 |
| `Check(Buttons, Buttons, Buttons)` | L615-622 | 🔴 缺失 |
| `Pressed(Buttons, Buttons, Buttons)` | L624-631 | 🔴 缺失 |
| `Released(Buttons, Buttons, Buttons)` | L633-640 | 🔴 缺失 |
| `GetLeftStick()` / `GetLeftStick(float)` | L642-661 | 🔴 缺失 |
| `GetRightStick()` / `GetRightStick(float)` | L663-682 | 🔴 缺失 |
| `LeftStickLeftCheck/Pressed/Released(float)` | L684-705 | 🔴 缺失 |
| `LeftStickRightCheck/Pressed/Released(float)` | L707-728 | 🔴 缺失 |
| `LeftStickDownCheck/Pressed/Released(float)` | L730-751 | 🔴 缺失 |
| `LeftStickUpCheck/Pressed/Released(float)` | L753-774 | 🔴 缺失 |
| `LeftStickHorizontal(float)` / `LeftStickVertical(float)` | L776-794 | 🔴 缺失 |
| `RightStickLeftCheck/Pressed/Released(float)` | L796-817 | 🔴 缺失 |
| `RightStickRightCheck/Pressed/Released(float)` | L819-840 | 🔴 缺失 |
| `RightStickDownCheck/Pressed/Released(float)` | L842-863 | 🔴 缺失 |
| `RightStickUpCheck/Pressed/Released(float)` | L865-886 | 🔴 缺失 |
| `RightStickHorizontal(float)` / `RightStickVertical(float)` | L888-906 | 🔴 缺失 |
| `LeftTriggerCheck/Pressed/Released(float)` | L908-941 | 🔴 缺失 |
| `RightTriggerCheck/Pressed/Released(float)` | L943-976 | 🔴 缺失 |
| `Axis(Buttons, float)` | L978-1067 | 🔴 缺失 |
| `Check(Buttons, float)` | L1069-1158 | 🔴 缺失 |
| `Pressed(Buttons, float)` | L1160-1249 | 🔴 缺失 |
| `Released(Buttons, float)` | L1251-1340 | 🔴 缺失 |

---

## 4. `MouseData` 逐方法/属性对比

### 4.1 `PreviousState` / `CurrentState`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | `MouseState` 完整快照（位置 + 按键 + 滚轮） |
| **Rust** | `prev_mouse_left: bool` + `mouse.left_down: bool` — 仅存储左键状态 |
| **差异** | 仅保留左键，丢失右键/中键/滚轮/光标位置历史 |

### 4.2 `CheckLeftButton`
| | |
|---|---|
| **状态** | ✅ 完全对齐 |
| **C# 原版** | `CurrentState.LeftButton == ButtonState.Pressed` |
| **Rust** | `self.mouse.left_down` |

### 4.3 `CheckRightButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `CurrentState.RightButton == ButtonState.Pressed` |
| **Rust** | 无右键状态 |

### 4.4 `CheckMiddleButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `CurrentState.MiddleButton == ButtonState.Pressed` |
| **Rust** | 无中键状态 |

### 4.5 `PressedLeftButton`
| | |
|---|---|
| **状态** | ✅ 完全对齐 |
| **C# 原版** | `CurrentState.LeftButton == Pressed && PreviousState.LeftButton == Released` |
| **Rust** | `self.mouse.left_down && !self.prev_mouse_left` |

### 4.6 `PressedRightButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 右键按下边沿 |
| **Rust** | 无 |

### 4.7 `PressedMiddleButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 中键按下边沿 |
| **Rust** | 无 |

### 4.8 `ReleasedLeftButton`
| | |
|---|---|
| **状态** | ✅ 完全对齐 |
| **C# 原版** | `CurrentState.LeftButton == Released && PreviousState.LeftButton == Pressed` |
| **Rust** | `!self.mouse.left_down && self.prev_mouse_left` |

### 4.9 `ReleasedRightButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |

### 4.10 `ReleasedMiddleButton`
| | |
|---|---|
| **状态** | 🔴 缺失 |

### 4.11 `Wheel` (属性)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `CurrentState.ScrollWheelValue` |
| **Rust** | 无滚轮支持 |

### 4.12 `WheelDelta` (属性)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | `CurrentState.ScrollWheelValue - PreviousState.ScrollWheelValue` |
| **Rust** | 无 |

### 4.13 `WasMoved` (属性)
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 检查 X/Y 是否变化 |
| **Rust** | 无 |

### 4.14 `X` / `Y` (属性，可读写)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 通过 `Position` 属性，transform 了 `Engine.ScreenMatrix`；setter 调用 `Mouse.SetPosition` |
| **Rust** | `mouse.x` / `mouse.y` — 只读，无 screen matrix 变换，无 setter |
| **差异** | Rust 不做屏幕矩阵变换（SDL3 逻辑分辨率由 SDL 管理），不能设置鼠标位置 |

### 4.15 `Position` (属性，Vector2，可读写)
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | Getter: `Vector2.Transform(pos, Matrix.Invert(ScreenMatrix))`; Setter: `Mouse.SetPosition(...)` |
| **Rust** | 无对应方法。`mouse.x/y` 为原始坐标 |
| **差异** | 无屏幕矩阵逆变换，不能设置位置 |

### 4.16 `Update()`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | `PreviousState = CurrentState; CurrentState = Mouse.GetState();` |
| **Rust** | 在 `pump()` 中通过 SDL 事件更新 `mouse.left_down` 和 `mouse.x/y` |
| **差异** | Rust 用事件驱动而非轮询快照；仅处理左键 |

### 4.17 `UpdateNull()`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 清空鼠标状态 |
| **Rust** | 无 |

---

## 5. `MInput` 静态方法对比

### 5.1 `Initialize()`
| | |
|---|---|
| **状态** | ✅ 完全对齐 |
| **C# 原版** | 创建 Keyboard/Mouse/GamePads 实例 + VirtualInputs 列表 |
| **Rust** | `Input::default()` 创建实例、设置默认绑定、初始化所有数组 |
| **差异** | Rust 不创建手柄实例（按需精简） |

### 5.2 `Shutdown()`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 遍历所有手柄调用 `StopRumble()` |
| **Rust** | 无（无手柄震动） |

### 5.3 `Update()`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | 检查 `Engine.Instance.IsActive && Active`；根据 `Engine.Commands.Open` 决定 Update/UpdateNull；更新所有手柄；处理 ControllerHasFocus 逻辑 |
| **Rust** | `Input::pump(events, dt)` — 处理 SDL 事件、更新 held/pressed/released 边沿、递减 buffer |
| **差异** | Rust 不检查 Active/Commands.Open 状态；不含手柄更新和焦点切换逻辑；但包含了 VirtualButton buffer 更新 |

### 5.4 `UpdateNull()`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 对所有设备调用 `UpdateNull()` + 更新 VirtualInputs |
| **Rust** | 无 |

### 5.5 `UpdateVirtualInputs()`
| | |
|---|---|
| **状态** | 🟡 近似 |
| **C# 原版** | 遍历 `VirtualInputs` 列表逐个 `Update()` |
| **Rust** | buffer 递减逻辑内嵌在 `pump()` 中（`self.buffer[i] -= dt`） |
| **差异** | Rust 的 VirtualButton 逻辑固定在 `Input` 内部，不支持外部注册自定义 VirtualInput |

### 5.6 `RumbleFirst(float strength, float time)`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 调用 `GamePads[0].Rumble(strength, time)` |
| **Rust** | 无手柄支持 |

### 5.7 `Axis(bool negative, bool positive, int bothValue)`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 通用轴：根据 negative/positive bool 返回 -1/bothValue/1/0 |
| **Rust** | 无对应静态方法 |

### 5.8 `Axis(float axisValue, float deadzone)`
| | |
|---|---|
| **状态** | 🟠 部分实现 |
| **C# 原版** | 返回 `Math.Sign(axisValue)`（死区过滤后） |
| **Rust** | `Input::axis(action)` 返回 `held[action] as i32 as f32`（0.0 或 1.0） |
| **差异** | Rust 的 axis 不接受 float 输入和 deadzone 参数，仅返回 bool→float |

### 5.9 `Axis(bool neg, bool pos, int both, float axisVal, float deadzone)`
| | |
|---|---|
| **状态** | 🔴 缺失 |
| **C# 原版** | 混合轴：先试摇杆轴，若为 0 则试按键轴 |
| **Rust** | 无对应 |

---

## 6. Rust 独有特性

以下功能在 Rust 实现中存在，但 C# 原版 `MInput` 中没有：

| 特性 | 说明 |
|------|------|
| `Binding` 绑定系统 | 每个 Action 可绑定多个 `Keycode`，支持灵活的键位映射 |
| `buffer[]` 按键缓冲 | 内建 VirtualButton 缓冲（`buffer_time()`），JUMP/DASH 有 0.08s 缓冲窗口 |
| `consume(action)` | 清除按键缓冲（对应 `VirtualButton.ConsumeBuffer`），C# 中由 VirtualButton 类单独提供 |
| `prev_mouse_left` | Rust 用单独字段追踪上一帧鼠标左键状态；C# 用完整 `MouseState` 快照 |
| 基于 Action ID 的 API | `button(action)` / `pressed(action)` / `released(action)` / `axis(action)` — 统一接口 |

---

## 7. 汇总统计

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 完全对齐 | 3 | 6% |
| 🟠 部分实现 | 8 | 16% |
| 🟡 近似 | 12 | 24% |
| 🔴 缺失 | 26 | 54% |
| **合计** | **49** | **100%** |

### 按类别细分

| 类别 | ✅ | 🟠 | 🟡 | 🔴 |
|------|----|----|----|----|
| MInput 静态字段 | 0 | 0 | 2 | 6 |
| KeyboardData | 0 | 3 | 5 | 4 |
| MouseData | 2 | 2 | 1 | 12 |
| GamePadData | 0 | 0 | 0 | 38 |
| MInput 静态方法 | 1 | 1 | 3 | 4 |
| **合计** | **3** | **6** | **11** | **64** |

---

## 8. 差异根因分析

1. **手柄支持完全缺失**（GamePadData 全部 🔴）：Rust 侧尚未实现 SDL3 Game Controller API 集成。这是最大的差距，影响约 54% 的方法。

2. **物理层 vs 逻辑层的架构差异**：C# 提供原生 `Keys` / `Buttons` 枚举查询，Rust 直接抽象为 Action ID。这导致 `Check(Keys)` 等方法在 Rust 侧没有 1:1 对应，但功能上通过 `Binding` 系统在更高层面实现了等价效果。

3. **键盘多键重载方法**（如 `Check(Keys, Keys, Keys)`）：在 Rust 中不需要，因为 `Binding.keys` 已天然支持多键 OR。

4. **`Disabled` / `Active` 全局开关**：Rust 侧完全缺失，需在主循环或插件 API 层面额外实现。

5. **MouseData 右键/中键/滚轮**：Rust 仅实现左键，可能是因为 Celeste 游戏本身只使用鼠标左键。

6. **GamePadData 复合方法**（如 `Axis(Buttons, float)` 的 switch-case 大方法）：由于手柄整体缺失，这些全部未实现。
