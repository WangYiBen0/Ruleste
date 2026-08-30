#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-touch-switch` — `touchSwitch` (universal).
//!
//! A 16×16 pad that fires the `SWITCH` event once the player overlaps it. The
//! `switch-gate` crate listens for that event. Mirrors `TouchSwitch` in
//! `references/source/Celeste/Celeste/TouchSwitch.cs`. It only emits (never
//! drains) the event bus, so it lives in its own crate.

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{self, draw_image, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("touch-switch");
ruleste_plugins_api::ruleste_entity_types!("touchSwitch");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SWITCH_ON: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0xff,
    a: 0xff,
};

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

#[allow(clippy::too_many_arguments)]
fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

/// True if any entity of one of `types` overlaps the given rect. Used so the
/// switch also fires for `Holdable`s (Theo / Key) and `Seeker`s, matching
/// `TouchSwitch.OnHoldable` / `OnSeeker` in the original.
fn overlap_any(types: &[&str], bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    for t in types {
        for eid in entities_by_type(t) {
            let ep = Position::new(eid).get();
            let (ew, eh, eox, eoy) = Hitbox::new(eid).get();
            if overlap(ep.x + eox, ep.y + eoy, ew, eh, bx, by, bw, bh) {
                return true;
            }
        }
    }
    false
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
    let switch = (p.x - 8.0, p.y - 8.0, 16.0, 16.0);
    let player_hits = player_rect().is_some_and(|(px, py, pw, ph)| {
        overlap(px, py, pw, ph, switch.0, switch.1, switch.2, switch.3)
    });
    // `TouchSwitch.OnHoldable` / `OnSeeker`: holdables and seekers also depress
    // the pad, not just the player.
    let holdable_hits = overlap_any(
        &["theo-crystal", "key"],
        switch.0,
        switch.1,
        switch.2,
        switch.3,
    );
    let seeker_hits = overlap_any(&["seeker"], switch.0, switch.1, switch.2, switch.3);
    if player_hits || holdable_hits || seeker_hits {
        TRIGGERED.with(|s| s.borrow_mut().insert(id, true));
        host::emit(id, event::SWITCH, &[]);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let triggered = TRIGGERED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false));
    let p = Entity::new(id).position.get();
    // Real touch-switch pad (14x14) centered on the 16x16 hitbox.
    draw_image(
        "objects/touchswitch/container",
        p.x - 8.0,
        p.y - 8.0,
        0.0,
        1.0,
        1.0,
    );
    if triggered {
        draw_rect(p.x - 2.0, p.y - 2.0, 4.0, 4.0, SWITCH_ON);
    }
}
