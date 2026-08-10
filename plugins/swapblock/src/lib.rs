#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `swapBlock` entity plugin.
//!
//! Mirrors `SwapBlock.cs` (the dashed yellow mirrors block): a solid 16x16
//! block that stays put until Madeline dashes into it, then snap-moves to the
//! other end of its node chain and waits there. A second dash swaps it back.
//! It is a dynamic platform, so riders come along during the swap.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};
use std::vec::Vec;

ruleste_plugin_api::ruleste_meta!("swapblock");
ruleste_plugin_api::ruleste_entity_types!("swapBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Snappish swap speed in pixels/second (the block feels fast and loose).
const SWAP_SPEED: f32 = 220.0;
/// Dash slam threshold to count as an intentional hit.
const HIT_SPEED: f32 = 220.0;

#[derive(Debug, Default)]
struct SwapState {
    /// All visited rest positions, starting at the current one.
    chain: Vec<Vec2>,
    /// Index of the current rest position in `chain`.
    idx: usize,
    /// Active destination, `None` while resting.
    target: Option<Vec2>,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SwapState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SwapState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SwapState::default) as *mut SwapState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 16.0);
    let h = spawn.get_float("height", 16.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(500);
    entity.collision.platform(true);
    with_state(id, |st| {
        st.chain = std::iter::once(Vec2::new(x, y))
            .chain(spawn.nodes().iter().copied())
            .collect();
        st.idx = 0;
    });
}

fn player_rammed(id: EntityId) -> bool {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let overlap = pp.x + pox < p.x + ox + w
            && pp.x + pox + pw > p.x + ox
            && pp.y + poy < p.y + oy + h
            && pp.y + poy + ph > p.y + oy;
        if overlap {
            let v = host::Speed::new(player_id).get();
            return v.x.abs() >= HIT_SPEED || v.y.abs() >= HIT_SPEED;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let cur = entity.position.get();
        match st.target {
            None => {
                if player_rammed(id) {
                    let n = st.chain.len();
                    st.idx = (st.idx + 1) % n;
                    st.target = Some(st.chain[st.idx]);
                }
            }
            Some(dest) => {
                let dx = dest.x - cur.x;
                let dy = dest.y - cur.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let step = (SWAP_SPEED * dt).min(dist);
                if dist < 0.5 {
                    st.target = None;
                } else {
                    let ux = dx / dist;
                    let uy = dy / dist;
                    let _on_move = entity.collision.actor_move(ux * step, uy * step);
                }
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    // Dashed yellow block: bright fill with a dark dashed outline.
    ruleste_plugin_api::host::draw_rect(p.x + ox, p.y + oy, w, h, color_bg());
    ruleste_plugin_api::host::draw_rect(p.x + ox, p.y + oy, w, 2.0, color_edge());
}

fn color_bg() -> ruleste_plugin_api::types::Color {
    ruleste_plugin_api::types::Color::new(0xd8, 0xc0, 0x3a, 0xff)
}

fn color_edge() -> ruleste_plugin_api::types::Color {
    ruleste_plugin_api::types::Color::new(0x7a, 0x64, 0x18, 0xff)
}
