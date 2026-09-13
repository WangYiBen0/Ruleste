//! Virtual input layer. Maps physical keys (via the `VirtualButton`/binding
//! model of the original `Input`) onto the abstract action ids exposed to
//! plugins. Default bindings mirror `Settings.SetDefaultKeyboardControls`.
//!
//! Also handles mouse input (position and left-button), mirroring `MInput.MouseData`.
//! Mouse coordinates are translated to world units using the camera offset so
//! plugins can query world-space mouse positions.

use std::collections::HashSet;

use ruleste_plugins_api::types::input as act;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::mouse::MouseButton;

#[derive(Debug, Clone, Default)]
pub struct Binding {
    pub keys: Vec<Keycode>,
}

impl Binding {
    fn add(&mut self, keys: &[Keycode]) {
        self.keys.extend_from_slice(keys);
    }
}

/// Buffer window per action, mirroring `Input.cs` `new VirtualButton(..., 0.08f, 0.2f)`.
/// A `pressed()` stays true for this long after the physical key goes down,
/// letting a jump/dash input register just before the player becomes able to
/// act (the basis of jump-buffered wavedashes).
fn buffer_time(action: usize) -> f32 {
    const JUMP: usize = act::JUMP as usize;
    const DASH: usize = act::DASH as usize;
    match action {
        JUMP | DASH => 0.08,
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Default)]
pub struct MouseState {
    pub x: f32,
    pub y: f32,
    pub left_down: bool,
    pub left_pressed: bool,
    pub left_released: bool,
}

impl MouseState {
    fn new() -> MouseState {
        MouseState::default()
    }
}

/// A `VirtualIntegerAxis` equivalent for the movement stick, mirroring the
/// original `Input.MoveX`/`MoveY` (`VirtualIntegerAxis` with the default
/// `OverlapBehavior.TakeNewer`). When both the negative and positive bindings
/// are held, the value flips to the previously held direction instead of
/// cancelling out, so a left→hold-right transition never dithers through 0.
#[derive(Debug, Clone)]
struct IntegerAxis {
    value: i32,
    previous: i32,
    turned: bool,
}

impl IntegerAxis {
    fn new() -> IntegerAxis {
        IntegerAxis {
            value: 0,
            previous: 0,
            turned: false,
        }
    }

    /// Mirrors `VirtualIntegerAxis.Update()`: reads the current held state of
    /// the negative/positive bindings and applies the overlap behavior.
    fn update(&mut self, negative: bool, positive: bool) -> i32 {
        self.previous = self.value;
        match (negative, positive) {
            (true, true) => {
                // OverlapBehaviors.TakeNewer: flip to the last direction held.
                if !self.turned {
                    self.value = -self.value;
                    self.turned = true;
                }
            }
            (true, false) => {
                self.turned = false;
                self.value = -1;
            }
            (false, true) => {
                self.turned = false;
                self.value = 1;
            }
            (false, false) => {
                self.turned = false;
                self.value = 0;
            }
        }
        self.value
    }
}

#[derive(Debug)]
pub struct Input {
    pub bindings: [Binding; act::COUNT as usize],
    keys_down: HashSet<Keycode>,
    held: [bool; act::COUNT as usize],
    prev_held: [bool; act::COUNT as usize],
    pressed: [bool; act::COUNT as usize],
    released: [bool; act::COUNT as usize],
    /// VirtualButton `bufferCounter`: counts down while the binding is held
    /// after a fresh press; zeroed immediately if the key is released.
    buffer: [f32; act::COUNT as usize],
    /// Mouse state, updated from SDL mouse events during `pump`.
    pub mouse: MouseState,
    prev_mouse_left: bool,
    /// Latched movement axes, mirroring `Input.MoveX`/`MoveY`
    /// (`VirtualIntegerAxis`, `OverlapBehavior.TakeNewer`). Kept as the
    /// authoritative -1/0/1 source so plugins don't re-derive it from the two
    /// directional actions (which would cancel out when both are held).
    move_x: IntegerAxis,
    move_y: IntegerAxis,
}

