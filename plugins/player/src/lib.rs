//! Built-in `player` entity plugin.
//!
//! Implements Madeline's normal-state movement as a Wasm plugin: running,
//! jumping (with jump grace and variable jump), ducking, wall slides and wall
//! jumps, and dashing. Physics constants mirror `Celeste.Player.cs`; state is
//! kept per entity in this module's linear memory and migrated across hot
//! reloads through the serialize/deserialize pair.

use std::cell::RefCell;

use ruleste_plugin_api::host::{ActorMoveResult, Input};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::ruleste_entity_types;
use ruleste_plugin_api::ruleste_meta;
use ruleste_plugin_api::ruleste_noop_destroy;
use ruleste_plugin_api::types::input;
use ruleste_plugin_api::types::{EntityId, Vec2};
const GRAVITY: f32 = 900.0;
const MAX_FALL: f32 = 160.0;
const FAST_MAX_FALL: f32 = 240.0;
const MAX_RUN: f32 = 90.0;
const RUN_ACCEL: f32 = 1000.0;
const RUN_REDUCE: f32 = 400.0;
const AIR_MULT: f32 = 0.65;
const DUCK_FRICTION: f32 = 500.0;
const JUMP_GRACE_TIME: f32 = 0.1;
const JUMP_SPEED: f32 = -105.0;
const JUMP_HBOOST: f32 = 40.0;
const VAR_JUMP_TIME: f32 = 0.2;
const DASH_SPEED: f32 = 240.0;
const DASH_TIME: f32 = 0.15;
const DASH_COOLDOWN: f32 = 0.2;
const END_DASH_SPEED: f32 = 160.0;
const END_DASH_UP_MULT: f32 = 0.75;
const WALL_JUMP_HSPEED: f32 = 130.0;
const WALL_JUMP_SPEED: f32 = -160.0;
const WALL_SLIDE_MAX_FALL: f32 = 20.0;
const WALL_SLIDE_TIME: f32 = 1.2;
const WALL_CHECK_DIST: f32 = 3.0;
const SUPER_JUMP_H: f32 = 260.0;
const SUPER_JUMP_Y: f32 = -105.0;
const DUCK_SUPER_X_MULT: f32 = 1.25;
const DUCK_SUPER_Y_MULT: f32 = 0.5;
const DASH_FLATTEN_MULT: f32 = 1.2;

/// Hitboxes (`Player.cs`): size and offset relative to the foot-center anchor.
const NORMAL_HITBOX: (f32, f32, f32, f32) = (8.0, 11.0, -4.0, -11.0);
const DUCK_HITBOX: (f32, f32, f32, f32) = (8.0, 6.0, -4.0, -6.0);

/// State-machine values mirroring `Player.cs` `StNormal`/`StClimb`/`StDash`,
/// for the debug state dump (`RULESTE_DEBUG=1`). `StRedDash`/`StSwim` etc. are
/// not reachable from the normal/dash/climb set this plugin implements.
const ST_NORMAL: u32 = 0;
const ST_CLIMB: u32 = 1;
const ST_DASH: u32 = 2;

ruleste_meta!("player");
ruleste_entity_types!("player");
ruleste_noop_destroy!();

/// Per-entity movement state.
#[derive(Clone, Copy)]
struct PlayerState {
    facing: i32,
    ducking: bool,
    jump_grace: f32,
    var_jump_timer: f32,
    var_jump_speed: f32,
    dash_timer: f32,
    dash_cooldown: f32,
    dashes: i32,
    dash_dir: Vec2,
    wall_slide_timer: f32,
    wall_slide_dir: i32,
    /// Last derived state-machine value, used to log transitions in debug mode.
    debug_state: u32,
}

