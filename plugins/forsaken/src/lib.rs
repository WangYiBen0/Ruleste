#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-forsaken` — Forsaken City (chapter 1) specific entities that
//! are not shared with other chapters.
//!
//! * `birdForsakenCityGem` -> the cutscene bird carrying the gem (decorative, no
//!   gameplay).
//! * `memorialTextController` -> invisible cutscene trigger (no gameplay).

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("forsaken");
ruleste_plugins_api::ruleste_entity_types!("birdForsakenCityGem", "memorialTextController");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const BIRD: Color = Color {
    r: 0xe0,
    g: 0xc0,
    b: 0x60,
    a: 0xff,
};
const GEM: Color = Color {
    r: 0x18,
    g: 0x18,
    b: 0x20,
    a: 0xff,
};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bird,
    Memorial,
}

thread_local! {
    static KINDS: RefCell<ruleste_plugins_api::plugin::EntityState<Kind>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn kind_for(t: &str) -> Option<Kind> {
    match t {
        "birdForsakenCityGem" => Some(Kind::Bird),
        "memorialTextController" => Some(Kind::Memorial),
        _ => None,
    }
}

fn spawn_type(data: *const u8, len: u32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    spawn_data(bytes).get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let Some(kind) = kind_for(&spawn_type(data, len)) else {
        return;
    };
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
    KINDS.with(|s| s.borrow_mut().insert(id, kind));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let kind = match KINDS.with(|s| s.borrow_mut().get(id).copied()) {
        Some(k) => k,
        None => return,
    };
    if kind != Kind::Bird {
        return;
    }
    let p = Entity::new(id).position.get();
    draw_rect(p.x - 8.0, p.y - 4.0, 16.0, 8.0, BIRD);
    draw_rect(p.x - 2.0, p.y - 12.0, 4.0, 6.0, GEM);
}
