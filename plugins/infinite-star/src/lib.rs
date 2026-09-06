#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `infiniteStar` entity plugin — reworked as a `FlyFeather` (`FlyFeather.cs`).
//!
//! The floating golden feather of chapter 6. Touching it grants the player the
//! star-fly state (`EV_STARFLY`), after which it hides and either respawns 3 s
//! later (`singleUse=false`, the default) or is collected for the session
//! (`singleUse=true`). A `shielded` feather instead bounces the player away
//! (`Player.PointBounce`) unless they are dash-attacking. Drawing is procedural
//! (four-point star + shield ring) so it reads clearly without the atlas art.

use ruleste_plugins_api::host::{self, draw_image, draw_line, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("infinite-star");
ruleste_plugins_api::ruleste_entity_types!("infiniteStar");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// `FlyFeather.cs`: `new Hitbox(20f, 20f, -10f, -10f)`.
const HIT: f32 = 20.0;
/// `RespawnTime`.
const RESPAWN_TIME: f32 = 3.0;
/// `Player.StartStarFly` duration.
const STARFLY_STRENGTH: f32 = 2.0;
/// `Player.PointBounce` speed.
const BOUNCE_SPEED: f32 = 220.0;

#[derive(Debug, Default)]
struct FeatherState {
    timer: f32,
    /// Hidden while awaiting respawn (`respawnTimer` in the original).
    hidden: bool,
    respawn_timer: f32,
    shielded: bool,
    single_use: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FeatherState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FeatherState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FeatherState::default) as *mut FeatherState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn player_center(player_id: EntityId) -> Vec2 {
    let pp = host::Position::new(player_id).get();
    let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
    Vec2::new(pp.x + pox + pw * 0.5, pp.y + poy + ph * 0.5)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.hitbox.set(HIT, HIT, -HIT * 0.5, -HIT * 0.5);
    entity.depth.set(0);
    with_state(id, |st| {
        st.shielded = spawn.get_bool("shielded", false);
        st.single_use = spawn.get_bool("singleUse", false);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        if st.respawn_timer > 0.0 {
            st.respawn_timer -= dt;
            if st.respawn_timer <= 0.0 {
                // `Respawn`: become visible and solid again.
                st.hidden = false;
                host::set_visible(id, true);
            }
        }
        if st.hidden {
            return;
        }
        let p = Entity::new(id).position.get();
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pc = player_center(player_id);
            let dx = pc.x - p.x;
            let dy = pc.y - p.y;
            if dx.abs() > HIT * 0.5 || dy.abs() > HIT * 0.5 {
                continue;
            }
            if st.shielded && !dash_attacking(player_id) {
                point_bounce(player_id, p);
            } else if !st.hidden {
                // `OnPlayer` → `Player.StartStarFly`.
                let mut payload = Vec::with_capacity(4);
                payload.extend_from_slice(&STARFLY_STRENGTH.to_le_bytes());
                host::emit(player_id, host::EV_STARFLY, &payload);
                st.hidden = true;
                host::set_visible(id, false);
                if st.single_use {
                    // Consumed for the session (`singleUse`).
                    host::collect(id);
                } else {
                    st.respawn_timer = RESPAWN_TIME;
                }
            }
            break;
        }
    });
}

/// True when the player is dashing or otherwise moving fast enough that a
/// shielded feather grants flight instead of bouncing (`DashAttacking`).
fn dash_attacking(player_id: EntityId) -> bool {
    let speed = host::Speed::new(player_id).get();
    speed.length() > 240.0
}

/// `Player.PointBounce(from)`: bounce the player away from the feather.
fn point_bounce(player_id: EntityId, from: Vec2) {
    let pc = player_center(player_id);
    let mut dx = pc.x - from.x;
    let mut dy = pc.y - from.y;
    let len = (dx * dx + dy * dy).sqrt().max(1e-4);
    dx /= len;
    dy /= len;
    // `if (vector.Y > -0.2f && vector.Y <= 0.4f) vector.Y = -0.2f;`
    if dy > -0.2 && dy <= 0.4 {
        dy = -0.2;
    }
    let mut vx = dx * BOUNCE_SPEED;
    let vy = dy * BOUNCE_SPEED;
    vx *= 1.5;
    if vx.abs() < 100.0 {
        vx = if vx == 0.0 {
            100.0
        } else {
            vx.signum() * 100.0
        };
    }
    host::Speed::new(player_id).set(Vec2::new(vx, vy));
    // `PointBounce` also refills.
    let payload = vec![1];
    host::emit(player_id, host::EV_REFILL, &payload);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.hidden {
            return;
        }
        let p = Entity::new(id).position.get();
        let bob = (st.timer * 3.0).sin() * 2.0;
        let y = p.y + bob;
        // Real fly feather sprite (objects/flyFeather/idle00, 16x17).
        draw_image(
            "objects/flyFeather/idle00",
            p.x - 8.0,
            y - 8.5,
            0.0,
            1.0,
            1.0,
        );
        if st.shielded {
            // Shield ring (`Draw.Circle` approximation).
            let r = 10.0 - (st.timer * 2.0).sin() * 1.5;
            let white = Color::new(0xff, 0xff, 0xff, 0xd0);
            let steps = 24;
            for i in 0..steps {
                let a0 = i as f32 * std::f32::consts::TAU / steps as f32;
                let a1 = (i + 1) as f32 * std::f32::consts::TAU / steps as f32;
                draw_line(
                    p.x + a0.cos() * r,
                    y + a0.sin() * r,
                    p.x + a1.cos() * r,
                    y + a1.sin() * r,
                    white,
                );
            }
        }
    });
}
