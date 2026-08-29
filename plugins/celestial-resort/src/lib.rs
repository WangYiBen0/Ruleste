#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-celestial-resort` — Celestial Resort (chapter 3) specific
//! entities that are not shared with other chapters.
//!
//! Behaviors follow `references/source/Celeste/Celeste/*`:
//! * `trapdoor`     -> a solid floor that opens (non-solid) while the player is
//!   on it, then re-solidifies behind them (`Trapdoor.cs`).
//! * `blockField`   -> a grid of decorative blocks (`BlockField.cs`, visual here).
//! * `clothesline`  -> hanging laundry prop (`Clothesline.cs`).
//! * `clutterCabinet` / `clutterDoor` -> breakable clutter props (`Clutter*`).
//! * `friendlyGhost` -> the ghost NPC (`FriendlyGhost.cs`, visual here).
//! * `oshirodoor`   -> Oshiro's door prop (`OshiroDoor.cs`).
//! * `picoconsole`  -> console prop used in a cutscene (`PicoConsole.cs`).
//! * `resortRoofEnding` / `resortmirror` -> resort set-dressing.

use ruleste_plugins_api::host::{draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("celestial-resort");
ruleste_plugins_api::ruleste_entity_types!(
    "blockField",
    "clothesline",
    "clutterCabinet",
    "clutterDoor",
    "friendlyGhost",
    "oshirodoor",
    "picoconsole",
    "resortRoofEnding",
    "resortmirror",
    "trapdoor",
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    BlockField,
    Clothesline,
    ClutterCabinet,
    ClutterDoor,
    FriendlyGhost,
    OshiroDoor,
    PicoConsole,
    ResortRoofEnding,
    ResortMirror,
    Trapdoor,
}

const TRAP: Color = Color {
    r: 0x6a,
    g: 0x5a,
    b: 0x4a,
    a: 0xff,
};
const TRAP_OPEN: Color = Color {
    r: 0x6a,
    g: 0x5a,
    b: 0x4a,
    a: 0x20,
};
const WOOD: Color = Color {
    r: 0x8a,
    g: 0x6a,
    b: 0x4a,
    a: 0xff,
};
const METAL: Color = Color {
    r: 0x70,
    g: 0x70,
    b: 0x78,
    a: 0xff,
};
const GHOST: Color = Color {
    r: 0xee,
    g: 0xee,
    b: 0xfa,
    a: 0xcc,
};
const PROP: Color = Color {
    r: 0x88,
    g: 0x88,
    b: 0x90,
    a: 0xff,
};

#[derive(Clone, Copy)]
struct State {
    kind: Kind,
    w: f32,
    h: f32,
    open: bool,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "blockField" => Kind::BlockField,
        "clothesline" => Kind::Clothesline,
        "clutterCabinet" => Kind::ClutterCabinet,
        "clutterDoor" => Kind::ClutterDoor,
        "friendlyGhost" => Kind::FriendlyGhost,
        "oshirodoor" => Kind::OshiroDoor,
        "picoconsole" => Kind::PicoConsole,
        "resortRoofEnding" => Kind::ResortRoofEnding,
        "resortmirror" => Kind::ResortMirror,
        "trapdoor" => Kind::Trapdoor,
        _ => return None,
    })
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
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    if kind == Kind::Trapdoor {
        e.collision.solid(true);
    }
    e.depth.set(200);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                kind,
                w,
                h,
                open: false,
            },
        )
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    if st.kind == Kind::Trapdoor {
        let p = Entity::new(id).position.get();
        let touching = player_rect()
            .is_some_and(|(px, py, pw, ph)| overlap(px, py, pw, ph, p.x, p.y, st.w, st.h));
        if touching && !st.open {
            st.open = true;
            Entity::new(id).collision.solid(false);
        } else if !touching && st.open {
            st.open = false;
            Entity::new(id).collision.solid(true);
        }
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
    match st.kind {
        Kind::Trapdoor => {
            draw_rect(p.x, p.y, st.w, st.h, if st.open { TRAP_OPEN } else { TRAP });
        }
        Kind::BlockField => {
            let mut y = p.y;
            while y < p.y + st.h - 1.0 {
                let mut x = p.x;
                while x < p.x + st.w - 1.0 {
                    draw_rect(x, y, 7.0, 7.0, PROP);
                    x += 8.0;
                }
                y += 8.0;
            }
        }
        Kind::Clothesline => {
            draw_rect(p.x, p.y, st.w, 1.0, METAL);
            let mut x = p.x + 4.0;
            while x < p.x + st.w - 2.0 {
                draw_rect(x, p.y + 1.0, 4.0, 8.0, GHOST);
                x += 12.0;
            }
        }
        Kind::ClutterCabinet => draw_rect(p.x, p.y, st.w, st.h, WOOD),
        Kind::ClutterDoor => draw_rect(p.x, p.y, st.w, st.h, PROP),
        Kind::FriendlyGhost => {
            draw_rect(p.x - 6.0, p.y - 8.0, 12.0, 14.0, GHOST);
            draw_rect(p.x - 3.0, p.y - 4.0, 2.0, 2.0, METAL);
            draw_rect(p.x + 1.0, p.y - 4.0, 2.0, 2.0, METAL);
        }
        Kind::OshiroDoor => draw_rect(p.x, p.y, st.w, st.h, PROP),
        Kind::PicoConsole => {
            draw_rect(p.x, p.y, st.w, st.h, METAL);
            draw_rect(p.x + 2.0, p.y + 2.0, st.w - 4.0, st.h * 0.5, GHOST);
        }
        Kind::ResortRoofEnding => draw_rect(p.x, p.y, st.w, st.h, WOOD),
        Kind::ResortMirror => {
            draw_rect(p.x, p.y, st.w, st.h, GHOST);
            draw_rect(p.x, p.y, st.w, 2.0, METAL);
        }
    }
}