impl Default for Input {
    fn default() -> Input {
        let mut bindings = std::array::from_fn(|_| Binding::default());
        let mut set = |action: usize, keys: &[Keycode]| bindings[action].add(keys);

        // `Settings.SetDefaultKeyboardControls` equivalents.
        set(act::MOVE_LEFT as usize, &[Keycode::Left]);
        set(act::MOVE_RIGHT as usize, &[Keycode::Right]);
        set(act::MOVE_UP as usize, &[Keycode::Up]);
        set(act::MOVE_DOWN as usize, &[Keycode::Down]);
        set(
            act::CLIMB as usize,
            &[Keycode::Z, Keycode::V, Keycode::LShift],
        );
        set(act::JUMP as usize, &[Keycode::C]);
        set(act::DASH as usize, &[Keycode::X]);
        set(act::START as usize, &[Keycode::Return]);
        set(act::CONFIRM as usize, &[Keycode::C]);
        set(act::CANCEL as usize, &[Keycode::X, Keycode::Backspace]);
        set(act::QUICK_RESTART as usize, &[Keycode::R]);
        set(act::PAUSE as usize, &[Keycode::Escape]);

        Input {
            bindings,
            keys_down: HashSet::new(),
            held: [false; act::COUNT as usize],
            prev_held: [false; act::COUNT as usize],
            pressed: [false; act::COUNT as usize],
            released: [false; act::COUNT as usize],
            buffer: [0.0; act::COUNT as usize],
            mouse: MouseState::new(),
            prev_mouse_left: false,
            move_x: IntegerAxis::new(),
            move_y: IntegerAxis::new(),
        }
    }
}

impl Input {
    /// Processes SDL key and mouse events, refreshes `held`, and computes the
    /// `pressed`/`released` edges for this frame. Call once per frame before
    /// plugins update. Quit/resize events are ignored here so the caller can
    /// handle them separately. `dt` drives the VirtualButton press buffer
    /// (mirroring `VirtualButton.Update`, which decrements `bufferCounter` by
    /// `Engine.DeltaTime`).
    pub fn pump(&mut self, events: impl IntoIterator<Item = Event>, dt: f32) {
        for event in events {
            match event {
                Event::KeyDown {
                    keycode: Some(kc),
                    repeat,
                    ..
                } if !repeat => {
                    self.keys_down.insert(kc);
                }
                Event::KeyUp {
                    keycode: Some(kc), ..
                } => {
                    self.keys_down.remove(&kc);
                }
                Event::MouseMotion { x, y, .. } => {
                    self.mouse.x = x;
                    self.mouse.y = y;
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    self.mouse.left_down = true;
                    self.mouse.x = x;
                    self.mouse.y = y;
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    self.mouse.left_down = false;
                    self.mouse.x = x;
                    self.mouse.y = y;
                }
                // Mirror `MInput.UpdateNull()`: when the window loses focus,
                // clear all held keys so the player doesn't walk off-screen
                // while alt-tabbed.
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => {
                    self.keys_down.clear();
                }
                _ => {}
            }
        }

        for (i, binding) in self.bindings.iter().enumerate() {
            let held = binding.keys.iter().any(|k| self.keys_down.contains(k));
            let fresh_press = held && !self.prev_held[i];
            self.pressed[i] = fresh_press;
            self.released[i] = !held && self.prev_held[i];
            self.held[i] = held;
            // VirtualButton.Update: decrement the buffer; a fresh press re-arms
            // it; letting go zeroes it (no held/repeat state to keep it alive).
            self.buffer[i] -= dt;
            if fresh_press {
                self.buffer[i] = buffer_time(i);
            } else if !held {
                self.buffer[i] = 0.0;
            }
        }
        self.prev_held = self.held;

        // Mouse edge detection (left button only, mirroring
        // `MInput.MouseData.Check/Pressed/Released`).
        self.mouse.left_pressed = self.mouse.left_down && !self.prev_mouse_left;
        self.mouse.left_released = !self.mouse.left_down && self.prev_mouse_left;
        self.prev_mouse_left = self.mouse.left_down;

        // Latched movement axes (`VirtualIntegerAxis.Update` runs after the
        // keyboard state update, using the current held state).
        self.move_x.update(
            self.held[act::MOVE_LEFT as usize],
            self.held[act::MOVE_RIGHT as usize],
        );
        self.move_y.update(
            self.held[act::MOVE_UP as usize],
            self.held[act::MOVE_DOWN as usize],
        );
    }

