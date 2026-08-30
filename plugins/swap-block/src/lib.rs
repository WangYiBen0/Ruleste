#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `swapBlock` entity plugin — rewritten to mirror `SwapBlock.cs`.
//!
//! The dashed yellow mirror block: a solid (non-safe) platform that swaps to
//! the far end of its single node on every dash (`DashListener.OnDash`, here a
//! `PLAYER_DASH` event). It accelerates toward the far end, waits
//! `ReturnTime` (0.8 s), then glides back. Speeds derive from the span:
//! `maxForwardSpeed = 360 / distance`, `maxBackwardSpeed = 40%` of that. The
//! block moves through the actor API so riders come along.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("swap-block");
ruleste_plugins_api::ruleste_entity_types!("swapBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// `ReturnTime`: how long the block rests at the far end before returning.
const RETURN_TIME: f32 = 0.8;

#[derive(Debug, Default)]
struct SwapState {
    start: Vec2,
    end: Vec2,
    /// 0 = resting at `start`, 1 = swapped to `end`.
    lerp: f32,
    /// 0 = return, 1 = swap (the `SwapBlock.target`).
    target: f32,
    speed: f32,
    return_timer: f32,
    max_forward: f32,
    max_backward: f32,
    /// Animation phase for the centred swap face (≈12.5 fps, matching the sprite).
    phase: f32,
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

fn lerp_point(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn approach(value: f32, target: f32, max_move: f32) -> f32 {
    let diff = target - value;
    if diff.abs() <= max_move {
        target
    } else {
        value + diff.signum() * max_move
    }
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
    entity.depth.set(-9999);
    entity.collision.solid(true);
    with_state(id, |st| {
        let start = Vec2::new(x, y);
        let end = spawn.get_node(0).unwrap_or(start);
        st.start = start;
        st.end = end;
        let dist = (end.x - start.x).hypot(end.y - start.y).max(0.001);
        st.max_forward = 360.0 / dist;
        st.max_backward = st.max_forward * 0.4;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.phase += dt * 12.0;
        // `DashListener.OnDash` fires on every dash: start swapping.
        for (_, kind, _) in host::drain_events() {
            if kind == ruleste_plugins_api::plugin::event::PLAYER_DASH {
                st.target = 1.0;
                st.return_timer = RETURN_TIME;
                st.speed = if st.lerp >= 0.2 {
                    st.max_forward
                } else {
                    lerp(st.max_forward * 0.333, st.max_forward, st.lerp / 0.2)
                };
                break;
            }
        }

        // `Update`: the rest timer and speed approach.
        if st.return_timer > 0.0 {
            st.return_timer -= dt;
            if st.return_timer <= 0.0 {
                st.target = 0.0;
                st.speed = 0.0;
            }
        }
        if st.target == 1.0 {
            st.speed = approach(st.speed, st.max_forward, st.max_forward / 0.2 * dt);
        } else {
            st.speed = approach(st.speed, st.max_backward, st.max_backward / 1.5 * dt);
        }
        let new_lerp = approach(st.lerp, st.target, st.speed * dt);
        if new_lerp != st.lerp {
            let new_pos = lerp_point(st.start, st.end, new_lerp);
            let cur = Entity::new(id).position.get();
            let entity = Entity::new(id);
            // Move through the actor API so riders are carried (`MoveTo`).
            let _ = entity
                .collision
                .actor_move(new_pos.x - cur.x, new_pos.y - cur.y);
            st.lerp = new_lerp;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    let phase = with_state(id, |st| st.phase);

    // Body: the swap-block tile, repeated every 8px.
    let mut ty = oy;
    while ty < h {
        let mut tx = ox;
        while tx < w {
            host::draw_image("objects/swapblock/block", p.x + tx, p.y + ty, 0.0, 1.0, 1.0);
            tx += 8.0;
        }
        ty += 8.0;
    }

    // Animated swap face (midBlock00..03), centred on the block.
    let frame = ((phase as i32) % 4 + 4) % 4;
    let face = format!("objects/swapblock/midBlock{frame:02}");
    host::draw_image(
        &face,
        p.x + ox + w * 0.5 - 8.0,
        p.y + oy + h * 0.5 - 8.0,
        0.0,
        1.0,
        1.0,
    );
}
