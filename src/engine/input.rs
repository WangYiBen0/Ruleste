//! Virtual input layer. Maps physical keys (via the `VirtualButton`/binding
//! model of the original `Input`) onto the abstract action ids exposed to
//! plugins. Default bindings mirror `Settings.SetDefaultKeyboardControls`.

use std::collections::HashSet;

use ruleste_plugins_api::types::input as act;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;

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
        }
    }
}

impl Input {
    /// Processes SDL key events, refreshes `held`, and computes the
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