impl Default for PlayerState {
    fn default() -> PlayerState {
        PlayerState {
            facing: 1,
            ducking: false,
            jump_grace: 0.0,
            var_jump_timer: 0.0,
            var_jump_speed: 0.0,
            dash_timer: 0.0,
            dash_cooldown: 0.0,
            dashes: 1,
            dash_dir: Vec2::ZERO,
            wall_slide_timer: 0.0,
            wall_slide_dir: 0,
            debug_state: ST_NORMAL,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<PlayerState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

fn state(id: EntityId) -> PlayerState {
    STATES.with(|s| s.borrow().get(id).copied().unwrap_or_default())
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut PlayerState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, PlayerState::default) as *mut PlayerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[no_mangle]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity.hitbox.set(
        NORMAL_HITBOX.0,
        NORMAL_HITBOX.1,
        NORMAL_HITBOX.2,
        NORMAL_HITBOX.3,
    );
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    entity.position.set_xy(x, y);
    entity.speed.set_xy(0.0, 0.0);
    with_state(id, |_| {});
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    handle_events(id);
    let entity = Entity::new(id);
    let mut speed = entity.speed.get();

    if try_start_dash(&entity, dt, &mut speed) {
        entity.speed.set(speed);
        return;
    }

    let mut st = state(id);
    let grounded = entity.collision.is_grounded();
    let move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
    let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);

    st.jump_grace -= dt;
    st.var_jump_timer -= dt;
    st.dash_cooldown -= dt;
    if grounded {
        st.dash_cooldown = 0.0;
        st.dashes = 1;
        // Keep the coyote timer topped up while grounded: when standing flush
        // on ground the vertical move below is a no-op (dy == 0), so the jump
        // would only be re-armed on the landing frame of an actual fall.
        st.jump_grace = JUMP_GRACE_TIME;
    }
    if move_x != 0.0 {
        st.facing = if move_x > 0.0 { 1 } else { -1 };
    }

    // Ducking.
    let want_duck = grounded && move_y > 0.0 && speed.y >= 0.0;
    if want_duck && !st.ducking {
        st.ducking = true;
        entity
            .hitbox
            .set(DUCK_HITBOX.0, DUCK_HITBOX.1, DUCK_HITBOX.2, DUCK_HITBOX.3);
    } else if !want_duck && st.ducking {
        st.ducking = false;
        entity.hitbox.set(
            NORMAL_HITBOX.0,
            NORMAL_HITBOX.1,
            NORMAL_HITBOX.2,
            NORMAL_HITBOX.3,
        );
    }

    // Horizontal movement.
    if st.ducking && grounded {
        speed.x = approach(speed.x, 0.0, DUCK_FRICTION * dt);
    } else {
        let mult = if grounded { 1.0 } else { AIR_MULT };
        let (accel, target) =
            if speed.x.abs() > MAX_RUN && move_x != 0.0 && speed.x.signum() == move_x.signum() {
                (RUN_REDUCE * mult, MAX_RUN * move_x)
            } else {
                (RUN_ACCEL * mult, MAX_RUN * move_x)
            };
        speed.x = approach(speed.x, target, accel * dt);
    }

    // Gravity and max fall speed.
    if !grounded {
        let target = if move_y > 0.0 && speed.y >= MAX_FALL {
            FAST_MAX_FALL
        } else {
            MAX_FALL
        };
        let half_grav = Input::button(input::JUMP) && speed.y.abs() < 40.0;
        let grav = GRAVITY * if half_grav { 0.5 } else { 1.0 };
        speed.y = approach(speed.y, target, grav * dt);
    }

    // Variable jump (lift cancel on release).
    if st.var_jump_timer > 0.0 {
        if Input::button(input::JUMP) {
            speed.y = speed.y.min(st.var_jump_speed);
        } else {
            st.var_jump_timer = 0.0;
        }
    }

    // Jumping: ground jump with grace, or wall jump.
    if Input::pressed(input::JUMP) {
        let wall_left = entity.collision.check(-WALL_CHECK_DIST, 0.0);
        let wall_right = entity.collision.check(WALL_CHECK_DIST, 0.0);
        if st.jump_grace > 0.0 {
            st.jump_grace = 0.0;
            speed.x += JUMP_HBOOST * move_x;
            speed.y = JUMP_SPEED;
            st.var_jump_timer = VAR_JUMP_TIME;
            st.var_jump_speed = speed.y;
            entity.speed.set(speed);
            Input::consume(input::JUMP);
            ruleste_plugin_api::host::emit(id, ruleste_plugin_api::plugin::event::PLAYER_JUMP, &[]);
        } else if !st.ducking && wall_right {
            speed.x = -WALL_JUMP_HSPEED;
            speed.y = WALL_JUMP_SPEED;
            st.var_jump_timer = VAR_JUMP_TIME;
            st.var_jump_speed = speed.y;
            st.facing = -1;
            st.wall_slide_dir = 0;
            st.wall_slide_timer = 0.0;
            entity.speed.set(speed);
            Input::consume(input::JUMP);
            ruleste_plugin_api::host::emit(id, ruleste_plugin_api::plugin::event::PLAYER_JUMP, &[]);
        } else if !st.ducking && wall_left {
            speed.x = WALL_JUMP_HSPEED;
            speed.y = WALL_JUMP_SPEED;
            st.var_jump_timer = VAR_JUMP_TIME;
            st.var_jump_speed = speed.y;
            st.facing = 1;
            st.wall_slide_dir = 0;
            st.wall_slide_timer = 0.0;
            entity.speed.set(speed);
            Input::consume(input::JUMP);
            ruleste_plugin_api::host::emit(id, ruleste_plugin_api::plugin::event::PLAYER_JUMP, &[]);
        }
    }

    // Wall sliding.
    if !grounded && !st.ducking && st.dash_timer <= 0.0 {
        let wall_left = entity.collision.check(-WALL_CHECK_DIST, 0.0);
        let wall_right = entity.collision.check(WALL_CHECK_DIST, 0.0);
        let toward_wall = (move_x < 0.0 && wall_left) || (move_x > 0.0 && wall_right);
        if toward_wall && speed.y >= 0.0 {
            st.wall_slide_dir = if wall_right { 1 } else { -1 };
            st.wall_slide_timer = WALL_SLIDE_TIME;
            let t = (st.wall_slide_timer / WALL_SLIDE_TIME).clamp(0.0, 1.0);
            let cap = MAX_FALL + (WALL_SLIDE_MAX_FALL - MAX_FALL) * t;
            if speed.y > cap {
                speed.y = cap;
            }
        } else {
            st.wall_slide_dir = 0;
        }
    }

    // Horizontal movement and collision.
    let result_h = entity.collision.actor_move(speed.x * dt, 0.0);
    if (result_h.hit_wall_left && speed.x < 0.0) || (result_h.hit_wall_right && speed.x > 0.0) {
        speed.x = 0.0;
    }

    // Vertical movement and collision.
    let result_v = entity.collision.actor_move(0.0, speed.y * dt);
    if result_v.on_ground && speed.y > 0.0 {
        speed.y = 0.0;
    }
    if (result_v.hit_wall_left && speed.x < 0.0) || (result_v.hit_wall_right && speed.x > 0.0) {
        // Horizontal collision during vertical move (e.g., corner).
        speed.x = 0.0;
    }
    if result_v.hit_ceiling && speed.y < 0.0 {
        speed.y = 0.0;
        st.var_jump_timer = 0.0;
    }
    if result_v.on_ground {
        st.jump_grace = JUMP_GRACE_TIME;
    }
    entity.speed.set(speed);

    debug_state_log(id, &st);
    STATES.with(|s| {
        s.borrow_mut().insert(id, st);
    });
}

/// Derives the current state-machine value (mirroring `Player.cs` `State`)
/// and, in debug mode, logs a line when it changes.
fn debug_state_log(id: EntityId, st: &PlayerState) {
    let state = if st.dash_timer > 0.0 {
        ST_DASH
    } else if st.wall_slide_dir != 0 {
        ST_CLIMB
    } else {
        ST_NORMAL
    };
    if st.debug_state == state {
        return;
    }
    if !ruleste_plugin_api::host::debug_enabled() {
        return;
    }
    let name = match state {
        ST_DASH => "Dash",
        ST_CLIMB => "Climb",
        _ => "Normal",
    };
    ruleste_plugin_api::host::log(&format!("player {id} state -> {name} ({state})"));
}

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let st = state(id);
    let grounded = entity.collision.is_grounded();
    let speed = entity.speed.get();

