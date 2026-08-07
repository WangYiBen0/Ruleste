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
    fn host_draw_line(x1: f32, y1: f32, x2: f32, y2: f32, r: u32, g: u32, b: u32, a: u32);
    fn host_entities_by_type(
        type_name: *const u8,
        type_len: u32,
        out_ids: *mut EntityId,
        max_count: u32,
    ) -> u32;
    fn host_drain_events(out_buf: *mut u8, buf_cap: u32) -> u32;
    fn host_entity_alive(id: EntityId) -> i32;
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

/// Appends a line segment (in world coordinates) to this frame's draw list.
/// Only meaningful during the `ruleste_entity_draw` hook.
pub fn draw_line(x1: f32, y1: f32, x2: f32, y2: f32, color: Color) {
    unsafe {
        host_draw_line(
            x1,
            y1,
            x2,
            y2,
            color.r as u32,
            color.g as u32,
            color.b as u32,
            color.a as u32,
        );
    }
}

/// Returns the IDs of all live entities whose `entity_type` matches `name`.
pub fn entities_by_type(name: &str) -> Vec<EntityId> {
    const MAX: u32 = 128;
    let mut buf = [0u32; MAX as usize];
    let count =
        unsafe { host_entities_by_type(name.as_ptr(), name.len() as u32, buf.as_mut_ptr(), MAX) };
    buf[..count as usize].to_vec()
}

/// Drains the event queue. Each event is `(entity_id, kind, data)`.
pub fn drain_events() -> Vec<(EntityId, u32, Vec<u8>)> {
    const BUF_CAP: u32 = 16384;
    let mut buf = vec![0u8; BUF_CAP as usize];
    let len = unsafe { host_drain_events(buf.as_mut_ptr(), BUF_CAP) };
    buf.truncate(len as usize);
    let mut events = Vec::new();
    let mut i = 0;
    while i + 12 <= buf.len() {
        let entity = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap());
        let kind = u32::from_le_bytes(buf[i + 4..i + 8].try_into().unwrap());
        let data_len = u32::from_le_bytes(buf[i + 8..i + 12].try_into().unwrap()) as usize;
        i += 12;
        let data = if i + data_len <= buf.len() {
            buf[i..i + data_len].to_vec()
        } else {
            break;
        };
        i += data_len;
        events.push((entity, kind, data));
    }
    events
}

/// Returns `true` if the entity with the given ID is alive in the world.
pub fn entity_alive(id: EntityId) -> bool {
    unsafe { host_entity_alive(id) != 0 }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    id: EntityId,
}
impl Position {
    pub fn new(id: EntityId) -> Position {
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
    pub fn new(id: EntityId) -> Speed {
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

#[derive(Clone, Copy, Debug)]
pub struct Depth {
    id: EntityId,
}

impl Depth {
    pub fn new(id: EntityId) -> Depth {
        Depth { id }
    }

    #[must_use]
    pub fn get(&self) -> i32 {
        unsafe { host_depth_get(self.id) }
    }

    pub fn set(&self, depth: i32) {
        unsafe {
            host_depth_set(self.id, depth);
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
    pub fn new(id: EntityId) -> Collision {
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
    pub fn new(id: EntityId) -> Hitbox {
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
