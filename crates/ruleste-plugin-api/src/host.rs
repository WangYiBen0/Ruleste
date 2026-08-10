//! Host-provided FFI functions that a Wasm plugin may call. Each function is
//! exported by the Ruleste host runtime and imported by the plugins. The
//! signatures here must stay in sync with `src/hotload/wasm_host.rs`.

use crate::types::{Color, EntityId, Vec2};
use std::string::String;

/// Event kinds passed over the host event bus (`host_emit` /
/// `host_drain_events`). Plugins agree on these numbers: a refill restores the
/// player's dashes, a booster launches it, a crushed block reports a dash hit.
/// All ids live in [`crate::event`]; these aliases keep old call sites working.
pub const EV_REFILL: u32 = crate::event::REFILL;
pub const EV_BOOST: u32 = crate::event::BOOST;
pub const EV_CRUSH: u32 = crate::event::CRUSH;

#[allow(dead_code)]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    // Every function here is part of the plugin ABI surface; individual
    // plugins use whichever subset they need.
    fn host_position_get(id: EntityId, out: *mut Vec2);
    fn host_position_set(id: EntityId, x: f32, y: f32);
    fn host_speed_get(id: EntityId, out: *mut Vec2);
    fn host_speed_set(id: EntityId, x: f32, y: f32);
    fn host_hitbox_set(id: EntityId, w: f32, h: f32, ox: f32, oy: f32);
    fn host_hitbox_get(id: EntityId, out: *mut f32);
    fn host_depth_get(id: EntityId) -> i32;
    fn host_depth_set(id: EntityId, depth: i32);
    fn host_visible_get(id: EntityId) -> bool;
    fn host_visible_set(id: EntityId, visible: bool);
    fn host_sprite_play(id: EntityId, name: *const u8, len: u32);
    fn host_sprite_bank_set(id: EntityId, name: *const u8, len: u32);
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
    fn host_input_consume(action: i32);
    fn host_collide_check(id: EntityId, offset_x: f32, offset_y: f32) -> bool;
    fn host_collide_solid_platform_set(id: EntityId, on: bool);
    fn host_actor_move(id: EntityId, h: f32, v: f32) -> u32;
    fn host_actor_is_grounded(id: EntityId) -> bool;
    fn host_play_sound(name: *const u8, len: u32);
    fn host_log(msg: *const u8, len: u32);
    fn host_emit(id: EntityId, event: u32, data: *const u8, len: u32);
    fn host_draw_line(x1: f32, y1: f32, x2: f32, y2: f32, r: u32, g: u32, b: u32, a: u32);
    fn host_draw_rect(x: f32, y: f32, w: f32, h: f32, r: u32, g: u32, b: u32, a: u32);
    fn host_draw_image(
        frame_ptr: *const u8,
        frame_len: u32,
        x: f32,
        y: f32,
        rotation: f32,
        scale_x: f32,
        scale_y: f32,
        flip_x: i32,
        flip_y: i32,
        r: u32,
        g: u32,
        b: u32,
        a: u32,
    );
    fn host_draw_tile_box(tile_id: u32, x: f32, y: f32, tiles_x: u32, tiles_y: u32);
    fn host_die();
    fn host_collect(id: EntityId);
    fn host_respawn_set(x: f32, y: f32);
    fn host_respawn_get(out: *mut Vec2);
    fn host_entities_by_type(
        type_name: *const u8,
        type_len: u32,
        out_ids: *mut EntityId,
        max_count: u32,
    ) -> u32;
    fn host_drain_events(out_buf: *mut u8, buf_cap: u32) -> u32;
    fn host_entity_alive(id: EntityId) -> i32;
    fn host_debug_enabled() -> i32;
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

/// Appends a filled rectangle (in world coordinates) to this frame's draw
/// list. Only meaningful during the `ruleste_entity_draw` hook.
pub fn draw_rect(x: f32, y: f32, w: f32, h: f32, color: Color) {
    unsafe {
        host_draw_rect(
            x,
            y,
            w,
            h,
            color.r as u32,
            color.g as u32,
            color.b as u32,
            color.a as u32,
        );
    }
}

/// Blits an atlas frame (e.g. `danger/spikes/default_up00`) centered at the
/// given world position, with optional rotation (degrees, clockwise) and
/// scale. Only meaningful during the `ruleste_entity_draw` hook.
pub fn draw_image(frame_id: &str, x: f32, y: f32, rotation: f32, scale_x: f32, scale_y: f32) {
    draw_image_flipped(frame_id, x, y, rotation, scale_x, scale_y, false, false);
}

