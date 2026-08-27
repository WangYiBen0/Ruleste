#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("reflection");
ruleste_plugins_api::ruleste_entity_types!(
    "bigWaterfall",
    "finalBoss",
    "finalBossFallingBlock",
    "finalBossMovingBlock",
    "reflectionHeartStatue",
    "tentacles"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const WATER: Color = Color { r: 0x44, g: 0x99, b: 0xcc, a: 0x88 };
const BOSS: Color = Color { r: 0x44, g: 0x22, b: 0x44, a: 0xff };
const BLOCK: Color = Color { r: 0x88, g: 0x88, b: 0x88, a: 0xff };
const STATUE: Color = Color { r: 0x77, g: 0x77, b: 0x66, a: 0xff };
const TENT: Color = Color { r: 0x66, g: 0x33, b: 0x66, a: 0xff };

#[derive(Copy, Clone, PartialEq, Eq)]
enum Kind {
    BigWaterfall,
    FinalBoss,
    FinalBossFallingBlock,
    FinalBossMovingBlock,
    ReflectionHeartStatue,
    Tentacles,
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "bigWaterfall" => Kind::BigWaterfall,
        "finalBoss" => Kind::FinalBoss,
        "finalBossFallingBlock" => Kind::FinalBossFallingBlock,
        "finalBossMovingBlock" => Kind::FinalBossMovingBlock,
        "reflectionHeartStatue" => Kind::ReflectionHeartStatue,
        "tentacles" => Kind::Tentacles,
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
    spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) }).get_str("_entity_type", "")
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
    let solid = matches!(
        kind,
        Kind::FinalBossFallingBlock | Kind::FinalBossMovingBlock | Kind::ReflectionHeartStatue
    );
    e.collision.solid(solid);
    let nodes: Vec<(f32, f32)> = spawn.nodes().iter().map(|n| (n.x, n.y)).collect();
    STATES.with(|s| {
        s.borrow_mut().insert(id, State { kind, w, h, nodes, idx: 0 });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).cloned()) {
        Some(st) => st,
        None => return,
    };
    if st.kind == Kind::FinalBossMovingBlock && !st.nodes.is_empty() {
        let e = Entity::new(id);
        let p = e.position.get();
        let dest = st.nodes[st.idx];
        let dx = dest.0 - p.x;
        let dy = dest.1 - p.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let speed = 30.0;
        if dist <= speed * dt {
            e.position.set_xy(dest.0, dest.1);
            st.idx = (st.idx + 1) % st.nodes.len();
        } else {
            e.position.set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
        }
        STATES.with(|s| {
            if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
                *st_ref = st;
            }
        });
    }
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
        Kind::BigWaterfall => WATER,
        Kind::FinalBoss => BOSS,
        Kind::FinalBossFallingBlock | Kind::FinalBossMovingBlock => BLOCK,
        Kind::ReflectionHeartStatue => STATUE,
        Kind::Tentacles => TENT,
    };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
