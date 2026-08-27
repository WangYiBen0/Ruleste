use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    #[must_use]
    pub fn length(&self) -> f32 {
        self.x.hypot(self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
}

pub mod input {
    pub const MOVE_LEFT: i32 = 0;
    pub const MOVE_RIGHT: i32 = 1;
    pub const MOVE_UP: i32 = 2;
    pub const MOVE_DOWN: i32 = 3;
    pub const JUMP: i32 = 4;
    pub const DASH: i32 = 5;
    pub const CLIMB: i32 = 6;
    pub const START: i32 = 7;
    pub const BACK: i32 = 8;
    pub const CONFIRM: i32 = 9;
    pub const CANCEL: i32 = 10;
    pub const QUICK_RESTART: i32 = 11;
    pub const PAUSE: i32 = 12;

    /// Total number of input actions understood by the host.
    pub const COUNT: i32 = 13;
}

/// A handle to a live game entity, shared between host and plugins.
pub type EntityId = u32;