    pub fn axis(&self, action: i32) -> f32 {
        match action {
            act::MOVE_LEFT => (self.held[act::MOVE_LEFT as usize] as i32) as f32,
            act::MOVE_RIGHT => (self.held[act::MOVE_RIGHT as usize] as i32) as f32,
            act::MOVE_UP => (self.held[act::MOVE_UP as usize] as i32) as f32,
            act::MOVE_DOWN => (self.held[act::MOVE_DOWN as usize] as i32) as f32,
            _ => 0.0,
        }
    }

    /// Mirrors `MInput.AxisCheck(negative, positive)`: returns `-1`/`0`/`1`
    /// (positive when `positive` is held alone, negative when `negative` is
    /// held alone, `0` when neither or both are held).
    pub fn axis_check(&self, negative: i32, positive: i32) -> i32 {
        if self.button(negative) {
            if self.button(positive) { 0 } else { -1 }
        } else if self.button(positive) {
            1
        } else {
            0
        }
    }

    /// The latched horizontal movement axis, mirroring `Input.MoveX.Value`
    /// (`VirtualIntegerAxis` with `OverlapBehavior.TakeNewer`): when both
    /// directions are held the value flips to the last-held direction.
    #[must_use]
    pub fn move_x(&self) -> i32 {
        self.move_x.value
    }

    /// The latched vertical movement axis, mirroring `Input.MoveY.Value`.
    #[must_use]
    pub fn move_y(&self) -> i32 {
        self.move_y.value
    }

    pub fn button(&self, action: i32) -> bool {
        if (0..act::COUNT).contains(&action) {
            self.held[action as usize]
        } else {
            false
        }
    }

    pub fn pressed(&self, action: i32) -> bool {
        if (0..act::COUNT).contains(&action) {
            // `VirtualButton.Pressed`: the fresh edge or a buffered press within
            // the window (bufferCounter > 0), unless consumed.
            self.pressed[action as usize] || self.buffer[action as usize] > 0.0
        } else {
            false
        }
    }

    pub fn released(&self, action: i32) -> bool {
        if (0..act::COUNT).contains(&action) {
            self.released[action as usize]
        } else {
            false
        }
    }

    /// Clears the press buffer for an action (`VirtualButton.ConsumeBuffer`).
    pub fn consume(&mut self, action: i32) {
        if (0..act::COUNT).contains(&action) {
            self.buffer[action as usize] = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl3::mouse::MouseButton;

    fn mouse_down(x: f32, y: f32) -> Event {
        Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x,
            y,
        }
    }

    fn mouse_up(x: f32, y: f32) -> Event {
        Event::MouseButtonUp {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x,
            y,
        }
    }

    fn mouse_motion(x: f32, y: f32) -> Event {
        Event::MouseMotion {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mousestate: sdl3::mouse::MouseState::from_sdl_state(0),
            x,
            y,
            xrel: 0.0,
            yrel: 0.0,
        }
    }

    #[test]
    fn mouse_position_updates_from_motion() {
        let mut input = Input::default();
        input.pump([mouse_motion(50.0, 30.0)], 1.0 / 60.0);
        assert_eq!(input.mouse.x, 50.0);
        assert_eq!(input.mouse.y, 30.0);
    }

    #[test]
    fn mouse_left_pressed_and_released_edges() {
        let mut input = Input::default();
        // Down: pressed edge fires.
        input.pump([mouse_down(10.0, 20.0)], 1.0 / 60.0);
        assert!(input.mouse.left_pressed, "first down should fire pressed");
        assert!(
            !input.mouse.left_released,
            "first down should not fire released"
        );
        assert!(input.mouse.left_down);
        // Held: no new edges.
        input.pump([mouse_down(10.0, 20.0)], 1.0 / 60.0);
        assert!(!input.mouse.left_pressed, "held should not fire pressed");
        assert!(!input.mouse.left_released, "held should not fire released");
        // Release: released edge fires.
        input.pump([mouse_up(10.0, 20.0)], 1.0 / 60.0);
        assert!(!input.mouse.left_pressed, "release should not fire pressed");
        assert!(input.mouse.left_released, "release should fire released");
        assert!(!input.mouse.left_down);
        // Idle: no edges.
        input.pump([], 1.0 / 60.0);
        assert!(!input.mouse.left_pressed);
        assert!(!input.mouse.left_released);
    }

    #[test]
    fn focus_loss_clears_held_keys() {
        let mut input = Input::default();
        // Press and hold Escape (PAUSE action).
        input.pump(
            [Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: Some(Keycode::Escape),
                scancode: None,
                keymod: sdl3::keyboard::Mod::empty(),
                repeat: false,
                which: 0,
                raw: 0,
            }],
            1.0 / 60.0,
        );
        assert!(
            input.button(act::PAUSE),
            "escape should be held after keydown"
        );
        // Window loses focus — all held keys should be cleared.
        input.pump(
            [Event::Window {
                timestamp: 0,
                window_id: 0,
                win_event: WindowEvent::FocusLost,
            }],
            1.0 / 60.0,
        );
        assert!(
            !input.button(act::PAUSE),
            "escape should be cleared after focus lost"
        );
    }

