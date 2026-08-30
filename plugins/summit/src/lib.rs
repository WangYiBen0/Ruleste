#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{collect, draw_image, draw_rect, entities_by_type, set_respawn};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("summit");
ruleste_plugins_api::ruleste_entity_types!(
    "summitGemManager",
    "summitcheckpoint",
    "summitcloud",
    "summitgem"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const CLOUD: Color = Color {
    r: 0xee,
    g: 0xee,
    b: 0xff,
    a: 0xff,
};
const FLAG: Color = Color {
    r: 0x44,
    g: 0xcc,
    b: 0x66,
    a: 0xff,
};
const MANAGER: Color = Color {
    r: 0x66,
    g: 0x66,
    b: 0x66,
    a: 0xff,
};

#[derive(Copy, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum Kind {
    SummitGemManager,
    SummitCheckpoint,
    SummitCloud,
    SummitGem,
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "summitGemManager" => Kind::SummitGemManager,
        "summitcheckpoint" => Kind::SummitCheckpoint,
        "summitcloud" => Kind::SummitCloud,
        "summitgem" => Kind::SummitGem,
        _ => return None,
    })
}

#[derive(Clone)]
struct State {
    kind: Kind,
    w: f32,
    h: f32,
    nodes: Vec<(f32, f32)>,
    idx: usize,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

fn spawn_type(data: *const u8, len: u32) -> String {
    spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) })
        .get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let Some(kind) = kind_for(&spawn_type(data, len)) else {
        return;
    };
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    if kind == Kind::SummitCloud {
        e.collision.platform(true);
    }
    if kind == Kind::SummitCheckpoint {
        set_respawn(x, y);
    }
    let nodes: Vec<(f32, f32)> = spawn.nodes().iter().map(|n| (n.x, n.y)).collect();
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                kind,
                w,
                h,
                nodes,
                idx: 0,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).cloned()) {
        Some(st) => st,
        None => return,
    };
    if st.kind == Kind::SummitCloud && !st.nodes.is_empty() {
        let e = Entity::new(id);
        let p = e.position.get();
        let dest = st.nodes[st.idx];
        let dx = dest.0 - p.x;
        let dy = dest.1 - p.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let speed = 20.0;
        if dist <= speed * dt {
            e.position.set_xy(dest.0, dest.1);
            st.idx = (st.idx + 1) % st.nodes.len();
        } else {
            e.position
                .set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
        }
    } else if st.kind == Kind::SummitGem {
        let e = Entity::new(id);
        let p = e.position.get();
        for pid in entities_by_type("player") {
            let pp = Entity::new(pid).position.get();
            let (pw, ph, _, _) = Entity::new(pid).hitbox.get();
            if p.x < pp.x + pw && p.x + st.w > pp.x && p.y < pp.y + ph && p.y + st.h > pp.y {
                collect(id);
                return;
            }
        }
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow().get(&id).cloned()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let p = e.position.get();
    match st.kind {
        // The summit gem is the real heart-gem collectable sprite.
        Kind::SummitGem => {
            draw_image(
                "collectables/heartGem/0/00",
                p.x + st.w * 0.5 - 8.0,
                p.y + st.h * 0.5 - 8.0,
                0.0,
                1.0,
                1.0,
            );
        }
        _ => {
            let c = match st.kind {
                Kind::SummitCloud => CLOUD,
                Kind::SummitCheckpoint => FLAG,
                Kind::SummitGemManager => MANAGER,
                _ => CLOUD,
            };
            draw_rect(p.x, p.y, st.w, st.h, c);
        }
    }
}