    entity.sprite.flip_x(st.facing == -1);

    let anim = if st.dash_timer > 0.0 {
        "dash"
    } else if st.ducking && grounded {
        "duck"
    } else if !grounded && st.wall_slide_dir != 0 {
        "wallslide"
    } else if !grounded && speed.y < 0.0 {
        if speed.x.abs() > 90.0 {
            "jumpFast"
        } else {
            "jumpSlow"
        }
    } else if !grounded {
        if speed.y >= 160.0 {
            "fallFast"
        } else {
            "fallSlow"
        }
    } else if speed.x.abs() > 45.0 {
        "runFast"
    } else if speed.x.abs() > 0.0 {
        "runSlow"
    } else {
        "idle"
    };

    if entity.sprite.animation() != anim {
        entity.sprite.play(anim);
    }
}

/// Handles events from other plugins (refills, boosters) before movement.
/// Refills top up the dash count (via `EV_REFILL`); boosters launch the player
/// along the held aim direction (via `EV_BOOST`), refilling dashes like the
/// original's `BoostBegin`.
fn handle_events(id: EntityId) {
    for (_, kind, data) in ruleste_plugin_api::host::drain_events() {
        with_state(id, |st| match kind {
            ruleste_plugin_api::host::EV_REFILL => {
                let two = data.first().copied().unwrap_or(0) != 0;
                st.dashes = if two { 2 } else { 1 };
                st.dash_cooldown = 0.0;
            }
            ruleste_plugin_api::host::EV_BOOST => {
                let move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
                let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
                let dir = if move_x != 0.0 || move_y != 0.0 {
                    normalize(move_x, move_y)
                } else {
                    Vec2::new(st.facing as f32, 0.0)
                };
                st.dashes = 1;
                st.dash_dir = dir;
                st.dash_timer = DASH_TIME;
                st.dash_cooldown = 0.0;
                st.wall_slide_dir = 0;
                ruleste_plugin_api::host::Speed::new(id)
                    .set(Vec2::new(dir.x * DASH_SPEED, dir.y * DASH_SPEED));
                ruleste_plugin_api::host::emit(
                    id,
                    ruleste_plugin_api::plugin::event::PLAYER_DASH,
                    &[],
                );
            }
            _ => {}
        });
    }
}

