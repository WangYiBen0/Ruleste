#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-cassette-block` — `cassetteBlock` from the Forsaken City maps.
//!
//! A solid block that flips between passable and solid each time a `cassette`
//! collectible emits the `CASSETTE` event (see
//! `references/source/Celeste/Celeste/CassetteBlock.cs`). It listens on the event
//! bus, so it lives in its own plugin crate (the host keeps one event cursor per
//! crate).

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{
    Collision, EV_CASSETTE_RIDE, Hitbox, Position, drain_events, draw_rect, emit, entities_by_type,
};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("cassette-block");
ruleste_plugins_api::ruleste_entity_types!("cassetteBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const GATE: Color = Color {
    r: 0x5f,
    g: 0xcd,
    b: 0xe4,
    a: 0xcc,
};
const GATE_OPEN: Color = Color {
    r: 0x5f,
    g: 0xcd,
    b: 0xe4,
    a: 0x30,
};

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
    solid: bool,
    riding: bool,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn spawn_type(data: *const u8, len: u32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    spawn_data(bytes).get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    if spawn_type(data, len) != "cassetteBlock" {
        return;
    }
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(200);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                h,
                solid: true,
                riding: false,
            },
        )
    });
}

/// `Solid.HasPlayerRider()`: a player rests on (or overlaps) the block's top.
fn player_on_top(id: EntityId) -> bool {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, ox, oy) = e.hitbox.get();
    let top = p.y + oy;
    let left = p.x + ox;
    let right = left + w;
    for pid in entities_by_type("player") {
        let pp = Position::new(pid).get();
        let (pw, ph, pox, poy) = Hitbox::new(pid).get();
        let pleft = pp.x + pox;
        let pright = pleft + pw;
        let pbottom = pp.y + poy + ph;
        if pright > left + 1.0 && pleft < right - 1.0 && pbottom >= top - 1.0 && pbottom <= top + h
        {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    for (_, kind, _) in drain_events() {
        if kind == event::CASSETTE {
            st.solid = !st.solid;
            Entity::new(id).collision.solid(st.solid);
            // `BlockedCheck`/`TryActorWiggleUp`: when the block becomes solid
            // while a player overlaps its body, push the player up out of it
            // (instead of crushing) if there is headroom above.
            if st.solid {
                let p = Entity::new(id).position.get();
                for pid in entities_by_type("player") {
                    let pp = Position::new(pid).get();
                    let (pw, ph, pox, poy) = Hitbox::new(pid).get();
                    let pl = pp.x + pox;
                    let pr = pl + pw;
                    let pt = pp.y + poy;
                    let pb = pt + ph;
                    if pr > p.x && pl < p.x + st.w && pb > p.y && pt < p.y + st.h {
                        let push = pb - p.y;
                        if !Collision::new(pid).check(0.0, -(push + 1.0)) {
                            let _ = Entity::new(pid).collision.actor_move(0.0, -push);
                        }
                    }
                }
            }
            break;
        }
    }
    // While solid and the player rides the top, flag the player into
    // `StCassetteFly` (and clear it once they step off).
    let riding = st.solid && player_on_top(id);
    if riding != st.riding {
        let mut buf = [0u8; 1];
        buf[0] = u8::from(riding);
        emit(id, EV_CASSETTE_RIDE, &buf);
        st.riding = riding;
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow_mut().get(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    let c = if st.solid { GATE } else { GATE_OPEN };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
