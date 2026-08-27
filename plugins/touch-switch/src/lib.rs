#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-touch-switch` — `touchSwitch` (universal).
//!
//! A 16×16 pad that fires the `SWITCH` event once the player overlaps it. The
//! `switch-gate` crate listens for that event. Mirrors `TouchSwitch` in
//! `references/source/Celeste/Celeste/TouchSwitch.cs`. It only emits (never
//! drains) the event bus, so it lives in its own crate.

use ruleste_plugins_api::host::{self, draw_rect, entities_by_type};
use ruleste_plugins_api::event;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity, Hitbox, Position};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("touch-switch");
ruleste_plugins_api::ruleste_entity_types!("touchSwitch");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SWITCH_IDLE: Color = Color { r: 0x5f, g: 0xcd, b: 0xe4, a: 0xff };
const SWITCH_ON: Color = Color { r: 0xff, g: 0xff, b: 0xff, a: 0xff };

thread_local! {
    static TRIGGERED: RefCell<ruleste_plugins_api::plugin::EntityState<bool>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    let players = entities_by_type("player");
    let p = *players.first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some((pp.x + pox, pp.y + poy, pw, ph))
}

fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(2000);
    TRIGGERED.with(|s| s.borrow_mut().insert(id, false));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let triggered = TRIGGERED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false));
    if triggered {
        return;
    }
    let p = Entity::new(id).position.get();
    if let Some((px, py, pw, ph)) = player_rect() {
        if overlap(px, py, pw, ph, p.x - 8.0, p.y - 8.0, 16.0, 16.0) {
            TRIGGERED.with(|s| s.borrow_mut().insert(id, true));
            host::emit(id, event::SWITCH, &[]);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let triggered = TRIGGERED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false));
    let p = Entity::new(id).position.get();
    let c = if triggered { SWITCH_ON } else { SWITCH_IDLE };
    draw_rect(p.x - 8.0, p.y - 8.0, 16.0, 16.0, c);
    draw_rect(p.x - 2.0, p.y - 2.0, 4.0, 4.0, SWITCH_ON);
}