/// Attempts to start a dash. Returns `true` when the player is dashing and the
/// normal movement step must be skipped.
fn try_start_dash(entity: &Entity, dt: f32, speed: &mut Vec2) -> bool {
    if Input::pressed(input::DASH) {
        with_state(entity.id, |st| {
            if st.dashes > 0 && st.dash_cooldown <= 0.0 {
                let move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
                let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
                let dir = if move_x != 0.0 || move_y != 0.0 {
                    normalize(move_x, move_y)
                } else {
                    Vec2::new(st.facing as f32, 0.0)
                };
                st.dash_dir = dir;
                st.dash_timer = DASH_TIME;
                st.dashes -= 1;
                st.wall_slide_dir = 0;
                if dir.x != 0.0 {
                    st.facing = if dir.x > 0.0 { 1 } else { -1 };
                }
                // DashBegin: an airborne duck resumes standing when the dash
                // allows it; holding down while starting a dash ducks.
                if !entity.collision.is_grounded() && st.ducking && can_unduck(entity, true) {
                    st.ducking = false;
                    entity.hitbox.set(
                        NORMAL_HITBOX.0,
                        NORMAL_HITBOX.1,
                        NORMAL_HITBOX.2,
                        NORMAL_HITBOX.3,
                    );
                } else if !st.ducking && move_y > 0.0 {
                    st.ducking = true;
                    entity
                        .hitbox
                        .set(DUCK_HITBOX.0, DUCK_HITBOX.1, DUCK_HITBOX.2, DUCK_HITBOX.3);
                }
                *speed = Vec2::new(dir.x * DASH_SPEED, dir.y * DASH_SPEED);
                Input::consume(input::DASH);
                ruleste_plugin_api::host::emit(
                    entity.id,
                    ruleste_plugin_api::plugin::event::PLAYER_DASH,
                    &[],
                );
            }
        });
    }

    with_state(entity.id, |st| {
        if st.dash_timer > 0.0 {
            // SuperJump: while a horizontal dash is near the ground, a jump
            // press converts into a 260 u/s leap (325 u/s and flatter arc from
            // a duck) — the basis of hypers and wavedashes.
            if st.dash_dir.y.abs() < 0.1
                && Input::pressed(input::JUMP)
                && st.jump_grace > 0.0
                && can_unduck(entity, st.ducking)
            {
                st.dash_timer = 0.0;
                st.dash_cooldown = DASH_COOLDOWN;
                st.jump_grace = 0.0;
                st.var_jump_timer = VAR_JUMP_TIME;
                st.wall_slide_timer = WALL_SLIDE_TIME;
                *speed = Vec2::new(SUPER_JUMP_H * st.facing as f32, SUPER_JUMP_Y);
                if st.ducking {
                    st.ducking = false;
                    entity.hitbox.set(
                        NORMAL_HITBOX.0,
                        NORMAL_HITBOX.1,
                        NORMAL_HITBOX.2,
                        NORMAL_HITBOX.3,
                    );
                    speed.x *= DUCK_SUPER_X_MULT;
                    speed.y *= DUCK_SUPER_Y_MULT;
                }
                st.var_jump_speed = speed.y;
                Input::consume(input::JUMP);
                ruleste_plugin_api::host::emit(
                    entity.id,
                    ruleste_plugin_api::plugin::event::PLAYER_JUMP,
                    &[],
                );
                return true;
            }
            if entity.collision.is_grounded() {
                st.jump_grace = JUMP_GRACE_TIME;
            }
            st.dash_timer -= dt;
            let d = st.dash_dir;
            let result: ActorMoveResult = entity.collision.actor_move(speed.x * dt, speed.y * dt);
            let crushed = dash_hits_crushblock(entity);
            if crushed {
                emit_crush(entity.id, d);
            }
            // OnCollideV / DashCoroutine: a diagonal-down dash that reaches the
            // ground flattens into a horizontal 1.2x dash and ducks, instead of
            // ending — the setup for a hyperdash off the dash.
            if result.on_ground && d.x != 0.0 && d.y > 0.0 && speed.y > 0.0 {
                st.dash_dir = Vec2::new(d.x.signum(), 0.0);
                *speed = Vec2::new(speed.x * DASH_FLATTEN_MULT, 0.0);
                if !st.ducking {
                    st.ducking = true;
                    entity
                        .hitbox
                        .set(DUCK_HITBOX.0, DUCK_HITBOX.1, DUCK_HITBOX.2, DUCK_HITBOX.3);
                }
            } else if st.dash_timer <= 0.0
                || result.hit_wall_left
                || result.hit_wall_right
                || result.hit_ceiling
                || crushed
            {
                st.dash_timer = 0.0;
                st.dash_cooldown = DASH_COOLDOWN;
                // DashCoroutine end: walls/ceiling reflect the tail speed;
                // forward/upward dashes taper to `DashDir * 160` with upward
                // parts trimmed to 0.75x; downward dashes keep their momentum.
                let dx = if result.hit_wall_left {
                    -1.0
                } else if result.hit_wall_right {
                    1.0
                } else {
                    d.x
                };
                let dy = if result.hit_ceiling { 1.0 } else { d.y };
                if result.hit_wall_left || result.hit_wall_right || result.hit_ceiling {
                    let n = (dx * dx + dy * dy).sqrt().max(1.0);
                    *speed = Vec2::new(dx / n * END_DASH_SPEED, dy / n * END_DASH_SPEED);
                } else if d.y <= 0.0 {
                    *speed = Vec2::new(d.x * END_DASH_SPEED, d.y * END_DASH_SPEED);
                    if speed.y < 0.0 {
                        speed.y *= END_DASH_UP_MULT;
                    }
                }
            }
            true
        } else {
            false
        }
    })
}

