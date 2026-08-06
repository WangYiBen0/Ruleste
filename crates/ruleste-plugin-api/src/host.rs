//! Host-provided FFI functions that a Wasm plugin may call. Each function is
//! exported by the Ruleste host runtime and imported by the plugins. The
//! signatures here must stay in sync with `src/hotload/wasm_host.rs`.

use crate::types::{Color, EntityId, Vec2};
use std::string::String;

#[allow(dead_code)]
#[link(wasm_import_module = "env")]
extern "C" {
    // Every function here is part of the plugin ABI surface; individual
    // plugins use whichever subset they need.
    fn host_position_get(id: EntityId, out: *mut Vec2);
    fn host_position_set(id: EntityId, x: f32, y: f32);
    fn host_speed_get(id: EntityId, out: *mut Vec2);
    fn host_speed_set(id: EntityId, x: f32, y: f32);
    fn host_hitbox_set(id: EntityId, w: f32, h: f32, ox: f32, oy: f32);
    fn host_depth_get(id: EntityId) -> i32;
    fn host_depth_set(id: EntityId, depth: i32);
    fn host_visible_get(id: EntityId) -> bool;
    fn host_visible_set(id: EntityId, visible: bool);
    fn host_sprite_play(id: EntityId, name: *const u8, len: u32);
    fn host_sprite_animation(id: EntityId, out: *mut u8, out_cap: u32) -> u32;
    fn host_sprite_frame_get(id: EntityId) -> f32;
    fn host_sprite_frame_set(id: EntityId, frame: f32);
    fn host_sprite_rate_get(id: EntityId) -> f32;
    fn host_sprite_rate_set(id: EntityId, rate: f32);
    fn host_sprite_color_set(id: EntityId, color: Color);
    fn host_sprite_flip_x_get(id: EntityId) -> bool;
    fn host_sprite_flip_x_set(id: EntityId, flip: bool);
    fn host_sprite_flip_y_get(id: EntityId) -> bool;
    fn host_sprite_flip_y_set(id: EntityId, flip: bool);
    fn host_input_axis(action: i32) -> f32;
    fn host_input_button(action: i32) -> bool;
    fn host_input_pressed(action: i32) -> bool;
    fn host_input_released(action: i32) -> bool;
    fn host_collide_check(id: EntityId, offset_x: f32, offset_y: f32) -> bool;
    fn host_actor_move(id: EntityId, h: f32, v: f32) -> u32;
    fn host_actor_is_grounded(id: EntityId) -> bool;
    fn host_play_sound(name: *const u8, len: u32);
    fn host_log(msg: *const u8, len: u32);
    fn host_emit(id: EntityId, event: u32, data: *const u8, len: u32);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ActorMoveResult {
    pub on_ground: bool,
    pub hit_wall_left: bool,
    pub hit_wall_right: bool,
    pub hit_ceiling: bool,
}

impl ActorMoveResult {
    pub const GROUND: u32 = 1;
    pub const WALL_LEFT: u32 = 2;
    pub const WALL_RIGHT: u32 = 4;
    pub const CEILING: u32 = 8;

    pub fn from_flags(flags: u32) -> ActorMoveResult {
        ActorMoveResult {
            on_ground: flags & Self::GROUND != 0,
            hit_wall_left: flags & Self::WALL_LEFT != 0,
            hit_wall_right: flags & Self::WALL_RIGHT != 0,
            hit_ceiling: flags & Self::CEILING != 0,
        }
    }
}

pub fn log(msg: &str) {
    unsafe {
        host_log(msg.as_ptr(), msg.len() as u32);
    }
}

pub fn play_sound(name: &str) {
    unsafe {
        host_play_sound(name.as_ptr(), name.len() as u32);
    }
}

pub fn emit(id: EntityId, event: u32, data: &[u8]) {
    unsafe {
        host_emit(id, event, data.as_ptr(), data.len() as u32);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    id: EntityId,
}

impl Position {
    pub(crate) fn new(id: EntityId) -> Position {
        Position { id }
    }

    #[must_use]
    pub fn get(&self) -> Vec2 {
        let mut out = Vec2::ZERO;
        unsafe {
            host_position_get(self.id, &mut out);
        }
        out
    }

    pub fn set(&self, v: Vec2) {
        unsafe {
            host_position_set(self.id, v.x, v.y);
        }
    }

    pub fn set_xy(&self, x: f32, y: f32) {
        unsafe {
            host_position_set(self.id, x, y);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Speed {
    id: EntityId,
}

impl Speed {
    pub(crate) fn new(id: EntityId) -> Speed {
        Speed { id }
    }

    #[must_use]
    pub fn get(&self) -> Vec2 {
        let mut out = Vec2::ZERO;
        unsafe {
            host_speed_get(self.id, &mut out);
        }
        out
    }

    pub fn set(&self, v: Vec2) {
        unsafe {
            host_speed_set(self.id, v.x, v.y);
        }
    }

    pub fn set_xy(&self, x: f32, y: f32) {
        unsafe {
            host_speed_set(self.id, x, y);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    id: EntityId,
}

impl Sprite {
    pub(crate) fn new(id: EntityId) -> Sprite {
        Sprite { id }
    }

    pub fn play(&self, name: &str) {
        unsafe {
            host_sprite_play(self.id, name.as_ptr(), name.len() as u32);
        }
    }

    #[must_use]
    pub fn animation(&self) -> String {
        let mut buf = [0u8; 128];
        unsafe {
            host_sprite_animation(self.id, buf.as_mut_ptr(), buf.len() as u32);
        }
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..len]).into_owned()
    }

    #[must_use]
    pub fn frame(&self) -> f32 {
        unsafe { host_sprite_frame_get(self.id) }
    }

    pub fn set_frame(&self, frame: f32) {
        unsafe {
            host_sprite_frame_set(self.id, frame);
        }
    }

    #[must_use]
    pub fn rate(&self) -> f32 {
        unsafe { host_sprite_rate_get(self.id) }
    }

    pub fn set_rate(&self, rate: f32) {
        unsafe {
            host_sprite_rate_set(self.id, rate);
        }
    }

    pub fn set_color(&self, color: Color) {
        unsafe {
            host_sprite_color_set(self.id, color);
        }
    }

    pub fn flip_x(&self, flip: bool) {
        unsafe {
            host_sprite_flip_x_set(self.id, flip);
        }
    }

    pub fn flip_y(&self, flip: bool) {
        unsafe {
            host_sprite_flip_y_set(self.id, flip);
        }
    }
}

pub struct Input;

impl Input {
    #[must_use]
    pub fn axis(action: i32) -> f32 {
        unsafe { host_input_axis(action) }
    }

    #[must_use]
    pub fn button(action: i32) -> bool {
        unsafe { host_input_button(action) }
    }

    #[must_use]
    pub fn pressed(action: i32) -> bool {
        unsafe { host_input_pressed(action) }
    }

    #[must_use]
    pub fn released(action: i32) -> bool {
        unsafe { host_input_released(action) }
    }
}

pub struct Collision {
    id: EntityId,
}

impl Collision {
    pub(crate) fn new(id: EntityId) -> Collision {
        Collision { id }
    }

    /// Returns true when the entity's hitbox, offset by `(dx, dy)`, overlaps a
    /// solid tile.
    #[must_use]
    pub fn check(&self, dx: f32, dy: f32) -> bool {
        unsafe { host_collide_check(self.id, dx, dy) }
    }

    /// Moves the entity by `(h, v)` and resolves collisions against solid
    /// tiles, mirroring `Actor.MoveH/MoveV` of the original engine.
    #[must_use]
    pub fn actor_move(&self, h: f32, v: f32) -> ActorMoveResult {
        ActorMoveResult::from_flags(unsafe { host_actor_move(self.id, h, v) })
    }

    #[must_use]
    pub fn is_grounded(&self) -> bool {
        unsafe { host_actor_is_grounded(self.id) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hitbox {
    id: EntityId,
}

impl Hitbox {
    pub(crate) fn new(id: EntityId) -> Hitbox {
        Hitbox { id }
    }

    /// Sets the collision rect: size `(w, h)` anchored at
    /// `position + (ox, oy)`.
    pub fn set(&self, w: f32, h: f32, ox: f32, oy: f32) {
        unsafe {
            host_hitbox_set(self.id, w, h, ox, oy);
        }
    }
}
