#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `zipMover` entity plugin.
//!
//! Mirrors `ZipMover.cs`: a solid box that waits for a rider, slides back
//! and forth between its spawn point and its node with sine easing, carrying
//! anything standing on it.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("zip-mover");
ruleste_plugin_api::ruleste_entity_types!("zipMover");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

#[derive(Debug, Default)]
struct MoverState {
    from: Vec2,
    to: Vec2,
    w: f32,
    h: f32,
    /// 0: waiting for rider, 1: moving out, 2: pausing at target, 3: returning, 4: pausing at origin
    state: u32,
    timer: f32,
    progress: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<MoverState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut MoverState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, MoverState::default) as *mut MoverState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn player_on_top(entity: &Entity, w: f32) -> bool {
    let p = entity.position.get();
    let top = p.y;
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let bottom = pp.y + poy + ph;
        let overlap_x = pp.x + pox < p.x + w && pp.x + pox + pw > p.x;
        if overlap_x && bottom >= top - 1.0 && bottom <= top + 4.0 {
            return true;
        }
    }
    false
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
    entity.collision.solid(true);
    with_state(id, |st| {
        st.from = Vec2::new(x, y);
        st.to = spawn.get_node(0).unwrap_or(Vec2::new(x + 48.0, y));
        st.w = w;
        st.h = h;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let cur = entity.position.get();

        match st.state {
            0 => {
                if player_on_top(&entity, st.w) {
                    st.state = 1;
                    st.progress = 0.0;
                }
            }
            1 => {
                st.progress = (st.progress + dt / 1.2).min(1.0);
                let ease = 0.5 - 0.5 * (st.progress * std::f32::consts::PI).cos();
                let target = Vec2::new(
                    st.from.x + (st.to.x - st.from.x) * ease,
                    st.from.y + (st.to.y - st.from.y) * ease,
                );
                let _ = entity
                    .collision
                    .actor_move(target.x - cur.x, target.y - cur.y);
                if st.progress >= 1.0 {
                    st.state = 2;
                    st.timer = 2.0;
                }
            }
            2 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.state = 3;
                    st.progress = 0.0;
                }
            }
            3 => {
                st.progress = (st.progress + dt / 1.2).min(1.0);
                let ease = 0.5 - 0.5 * (st.progress * std::f32::consts::PI).cos();
                let target = Vec2::new(
                    st.to.x + (st.from.x - st.to.x) * ease,
                    st.to.y + (st.from.y - st.to.y) * ease,
                );
                let _ = entity
                    .collision
                    .actor_move(target.x - cur.x, target.y - cur.y);
                if st.progress >= 1.0 {
                    st.state = 0;
                }
            }
            _ => {}
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    host::draw_image(
        "objects/zipmover/block",
        p.x + ox + w * 0.5,
        p.y + oy + h * 0.5,
        0.0,
        1.0,
        1.0,
    );
}