/// True when the player can stand up right now: if ducking, the normal
/// hitbox's extra 5 px of headroom over the duck hitbox must be clear.
fn can_unduck(entity: &Entity, ducking: bool) -> bool {
    !ducking || !entity.collision.check(0.0, -5.0)
}

/// True when the dashing player overlaps a `crushBlock` entity's hitbox.
/// The dash pushes into the block from the player's side; the block then
/// crushes in the opposite direction.
fn dash_hits_crushblock(entity: &Entity) -> bool {
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    let px = p.x + ox;
    let py = p.y + oy;
    for block_id in ruleste_plugin_api::host::entities_by_type("crushBlock") {
        if !ruleste_plugin_api::host::entity_alive(block_id) {
            continue;
        }
        let bp = ruleste_plugin_api::host::Position::new(block_id).get();
        let (bw, bh, box_, boy) = ruleste_plugin_api::host::Hitbox::new(block_id).get();
        let bx = bp.x + box_;
        let by = bp.y + boy;
        let overlap = px < bx + bw && px + w > bx && py < by + bh && py + h > by;
        if overlap {
            return true;
        }
    }
    false
}

/// Tells a touched `crushBlock` that the player dashed into it, sending the
/// dash direction so the block can crush opposite to it.
fn emit_crush(id: EntityId, d: Vec2) {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&d.x.to_le_bytes());
    buf[4..8].copy_from_slice(&d.y.to_le_bytes());
    ruleste_plugin_api::host::emit(id, ruleste_plugin_api::host::EV_CRUSH, &buf);
}