/// Like [`draw_image`], with horizontal/vertical flipping.
#[allow(clippy::too_many_arguments)]
pub fn draw_image_flipped(
    frame_id: &str,
    x: f32,
    y: f32,
    rotation: f32,
    scale_x: f32,
    scale_y: f32,
    flip_x: bool,
    flip_y: bool,
) {
    unsafe {
        host_draw_image(
            frame_id.as_ptr(),
            frame_id.len() as u32,
            x,
            y,
            rotation,
            scale_x,
            scale_y,
            i32::from(flip_x),
            i32::from(flip_y),
            255,
            255,
            255,
            255,
        );
    }
}

/// Like [`draw_image`], tinted with an ARGB color (the alpha is used for
/// fading, e.g. a checkpoint highlight breathing with its sine timer).
pub fn draw_image_color(frame_id: &str, x: f32, y: f32, color: Color) {
    unsafe {
        host_draw_image(
            frame_id.as_ptr(),
            frame_id.len() as u32,
            x,
            y,
            0.0,
            1.0,
            1.0,
            false as i32,
            false as i32,
            color.r as u32,
            color.g as u32,
            color.b as u32,
            color.a as u32,
        );
    }
}

/// Draws a solid box of autotiled tiles at the given world position, mirroring
/// `GFX.FGAutotiler.GenerateBox`. `tile_id` is the ForegroundTiles.xml id
/// (e.g. `'3'` for snow); the host runs the adjacency pass once and caches the
/// grid. Only meaningful during the `ruleste_entity_draw` hook.
pub fn draw_tile_box(tile_id: char, x: f32, y: f32, tiles_x: u32, tiles_y: u32) {
    unsafe {
        host_draw_tile_box(tile_id as u32, x, y, tiles_x, tiles_y);
    }
}

/// Kills the player: the host freezes the room and respawns it shortly after.
/// Equivalent to `Player.Die()` in the original engine.
pub fn die() {
    unsafe {
        host_die();
    }
}

/// Permanently consumes the entity this session: it despawns immediately and
/// is not re-created when the room respawns. Used by collectibles.
pub fn collect(id: EntityId) {
    unsafe {
        host_collect(id);
    }
}

/// Records the world position the player respawns at after a death. Used by
/// checkpoints: once reached, deaths send the player back here instead of the
/// level start.
pub fn set_respawn(x: f32, y: f32) {
    unsafe {
        host_respawn_set(x, y);
    }
}

/// Returns the recorded respawn position, or `(0, 0)` if none was set yet.
pub fn respawn_position() -> Vec2 {
    let mut out = Vec2::ZERO;
    unsafe {
        host_respawn_get(&mut out);
    }
    out
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

/// Returns `true` when the host enables verbose debug logging from plugins
/// (currently gated on the `RULESTE_DEBUG` environment variable).
#[must_use]
pub fn debug_enabled() -> bool {
    unsafe { host_debug_enabled() != 0 }
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

    /// Selects which SpriteBank sprite this entity's animations come from.
    /// The host defaults this to the entity type name; use this to point at a
    /// differently-named SpriteBank entry (e.g. `goldenBerry` -> `goldberry`).
    pub fn set_bank(&self, name: &str) {
        unsafe {
            host_sprite_bank_set(self.id, name.as_ptr(), name.len() as u32);
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
    pub fn released(&self, action: i32) -> bool {
        unsafe { host_input_released(action) }
    }

    /// Zeroes the press buffer (`VirtualButton.ConsumeBuffer`): clears a
    /// buffered press so a jump/dash that already fired does not re-trigger
    /// during the remaining buffer window.
    pub fn consume(action: i32) {
        unsafe {
            host_input_consume(action);
        }
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

    /// Marks the entity as a standable dynamic platform: actors can land and
    /// stand on its top surface, and its movement carries them along.
    pub fn platform(&self, on: bool) {
        unsafe {
            host_collide_solid_platform_set(self.id, on);
        }
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

    /// Reads the collision rect: `(w, h, ox, oy)`.
    #[must_use]
    pub fn get(&self) -> (f32, f32, f32, f32) {
        let mut buf = [0f32; 4];
        unsafe {
            host_hitbox_get(self.id, buf.as_mut_ptr());
        }
        (buf[0], buf[1], buf[2], buf[3])
    }
}
