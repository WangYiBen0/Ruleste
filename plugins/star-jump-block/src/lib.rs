#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `starJumpBlock` entity plugin.
//!
//! Per `StarJumpBlock.cs` these are permanent *solid* platforms (`Solid`,
//! depth -10000, `SurfaceSoundIndex` 32). When `sinks` is set they ride a slow
//! sine dip of 12px while the player stands on them and ease back up when left.
//! The star-boost itself is powered by the separate `StarJumpController`; the
//! block only sinks. It is never consumed.

use ruleste_plugins_api::host::{self, draw_image};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("star-jump-block");
ruleste_plugins_api::ruleste_entity_types!("starJumpBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// How far a `sinks` block dips when ridden.
const SINK_DISTANCE: f32 = 12.0;
/// `Calc.Approach(yLerp, ..., 1f * dt)`.
const SINK_RATE: f32 = 1.0;
/// `HasPlayerRider` hold time once the player steps off.
const SINK_HOLD: f32 = 0.1;

/// StarJumpBlock railings are 7-frame loops; the frame ids carry autotile
/// suffixes, so list them explicitly rather than generating by index.
const LEFT_RAIL: [&str; 7] = [
    "objects/starjumpBlock/leftrailing00",
    "objects/starjumpBlock/leftrailing01",
    "objects/starjumpBlock/leftrailing02",
    "objects/starjumpBlock/leftrailing03",
    "objects/starjumpBlock/leftrailing04U",
    "objects/starjumpBlock/leftrailing05",
    "objects/starjumpBlock/leftrailing06",
];
const CENTER_RAIL: [&str; 7] = [
    "objects/starjumpBlock/railing00M",
    "objects/starjumpBlock/railing01",
    "objects/starjumpBlock/railing02s",
    "objects/starjumpBlock/railing03",
    "objects/starjumpBlock/railing04W",
    "objects/starjumpBlock/railing05",
    "objects/starjumpBlock/railing06",
];
const RIGHT_RAIL: [&str; 7] = [
    "objects/starjumpBlock/rightrailing00M",
    "objects/starjumpBlock/rightrailing01",
    "objects/starjumpBlock/rightrailing02",
    "objects/starjumpBlock/rightrailing03I",
    "objects/starjumpBlock/rightrailing04",
    "objects/starjumpBlock/rightrailing05",
    "objects/starjumpBlock/rightrailing06H",
];

fn sine_in_out(t: f32) -> f32 {
    ((t * std::f32::consts::PI).cos() * -0.5 + 0.5).clamp(0.0, 1.0)
}

#[derive(Debug)]
struct JumpState {
    sinks: bool,
    start_y: f32,
    y_lerp: f32,
    sink_timer: f32,
    anim_time: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<JumpState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut JumpState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || unreachable!("state seeded at init")) as *mut JumpState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// True when a player rests on the block's top face (`HasPlayerRider`).
fn player_on_top(entity: &Entity) -> bool {
    let p = entity.position.get();
    let (w, ox, oy) = {
        let (w, _, ox, oy) = entity.hitbox.get();
        (w, ox, oy)
    };
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let bottom = pp.y + poy + ph;
        let overlap_x = pp.x + pox < p.x + ox + w && pp.x + pox + pw > p.x + ox;
        if overlap_x && bottom >= p.y + oy - 1.0 && bottom <= p.y + oy + 4.0 {
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
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    // `StarJumpBlock` is a `Solid` (not a one-way platform): the player can
    // neither fall through nor pass the sides.
    entity.collision.solid(true);
    entity.depth.set(-10000);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            JumpState {
                sinks: spawn.get_bool("sinks", false),
                start_y: y,
                y_lerp: 0.0,
                sink_timer: 0.0,
                anim_time: 0.0,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        st.anim_time += dt;

        // Sink behavior: ride a 12px sine dip while stood on, ease back up.
        if st.sinks {
            let riding = player_on_top(&entity);
            if riding {
                st.sink_timer = SINK_HOLD;
            } else if st.sink_timer > 0.0 {
                st.sink_timer -= dt;
            }
            if st.sink_timer > 0.0 {
                st.y_lerp = (st.y_lerp + SINK_RATE * dt).min(1.0);
            } else {
                st.y_lerp = (st.y_lerp - SINK_RATE * dt).max(0.0);
            }
            let target = st.start_y + SINK_DISTANCE * sine_in_out(st.y_lerp);
            let delta = target - p.y;
            if delta.abs() > 0.001 {
                let _ = entity.collision.actor_move(0.0, delta);
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let (w, h, ox, oy) = entity.hitbox.get();
        let p = entity.position.get();
        let bx = p.x + ox;
        let by = p.y + oy;
        let b = "objects/starjumpBlock/";

        // Body fill (the star slab).
        host::draw_rect(bx, by, w, h, Color::new(0x60, 0x38, 0xa8, 0xff));

        // Static 1px border from the starjumpBlock set (corners 8x8, edges
        // 8x1 / 1x8). The corner frames carry the rounded corners; the thin
        // edges trace the perimeter over the fill.
        draw_image(&format!("{b}corner00"), bx, by, 0.0, 1.0, 1.0);
        draw_image(&format!("{b}corner01"), bx + w - 8.0, by, 0.0, 1.0, 1.0);
        draw_image(&format!("{b}corner02"), bx, by + h - 8.0, 0.0, 1.0, 1.0);
        draw_image(
            &format!("{b}corner03"),
            bx + w - 8.0,
            by + h - 8.0,
            0.0,
            1.0,
            1.0,
        );
        let mut x = bx + 8.0;
        while x < bx + w - 8.0 {
            draw_image(&format!("{b}edgeH00"), x, by, 0.0, 1.0, 1.0);
            draw_image(&format!("{b}edgeH02"), x, by + h - 1.0, 0.0, 1.0, 1.0);
            x += 8.0;
        }
        let mut y = by + 8.0;
        while y < by + h - 8.0 {
            draw_image(&format!("{b}edgeV00"), bx, y, 0.0, 1.0, 1.0);
            draw_image(&format!("{b}edgeV02"), bx + w - 1.0, y, 0.0, 1.0, 1.0);
            y += 8.0;
        }

        // Animated glowing rail along the top edge (left cap + tiled centre + right cap).
        let ri = (st.anim_time * 12.0) as usize % 7;
        draw_image(LEFT_RAIL[ri], bx, by, 0.0, 1.0, 1.0);
        let mut rx = bx + 8.0;
        while rx < bx + w - 8.0 {
            draw_image(CENTER_RAIL[ri], rx, by, 0.0, 1.0, 1.0);
            rx += 8.0;
        }
        draw_image(RIGHT_RAIL[ri], bx + w - 8.0, by, 0.0, 1.0, 1.0);
    });
}