#[no_mangle]
pub extern "C" fn ruleste_entity_serialize(id: EntityId, out_len: *mut u32) -> u32 {
    let st = state(id);
    let mut buf = Vec::with_capacity(64);
    push_i32(&mut buf, st.facing);
    push_u8(&mut buf, u8::from(st.ducking));
    push_f32(&mut buf, st.jump_grace);
    push_f32(&mut buf, st.var_jump_timer);
    push_f32(&mut buf, st.var_jump_speed);
    push_f32(&mut buf, st.dash_timer);
    push_f32(&mut buf, st.dash_cooldown);
    push_i32(&mut buf, st.dashes);
    push_f32(&mut buf, st.dash_dir.x);
    push_f32(&mut buf, st.dash_dir.y);
    push_f32(&mut buf, st.wall_slide_timer);
    push_i32(&mut buf, st.wall_slide_dir);
    push_u32(&mut buf, st.debug_state);
    unsafe { *out_len = buf.len() as u32 }
    let ptr = buf.as_ptr() as u32;
    SER_BUF.with(|b| *b.borrow_mut() = buf);
    ptr
}

#[no_mangle]
pub extern "C" fn ruleste_entity_deserialize(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    if let Some(st) = parse_state(bytes) {
        STATES.with(|s| {
            s.borrow_mut().insert(id, st);
        });
    }
}

fn push_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

fn parse_state(bytes: &[u8]) -> Option<PlayerState> {
    let mut r = Cursor::new(bytes);
    Some(PlayerState {
        facing: r.i32()?,
        ducking: r.u8()? != 0,
        jump_grace: r.f32()?,
        var_jump_timer: r.f32()?,
        var_jump_speed: r.f32()?,
        dash_timer: r.f32()?,
        dash_cooldown: r.f32()?,
        dashes: r.i32()?,
        dash_dir: Vec2::new(r.f32()?, r.f32()?),
        wall_slide_timer: r.f32()?,
        wall_slide_dir: r.i32()?,
        debug_state: r.u32()?,
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Cursor<'a> {
        Cursor { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let slice = self.bytes.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

fn approach(value: f32, target: f32, max_move: f32) -> f32 {
    let diff = target - value;
    if diff.abs() <= max_move {
        target
    } else {
        value + diff.signum() * max_move
    }
}

fn normalize(x: f32, y: f32) -> Vec2 {
    let len = (x * x + y * y).sqrt();
    Vec2::new(x / len, y / len)
}
