#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{draw_rect, drain_events, entities_by_type, die, emit, remove};
use ruleste_plugins_api::event;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("mirror-temple");
ruleste_plugins_api::ruleste_entity_types!(
    "conditionBlock",
    "dashSwitchH",
    "dashSwitchV",
    "playerSeeker",
    "seekerBarrier",
    "seekerStatue",
    "templeBigEyeball",
    "templeCrackedBlock",
    "templeEye",
    "templeGate",
    "templeMirror",
    "templeMirrorPortal",
    "theoCrystalHoldingBarrier",
    "theoCrystalPedestal"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SOLID: Color = Color { r: 0x88, g: 0x88, b: 0x88, a: 0xff };
const BARRIER: Color = Color { r: 0x66, g: 0x33, b: 0x99, a: 0xff };
const GATE: Color = Color { r: 0xaa, g: 0x44, b: 0x44, a: 0xff };
const GATE_OPEN: Color = Color { r: 0x55, g: 0x55, b: 0x55, a: 0x55 };
const SWITCH: Color = Color { r: 0x44, g: 0xaa, b: 0xff, a: 0xff };
const HAZARD: Color = Color { r: 0x33, g: 0x22, b: 0x44, a: 0xff };
const EYE: Color = Color { r: 0xff, g: 0x55, b: 0x55, a: 0xff };
const MIRROR: Color = Color { r: 0xcc, g: 0xee, b: 0xff, a: 0x88 };
const STATUE: Color = Color { r: 0x77, g: 0x77, b: 0x66, a: 0xff };

#[derive(Copy, Clone, PartialEq, Eq)]
enum Kind {
    ConditionBlock,
    DashSwitchH,
    DashSwitchV,
    PlayerSeeker,
    SeekerBarrier,
    SeekerStatue,
    TempleBigEyeball,
    TempleCrackedBlock,
    TempleEye,
    TempleGate,
    TempleMirror,
    TempleMirrorPortal,
    TheoCrystalHoldingBarrier,
    TheoCrystalPedestal,
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "conditionBlock" => Kind::ConditionBlock,
        "dashSwitchH" => Kind::DashSwitchH,
        "dashSwitchV" => Kind::DashSwitchV,
        "playerSeeker" => Kind::PlayerSeeker,
        "seekerBarrier" => Kind::SeekerBarrier,
        "seekerStatue" => Kind::SeekerStatue,
        "templeBigEyeball" => Kind::TempleBigEyeball,
        "templeCrackedBlock" => Kind::TempleCrackedBlock,
        "templeEye" => Kind::TempleEye,
        "templeGate" => Kind::TempleGate,
        "templeMirror" => Kind::TempleMirror,
        "templeMirrorPortal" => Kind::TempleMirrorPortal,
        "theoCrystalHoldingBarrier" => Kind::TheoCrystalHoldingBarrier,
        "theoCrystalPedestal" => Kind::TheoCrystalPedestal,
        _ => return None,
    })
}

#[derive(Copy, Clone)]
struct State {
    kind: Kind,
    w: f32,
    h: f32,
    open: bool,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
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
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    let solid = matches!(
        kind,
        Kind::ConditionBlock
            | Kind::SeekerBarrier
            | Kind::SeekerStatue
            | Kind::TempleGate
            | Kind::TempleCrackedBlock
            | Kind::TheoCrystalHoldingBarrier
            | Kind::TheoCrystalPedestal
    );
    e.collision.solid(solid);
    if kind == Kind::PlayerSeeker || kind == Kind::TempleEye {
        e.hitbox.set(14.0, 14.0, -3.0, -3.0);
    }
    STATES.with(|s| {
        s.borrow_mut()
            .insert(id, State { kind, w, h, open: false });
    });
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    for pid in entities_by_type("player") {
        let p = Entity::new(pid).position.get();
        let (w, h, _, _) = Entity::new(pid).hitbox.get();
        return Some((p.x, p.y, w, h));
    }
    None
}

fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let p = e.position.get();
    match st.kind {
        Kind::PlayerSeeker | Kind::TempleEye => {
            if let Some((px, py, pw, ph)) = player_rect() {
                let dx = (px + pw * 0.5) - p.x;
                let dy = (py + ph * 0.5) - p.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let speed = if st.kind == Kind::TempleEye { 70.0 } else { 50.0 };
                if dist > 1.0 {
                    e.position.set_xy(
                        p.x + dx / dist * speed * dt,
                        p.y + dy / dist * speed * dt,
                    );
                }
                if overlap(p.x - 7.0, p.y - 7.0, 14.0, 14.0, px, py, pw, ph) {
                    die();
                }
            }
        }
        Kind::DashSwitchH | Kind::DashSwitchV => {
            if let Some((px, py, pw, ph)) = player_rect() {
                if overlap(p.x, p.y, st.w, st.h, px, py, pw, ph) {
                    emit(id, event::DASH_BLOCK, &[]);
                }
            }
        }
        Kind::TempleGate => {
            if drain_events().iter().any(|(_, t, _)| *t == event::SWITCH) {
                st.open = true;
            }
            e.collision.solid(!st.open);
        }
        Kind::TempleCrackedBlock => {
            if drain_events().iter().any(|(_, t, _)| *t == event::DASH_BLOCK) {
                remove(id);
                return;
            }
        }
        _ => {}
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow().get(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let p = e.position.get();
    let c = match st.kind {
        Kind::ConditionBlock => SOLID,
        Kind::SeekerBarrier | Kind::TheoCrystalHoldingBarrier => BARRIER,
        Kind::TempleGate => {
            if st.open {
                GATE_OPEN
            } else {
                GATE
            }
        }
        Kind::TempleCrackedBlock => SOLID,
        Kind::DashSwitchH | Kind::DashSwitchV => SWITCH,
        Kind::PlayerSeeker | Kind::TempleEye => HAZARD,
        Kind::TempleBigEyeball => EYE,
        Kind::SeekerStatue => STATUE,
        Kind::TheoCrystalPedestal => STATUE,
        Kind::TempleMirror | Kind::TempleMirrorPortal => MIRROR,
    };
    let (w, h) = if st.kind == Kind::PlayerSeeker || st.kind == Kind::TempleEye {
        (14.0, 14.0)
    } else {
        (st.w, st.h)
    };
    draw_rect(p.x, p.y, w, h, c);
    if st.kind == Kind::TempleEye || st.kind == Kind::PlayerSeeker {
        draw_rect(p.x - 3.0, p.y - 3.0, 6.0, 6.0, EYE);
    }
}
