#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{die, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("lostlevels");
ruleste_plugins_api::ruleste_entity_types!(
    "birdPath",
    "crumbleWallOnRumble",
    "cutsceneNode",
    "eyebomb",
    "flingBird",
    "flingBirdIntro",
    "floatySpaceBlock",
    "glider",
    "kevins_pc",
    "lightning",
    "lightningBlock",
    "moonCreature",
    "playbackBillboard",
    "playbackTutorial",
    "powerSourceNumber",
    "wavedashmachine"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SOLID: Color = Color {
    r: 0x88,
    g: 0x88,
    b: 0x88,
    a: 0xff,
};
const HAZARD: Color = Color {
    r: 0xff,
    g: 0x44,
    b: 0x44,
    a: 0xff,
};
const PROP: Color = Color {
    r: 0x99,
    g: 0x88,
    b: 0x77,
    a: 0xff,
};
const PLAT: Color = Color {
    r: 0xaa,
    g: 0xaa,
    b: 0xcc,
    a: 0xff,
};
const LIGHT: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0x88,
    a: 0xff,
};

#[derive(Copy, Clone, PartialEq, Eq)]
enum Kind {
    BirdPath,
    CrumbleWallOnRumble,
    CutsceneNode,
    Eyebomb,
    FlingBird,
    FlingBirdIntro,
    FloatySpaceBlock,
    Glider,
    KevinsPc,
    Lightning,
    LightningBlock,
    MoonCreature,
    PlaybackBillboard,
    PlaybackTutorial,
    PowerSourceNumber,
    WavedashMachine,
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "birdPath" => Kind::BirdPath,
        "crumbleWallOnRumble" => Kind::CrumbleWallOnRumble,
        "cutsceneNode" => Kind::CutsceneNode,
        "eyebomb" => Kind::Eyebomb,
        "flingBird" => Kind::FlingBird,
        "flingBirdIntro" => Kind::FlingBirdIntro,
        "floatySpaceBlock" => Kind::FloatySpaceBlock,
        "glider" => Kind::Glider,
        "kevins_pc" => Kind::KevinsPc,
        "lightning" => Kind::Lightning,
        "lightningBlock" => Kind::LightningBlock,
        "moonCreature" => Kind::MoonCreature,
        "playbackBillboard" => Kind::PlaybackBillboard,
        "playbackTutorial" => Kind::PlaybackTutorial,
        "powerSourceNumber" => Kind::PowerSourceNumber,
        "wavedashmachine" => Kind::WavedashMachine,
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
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    let solid = matches!(kind, Kind::CrumbleWallOnRumble | Kind::LightningBlock);
    let platform = matches!(kind, Kind::FloatySpaceBlock);
    e.collision.solid(solid);
    e.collision.platform(platform);
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
    let hazard = matches!(st.kind, Kind::Eyebomb | Kind::Lightning);
    if st.kind == Kind::FloatySpaceBlock && !st.nodes.is_empty() {
        let e = Entity::new(id);
        let p = e.position.get();
        let dest = st.nodes[st.idx];
        let dx = dest.0 - p.x;
        let dy = dest.1 - p.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let speed = 25.0;
        if dist <= speed * dt {
            e.position.set_xy(dest.0, dest.1);
            st.idx = (st.idx + 1) % st.nodes.len();
        } else {
            e.position
                .set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
        }
    }
    if hazard {
        let e = Entity::new(id);
        let p = e.position.get();
        for pid in entities_by_type("player") {
            let pp = Entity::new(pid).position.get();
            let (pw, ph, _, _) = Entity::new(pid).hitbox.get();
            if p.x < pp.x + pw && p.x + st.w > pp.x && p.y < pp.y + ph && p.y + st.h > pp.y {
                die();
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
    let c = match st.kind {
        Kind::Eyebomb => HAZARD,
        Kind::Lightning => LIGHT,
        Kind::LightningBlock => SOLID,
        Kind::FloatySpaceBlock => PLAT,
        Kind::CrumbleWallOnRumble => SOLID,
        _ => PROP,
    };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