    fn keydown(kc: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(kc),
            scancode: None,
            keymod: sdl3::keyboard::Mod::empty(),
            repeat: false,
            which: 0,
            raw: 0,
        }
    }

    fn keyup(kc: Keycode) -> Event {
        Event::KeyUp {
            timestamp: 0,
            window_id: 0,
            keycode: Some(kc),
            scancode: None,
            keymod: sdl3::keyboard::Mod::empty(),
            repeat: false,
            which: 0,
            raw: 0,
        }
    }

    #[test]
    fn axis_check_two_sides() {
        let mut input = Input::default();
        let (neg, pos) = (act::MOVE_LEFT, act::MOVE_RIGHT);
        // Neither held → 0.
        input.pump([], 1.0 / 60.0);
        assert_eq!(input.axis_check(neg, pos), 0);
        // Negative alone → -1.
        input.pump([keydown(Keycode::Left)], 1.0 / 60.0);
        assert_eq!(input.axis_check(neg, pos), -1);
        // Release, positive alone → 1.
        input.pump([keyup(Keycode::Left), keydown(Keycode::Right)], 1.0 / 60.0);
        assert_eq!(input.axis_check(neg, pos), 1);
        // Both held → 0 (mirrors `AxisCheck`).
        input.pump([keydown(Keycode::Left)], 1.0 / 60.0);
        assert_eq!(input.axis_check(neg, pos), 0);
        // Release positive → negative alone → -1 again.
        input.pump([keyup(Keycode::Right)], 1.0 / 60.0);
        assert_eq!(input.axis_check(neg, pos), -1);
    }

    #[test]
    fn move_axis_take_newer_on_overlap() {
        let mut input = Input::default();
        // Nothing held → 0.
        input.pump([], 1.0 / 60.0);
        assert_eq!(input.move_x(), 0);
        // Right alone → 1.
        input.pump([keydown(Keycode::Right)], 1.0 / 60.0);
        assert_eq!(input.move_x(), 1);
        // Left alone → -1.
        input.pump([keyup(Keycode::Right), keydown(Keycode::Left)], 1.0 / 60.0);
        assert_eq!(input.move_x(), -1);
        // Both held → TakeNewer flips to the most recently held direction
        // (right), matching the original: `Value *= -1` once.
        input.pump([keydown(Keycode::Right)], 1.0 / 60.0);
        assert_eq!(input.move_x(), 1);
        // Both still held → no further flip (turned stays latched).
        input.pump([], 1.0 / 60.0);
        input.pump([keydown(Keycode::Left)], 1.0 / 60.0);
        input.pump([keydown(Keycode::Right)], 1.0 / 60.0);
        assert_eq!(input.move_x(), 1);
    }
}
