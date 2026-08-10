#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! Built-in `player` entity plugin.
//!
//! Implements the `Celeste.Player.cs` state machine as a Wasm plugin:
//!
//! - `StNormal` — running, jumping (coyote grace + variable jump), ducking,
//!   wall slides, and grabbing into a climb;
//! - `StClimb` — grabs, climbs up/down/neutral, stamina drain (`ClimbUpCost` /
//!   `ClimbStillCost` / `ClimbJumpCost`), climb jump and wall jump out;
//! - `StDash` — 8-way dash, diagonal ground flatten (hyper setup), super jump,
//!   wavedash (jump on the landing frame), super wall jump.
//!
//! Constants and control flow mirror `Player.cs`. Subsystems that need engine
//! features that don't exist yet (water / `StSwim`, dream blocks, boosters with
//! an in-spiral hold, corner corrections, wall boosters, gliders, ice) are
//! deliberately omitted and marked below — see `ROADMAP.md`.
//!
//! State is kept per entity in this module's linear memory and migrated across
//! hot reloads through the `serialize`/`deserialize` pair.

// The `ruleste_*` export macros generate FFI glue that deliberately takes raw
// pointers; the deref happens inside `unsafe` blocks at the call sites.

use std::cell::RefCell;

use ruleste_plugin_api::host::Input;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::ruleste_entity_types;
use ruleste_plugin_api::ruleste_meta;
use ruleste_plugin_api::ruleste_noop_destroy;
use ruleste_plugin_api::types::input;
use ruleste_plugin_api::types::{EntityId, Vec2};

// Movement / physics (`Player.cs`).
const GRAVITY: f32 = 900.0;
const HALF_GRAV_THRESHOLD: f32 = 40.0;
const MAX_FALL: f32 = 160.0;
const FAST_MAX_FALL: f32 = 240.0;
const FAST_MAX_ACCEL: f32 = 300.0;
const MAX_RUN: f32 = 90.0;
const RUN_ACCEL: f32 = 1000.0;
const RUN_REDUCE: f32 = 400.0;
const AIR_MULT: f32 = 0.65;
const DUCK_FRICTION: f32 = 500.0;

// Jumping.
const JUMP_GRACE_TIME: f32 = 0.1;
const JUMP_SPEED: f32 = -105.0;
const JUMP_HBOOST: f32 = 40.0;
const VAR_JUMP_TIME: f32 = 0.2;
const JUMP_VAR_JUMP_TIME: f32 = 0.2;

// Wall jumps / slides.
const WALL_JUMP_CHECK_DIST: f32 = 3.0;
const WALL_JUMP_SPEED: f32 = -105.0;
const WALL_JUMP_HSPEED: f32 = 130.0;
const SUPER_WALL_JUMP_SPEED: f32 = -160.0;
const SUPER_WALL_JUMP_HSPEED: f32 = 170.0;
const SUPER_WALL_JUMP_VAR_TIME: f32 = 0.25;
const WALL_JUMP_FORCE_TIME: f32 = 0.16;
const WALL_SLIDE_START_MAX: f32 = 20.0;
const WALL_SLIDE_TIME: f32 = 1.2;
const WALL_CHECK_DIST: f32 = 3.0;

// Climbing.
const CLIMB_MAX_STAMINA: f32 = 110.0;
const CLIMB_UP_COST: f32 = 45.454544;
const CLIMB_STILL_COST: f32 = 10.0;
const CLIMB_JUMP_COST: f32 = 27.5;
const CLIMB_UP_SPEED: f32 = -45.0;
const CLIMB_DOWN_SPEED: f32 = 80.0;
const CLIMB_SLIP_SPEED: f32 = 30.0;
const CLIMB_ACCEL: f32 = 900.0;
const CLIMB_GRAB_Y_MULT: f32 = 0.2;
const CLIMB_HOP_Y: f32 = -120.0;
const CLIMB_HOP_X: f32 = 100.0;
const CLIMB_NO_MOVE_TIME: f32 = 0.1;
/// `ClimbHop.forceMoveXTimer`: the hop ignores directional input while pricey.
const CLIMB_HOP_NO_MOVE_TIME: f32 = 0.2;
const CLIMB_HOP_FORCE_TIME: f32 = 0.15;
const CLIMB_CHECK_DIST: f32 = 2.0;

// Dashing.
const DASH_SPEED: f32 = 240.0;
const END_DASH_SPEED: f32 = 160.0;
const END_DASH_UP_MULT: f32 = 0.75;
const DASH_TIME: f32 = 0.15;
const DASH_COOLDOWN: f32 = 0.2;
const DASH_REFILL_COOLDOWN: f32 = 0.1;
const DASH_ATTACK_TIME: f32 = 0.3;
const DASH_FLATTEN_MULT: f32 = 1.2;
const SUPER_JUMP_H: f32 = 260.0;
const SUPER_JUMP_Y: f32 = -105.0;
const DUCK_SUPER_X_MULT: f32 = 1.25;
const DUCK_SUPER_Y_MULT: f32 = 0.5;

// Hitboxes (`Player.cs`): size and offset relative to the foot-center anchor.
const NORMAL_HITBOX: (f32, f32, f32, f32) = (8.0, 11.0, -4.0, -11.0);
const DUCK_HITBOX: (f32, f32, f32, f32) = (8.0, 6.0, -4.0, -6.0);

/// State-machine values mirroring `Player.cs`.
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
    dashes: i32,
    stamina: f32,
    // Timers.
    jump_grace: f32,
    var_jump_timer: f32,
    var_jump_speed: f32,
    dash_timer: f32,
    dash_cooldown: f32,
    dash_refill_cooldown: f32,
    dash_attack_timer: f32,
    wall_slide_timer: f32,
    wall_slide_dir: i32,
    wall_boost_timer: f32,
    climb_no_move: f32,
    low_friction_stop: f32,
    last_climb_move: i32,
    // Climb-hop intent (`hopWaitX`/`hopWaitXSpeed`): after a `ClimbHop` we wait
    // for the wall ahead to clear, then slide sideways onto the ledge.
    hop_wait_x: i32,
    hop_wait_x_speed: f32,
    // `forceMoveX`: the hop ignores directional input for a moment so it slides
    // forward on its own momentum.
    force_move_x: f32,
    force_move_timer: f32,
    // Dash intent.
    dash_dir: Vec2,
    before_dash_speed: Vec2,
    launched: bool,
    // Current state machine value.
    state: u32,
    /// Last derived state-machine value, used to log transitions in debug mode.
    debug_state: u32,
}

impl Default for PlayerState {
    fn default() -> PlayerState {
        PlayerState {
            facing: 1,
            ducking: false,
            dashes: 1,
            stamina: CLIMB_MAX_STAMINA,
            jump_grace: 0.0,
            var_jump_timer: 0.0,
            var_jump_speed: 0.0,
            dash_timer: 0.0,
            dash_cooldown: 0.0,
            dash_refill_cooldown: 0.0,
            dash_attack_timer: 0.0,
            wall_slide_timer: 0.0,
            wall_slide_dir: 0,
            wall_boost_timer: 0.0,
            climb_no_move: 0.0,
            low_friction_stop: 0.0,
            last_climb_move: 0,
            hop_wait_x: 0,
            hop_wait_x_speed: 0.0,
            force_move_x: 0.0,
            force_move_timer: 0.0,
            dash_dir: Vec2::ZERO,
            before_dash_speed: Vec2::ZERO,
            launched: false,
            state: ST_NORMAL,
            debug_state: ST_NORMAL,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<PlayerState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    handle_events(id);
    let entity = Entity::new(id);
    let mut st = state(id);
    let mut speed = entity.speed.get();
    let grounded = entity.collision.is_grounded();

    // On the ground dash refills, the cooldown clears, and the coyote timer is
    // kept topped up (the original refills dashes on landing).
    if grounded {
        st.dashes = 1;
        st.dash_cooldown = 0.0;
        st.jump_grace = JUMP_GRACE_TIME;
    }

    // Common per-frame timers (`Player.Update`).
    st.jump_grace -= dt;
    st.var_jump_timer -= dt;
    st.dash_cooldown -= dt;
    st.dash_refill_cooldown -= dt;
    if st.dash_attack_timer > 0.0 {
        st.dash_attack_timer -= dt;
    }
    st.wall_boost_timer -= dt;
    st.low_friction_stop -= dt;

    let mut move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
    let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
    // `forceMoveX` overrides directional input (ClimbHop keeps sliding on its
    // own even if the player lets go or presses the other way).
    if st.force_move_timer > 0.0 {
        st.force_move_timer -= dt;
        move_x = st.force_move_x;
    }
    if move_x != 0.0 && st.state != ST_DASH {
        st.facing = if move_x > 0.0 { 1 } else { -1 };
    }

    st.state = match st.state {
        ST_NORMAL => normal_update(&entity, &mut st, &mut speed, dt, move_x, move_y, grounded),
        ST_CLIMB => climb_update(&entity, &mut st, &mut speed, dt, move_x, move_y, grounded),
        ST_DASH => dash_update(&entity, &mut st, &mut speed, dt),
        _ => ST_NORMAL,
    };

    // `hopWaitX` (Player.Update): a `ClimbHop` arming a ledge slide fires it
    // once the wall ahead clears; any push back or downward start cancels it.
    if st.hop_wait_x != 0 {
        if speed.x.signum() as i32 == -st.hop_wait_x || speed.y > 0.0 {
            st.hop_wait_x = 0;
        } else if !entity.collision.check(st.hop_wait_x as f32, 0.0) {
            st.low_friction_stop = CLIMB_HOP_FORCE_TIME;
            speed.x = st.hop_wait_x_speed;
            st.hop_wait_x = 0;
        }
    }

    // `StDash` moves inside `dash_update`; the other states get the shared
    // `Actor.MoveH/MoveV` pass here.
    if st.state != ST_DASH {
        move_and_collide(&entity, &mut st, &mut speed, dt);
    }
    entity.speed.set(speed);

    debug_state_log(id, &st);
    STATES.with(|s| {
        s.borrow_mut().insert(id, st);
    });
}

#[unsafe(no_mangle)]
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

/// StNormal: run, jump, duck, wall slide, grab into climb.
///
/// Mirrors `Player.NormalUpdate`; water / gliders / holdables are omitted.
#[allow(clippy::too_many_arguments)]
fn normal_update(
    entity: &Entity,
    st: &mut PlayerState,
    speed: &mut Vec2,
    dt: f32,
    move_x: f32,
    move_y: f32,
    grounded: bool,
) -> u32 {
    // Grab toward the wall while falling at/after peak → climb. Checks the
    // wall at the current height and 1–2 px above ("corner pull-up").
    if Input::button(input::CLIMB)
        && !st.ducking
        && speed.y >= 0.0
        && speed.x.signum() as i32 != -st.facing
    {
        if climb_check(entity, st.facing, 0.0) {
            st.ducking = false;
            climb_begin(entity, st, speed);
            return ST_CLIMB;
        }
        for i in 1..=2 {
            let dy = -i as f32;
            if !entity.collision.check(0.0, dy) && climb_check(entity, st.facing, dy) {
                let _ = entity.collision.actor_move(0.0, dy);
                climb_begin(entity, st, speed);
                return ST_CLIMB;
            }
        }
    }

    // Dash. Wall dash / red dash / dream dash entries are engine features not
    // present yet, so only the plain `StartDash` path exists.
    if Input::pressed(input::DASH) && can_dash(*st) {
        start_dash(entity, st, speed, move_x, move_y);
        return ST_DASH;
    }

    // Ducking toggle (squash "pressed while airborne" cases are cosmetic).
    let want_duck = grounded && move_y > 0.0 && speed.y >= 0.0;
    if want_duck && !st.ducking {
        set_ducking(entity, st, true);
    } else if !want_duck && st.ducking {
        if can_unduck(entity, st.ducking) {
            set_ducking(entity, st, false);
        } else if speed.x == 0.0 {
            // Duck-correct: slide sideways until the headroom clears.
            let mut slid = false;
            for n in 1..=4 {
                let n = n as f32;
                if can_unduck_at(entity, n) {
                    let _ = entity.collision.actor_move(50.0 * dt, 0.0);
                    slid = true;
                    break;
                }
                if can_unduck_at(entity, -n) {
                    let _ = entity.collision.actor_move(-50.0 * dt, 0.0);
                    slid = true;
                    break;
                }
            }
            let _ = slid;
        }
    }

    // Horizontal movement: duck friction, or run accel/reduce (ground vs air).
    if st.ducking && grounded {
        speed.x = approach(speed.x, 0.0, DUCK_FRICTION * dt);
    } else {
        let mult = if grounded { 1.0 } else { AIR_MULT } * slow_stop_mult(st);
        let (accel, target) =
            if speed.x.abs() > MAX_RUN && move_x != 0.0 && speed.x.signum() == move_x.signum() {
                (RUN_REDUCE * mult, MAX_RUN * move_x)
            } else {
                (RUN_ACCEL * mult, MAX_RUN * move_x)
            };
        speed.x = approach(speed.x, target, accel * dt);
    }

    // Max fall speed ramps up when fast-falling (`Input.MoveY == 1`).
    let mut max_fall = MAX_FALL;
    if move_y > 0.0 && speed.y >= MAX_FALL {
        max_fall = approach(max_fall, FAST_MAX_FALL, FAST_MAX_ACCEL * dt);
    }

    // Gravity, with optional wall slide.
    if !grounded {
        let mut target = max_fall;
        let wall_left = entity.collision.check(-WALL_CHECK_DIST, 0.0);
        let wall_right = entity.collision.check(WALL_CHECK_DIST, 0.0);
        let toward_wall = (move_x < 0.0 && wall_left) || (move_x > 0.0 && wall_right);
        let grab_wall = (move_x == 0.0 && Input::button(input::CLIMB)) && (wall_left || wall_right);
        if speed.y >= 0.0 && (toward_wall || grab_wall) && can_unduck(entity, st.ducking) {
            let dir = if wall_right { 1 } else { -1 };
            st.wall_slide_dir = dir;
            st.wall_slide_timer = WALL_SLIDE_TIME;
            target = MAX_FALL
                + (WALL_SLIDE_START_MAX - MAX_FALL) * (st.wall_slide_timer / WALL_SLIDE_TIME);
            // Grab during a slide transitions straight into a climb.
            if Input::button(input::CLIMB) {
                climb_begin(entity, st, speed);
                return ST_CLIMB;
            }
        } else {
            st.wall_slide_dir = 0;
        }
        let half_grav = Input::button(input::JUMP) && speed.y.abs() < HALF_GRAV_THRESHOLD;
        let grav = GRAVITY * if half_grav { 0.5 } else { 1.0 };
        speed.y = approach(speed.y, target, grav * dt);
    } else {
        st.wall_slide_dir = 0;
    }

    // Variable jump: lift cancel while the button is held past release window.
    if st.var_jump_timer > 0.0 {
        if Input::button(input::JUMP) {
            speed.y = speed.y.min(st.var_jump_speed);
        } else {
            st.var_jump_timer = 0.0;
        }
    }

    // Jumping.
    if Input::pressed(input::JUMP) {
        if st.jump_grace > 0.0 {
            jump(entity, st, speed, move_x);
            Input::consume(input::JUMP);
        } else if !st.ducking {
            if wall_jump_check(entity, 1) {
                if super_wall_jump_angle(*st) {
                    super_wall_jump(entity, st, speed, -1);
                } else {
                    wall_jump(entity, st, speed, -1);
                }
                Input::consume(input::JUMP);
                return ST_NORMAL;
            }
            if wall_jump_check(entity, -1) {
                if super_wall_jump_angle(*st) {
                    super_wall_jump(entity, st, speed, 1);
                } else {
                    wall_jump(entity, st, speed, 1);
                }
                Input::consume(input::JUMP);
                return ST_NORMAL;
            }
        }
        // Water / jump-through-with-water omitted (no liquid subsystem).
    }

    ST_NORMAL
}

/// StClimb: grab and climb walls with stamina.
///
/// Mirrors `Player.ClimbUpdate`; wall boosters, ledges and sweat sprites are
/// not present.
#[allow(clippy::too_many_arguments)]
fn climb_update(
    entity: &Entity,
    st: &mut PlayerState,
    speed: &mut Vec2,
    dt: f32,
    move_x: f32,
    move_y: f32,
    grounded: bool,
) -> u32 {
    st.climb_no_move -= dt;
    if grounded {
        st.stamina = CLIMB_MAX_STAMINA;
    }

    // Jump off: straight away if holding away, otherwise a climb jump.
    if Input::pressed(input::JUMP) && (!st.ducking || can_unduck(entity, st.ducking)) {
        if move_x == -(st.facing as f32) {
            wall_jump(entity, st, speed, -st.facing);
        } else {
            climb_jump(entity, st, speed, move_x);
        }
        Input::consume(input::JUMP);
        return ST_NORMAL;
    }

    // Dash straight out of a climb.
    if Input::pressed(input::DASH) && can_dash(*st) {
        start_dash(entity, st, speed, move_x, move_y);
        return ST_DASH;
    }

    // Let go (vertical speed carries into freefall).
    if !Input::button(input::CLIMB) {
        return ST_NORMAL;
    }

    // The wall vanished at the top → hop up onto the ledge.
    if !entity.collision.check(st.facing as f32, 0.0) {
        if speed.y < 0.0 {
            climb_hop(entity, st, speed);
        }
        return ST_NORMAL;
    }

    // Climb movement: up / down / neutral, with a forced slip when the wall
    // is too short to grip (approximation of `SlipCheck`).
    let mut target = 0.0;
    let mut slip = false;
    if st.climb_no_move <= 0.0 {
        if move_y < 0.0 {
            target = CLIMB_UP_SPEED;
            if blocked_climb_up(entity) {
                if speed.y < 0.0 {
                    speed.y = 0.0;
                }
                target = 0.0;
            } else if slip_check(entity, st.facing) {
                // Short wall above: hop over instead of slipping down.
                climb_hop(entity, st, speed);
                return ST_NORMAL;
            }
        } else if move_y > 0.0 {
            target = CLIMB_DOWN_SPEED;
            if grounded {
                if speed.y > 0.0 {
                    speed.y = 0.0;
                }
                target = 0.0;
            }
        } else if slip_check(entity, st.facing) {
            // Neutral: grip requires a wall tall enough for the whole body.
            target = CLIMB_SLIP_SPEED;
            slip = true;
        }
    } else {
        slip = true;
        target = CLIMB_SLIP_SPEED;
    }
    st.last_climb_move = match target {
        t if t < 0.0 => -1,
        t if t > 0.0 => 1,
        _ => 0,
    };
    // While slipping we still slide down at the slip speed.
    if slip && target >= 0.0 {
        target = CLIMB_SLIP_SPEED;
    }
    if target != 0.0 {
        speed.y = approach(speed.y, target, CLIMB_ACCEL * dt);
    } else {
        speed.y = approach(speed.y, 0.0, CLIMB_ACCEL * dt);
    }

    // Don't slide down past the bottom of the wall while neutral.
    if move_y <= 0.0 && speed.y > 0.0 && !entity.collision.check(st.facing as f32, 1.0) {
        speed.y = 0.0;
    }

    // Stamina drain (up and still costs; climb jump costs are charged on jump).
    if st.climb_no_move <= 0.0 {
        let cost = if st.last_climb_move < 0 {
            CLIMB_UP_COST
        } else if st.last_climb_move == 0 && !grounded {
            CLIMB_STILL_COST
        } else {
            0.0
        };
        st.stamina -= cost * dt;
    }
    if st.stamina <= 0.0 {
        return ST_NORMAL;
    }

    ST_CLIMB
}

/// StDash: dash state update. The physics move and dash-end handling live here
/// because a wall/ceiling collision (or the timer running out) ends the dash.
fn dash_update(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dt: f32) -> u32 {
    let d = st.dash_dir;

    // Hyper / wavedash: near-horizontal dash with coyote while grounded jumps
    // into a super jump. The coyote timer only exists once the dash grounded.
    if d.y.abs() < 0.1
        && Input::pressed(input::JUMP)
        && st.jump_grace > 0.0
        && can_unduck(entity, st.ducking)
    {
        super_jump(entity, st, speed);
        Input::consume(input::JUMP);
        return ST_NORMAL;
    }

    // Super wall jump out of an upward dash.
    if super_wall_jump_angle(*st) && Input::pressed(input::JUMP) && can_unduck(entity, st.ducking) {
        if wall_jump_check(entity, 1) {
            super_wall_jump(entity, st, speed, -1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
        if wall_jump_check(entity, -1) {
            super_wall_jump(entity, st, speed, 1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
    }

    // Regular wall jump out of a dash (holding grab climbs instead).
    if Input::pressed(input::JUMP) && can_unduck(entity, st.ducking) {
        if wall_jump_check(entity, 1) {
            wall_jump(entity, st, speed, -1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
        if wall_jump_check(entity, -1) {
            wall_jump(entity, st, speed, 1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
    }

    // Movement + collision; the flatten and end rules match the DashCoroutine.
    let result = entity.collision.actor_move(speed.x * dt, speed.y * dt);
    let crushed = dash_hits_crushblock(entity);

    if result.on_ground && d.x != 0.0 && d.y > 0.0 && speed.y > 0.0 {
        // Diagonal-down dash reaching the ground flattens into a 1.2x dash and
        // ducks — the setup for a hyperdash.
        st.dash_dir = Vec2::new(d.x.signum(), 0.0);
        *speed = Vec2::new(speed.x * DASH_FLATTEN_MULT, 0.0);
        if !st.ducking {
            set_ducking(entity, st, true);
        }
        st.jump_grace = JUMP_GRACE_TIME;
    } else if result.on_ground {
        st.jump_grace = JUMP_GRACE_TIME;
    }

    if st.dash_timer <= 0.0
        || result.hit_wall_left
        || result.hit_wall_right
        || result.hit_ceiling
        || crushed
    {
        // Dash end: forward/up tapering mirrors `DashCoroutine`'s tail.
        st.dash_timer = 0.0;
        st.dash_cooldown = DASH_COOLDOWN;
        if result.hit_wall_left || result.hit_wall_right || result.hit_ceiling {
            // Reflect the tail speed off the wall/ceiling.
            let dx = if result.hit_wall_left {
                -1.0
            } else if result.hit_wall_right {
                1.0
            } else {
                d.x
            };
            let dy = if result.hit_ceiling { 1.0 } else { d.y };
            let n = (dx * dx + dy * dy).sqrt().max(1.0);
            *speed = Vec2::new(dx / n * END_DASH_SPEED, dy / n * END_DASH_SPEED);
        } else if d.y <= 0.0 {
            *speed = Vec2::new(d.x * END_DASH_SPEED, d.y * END_DASH_SPEED);
            if speed.y < 0.0 {
                speed.y *= END_DASH_UP_MULT;
            }
        }
        return ST_NORMAL;
    }

    st.dash_timer -= dt;

    ST_DASH
}

/// The shared physics pass for the non-dash states: horizontal then vertical
/// `Actor.MoveH/MoveV`, mirroring `Player.cs`'s per-frame movement.
fn move_and_collide(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dt: f32) {
    let result_h = entity.collision.actor_move(speed.x * dt, 0.0);
    if (result_h.hit_wall_left && speed.x < 0.0) || (result_h.hit_wall_right && speed.x > 0.0) {
        speed.x = 0.0;
    }

    let result_v = entity.collision.actor_move(0.0, speed.y * dt);
    if result_v.on_ground && speed.y > 0.0 {
        speed.y = 0.0;
    }
    if (result_v.hit_wall_left && speed.x < 0.0) || (result_v.hit_wall_right && speed.x > 0.0) {
        speed.x = 0.0;
    }
    if result_v.hit_ceiling && speed.y < 0.0 {
        speed.y = 0.0;
        st.var_jump_timer = 0.0;
    }
    if result_v.on_ground {
        st.jump_grace = JUMP_GRACE_TIME;
    }
}

// ---------------------------------------------------------------------------
// State transitions
// ---------------------------------------------------------------------------

/// `ClimbBegin`: stop horizontal motion, damp vertical, arm the slide timer.
///
/// The grab can trigger up to `CLIMB_CHECK_DIST` px before the wall face, so
/// nudge flush against it: every climb-related check below probes `facing * 2`
/// px ahead and would otherwise miss a wall we are 1–2 px short of.
fn climb_begin(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2) {
    st.stamina = CLIMB_MAX_STAMINA;
    speed.x = 0.0;
    speed.y *= CLIMB_GRAB_Y_MULT;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.climb_no_move = CLIMB_NO_MOVE_TIME;
    st.last_climb_move = 0;
    st.wall_boost_timer = 0.0;
    // Snap toward the wall until the collision blocks — lands flush (gap ~0),
    // matching the resting pose the original reaches by running into the wall.
    let _ = entity
        .collision
        .actor_move(st.facing as f32 * (CLIMB_CHECK_DIST + 1.0), 0.0);
}

/// `ClimbHop`: the wall ended below the player while climbing up, so hop onto
/// the ledge and snap the horizontal speed away from the wall.
/// `ClimbHop` (`Player.ClimbHop`): hop over the top of a wall.
///
/// If the wall ahead is still present at this height, keep clinging and arm the
/// ledge slide (`hopWaitX`) so the hop fires once the top clears — the "automatic
/// climb up a short wall" the original gets from the same pair of fields.
/// Otherwise the wall already ended: hop straight out sideways.
fn climb_hop(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2) {
    st.low_friction_stop = CLIMB_HOP_FORCE_TIME;
    if entity.collision.check(st.facing as f32, 0.0) {
        st.hop_wait_x = st.facing;
        st.hop_wait_x_speed = st.facing as f32 * CLIMB_HOP_X;
    } else {
        st.hop_wait_x = 0;
        speed.x = st.facing as f32 * CLIMB_HOP_X;
    }
    speed.y = speed.y.min(CLIMB_HOP_Y);
    st.force_move_x = 0.0;
    st.force_move_timer = CLIMB_HOP_NO_MOVE_TIME;
}

fn climb_jump(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, move_x: f32) {
    if !entity.collision.is_grounded() {
        st.stamina -= CLIMB_JUMP_COST;
    }
    if move_x == 0.0 {
        // Arms wobble slightly off the wall when jumping straight up.
        speed.x += -(st.facing as f32) * 20.0;
    }
    jump(entity, st, speed, move_x);
}

/// `Jump`: consume buffer (caller), grace/variable timers, `Speed.X += 40*moveX`.
fn jump(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, move_x: f32) {
    st.jump_grace = 0.0;
    st.var_jump_timer = JUMP_VAR_JUMP_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.wall_slide_dir = 0;
    st.wall_boost_timer = 0.0;
    st.launched = false;
    speed.x += JUMP_HBOOST * move_x;
    speed.y = JUMP_SPEED;
    st.var_jump_speed = speed.y;
    let _ = entity;
    ruleste_plugin_api::host::emit(
        entity.id,
        ruleste_plugin_api::plugin::event::PLAYER_JUMP,
        &[],
    );
}

/// `WallJump(dir)`: `Speed.X = 130*dir; Speed.Y = -105`.
fn wall_jump(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dir: i32) {
    set_ducking(entity, st, false);
    st.jump_grace = 0.0;
    st.var_jump_timer = VAR_JUMP_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.wall_slide_dir = 0;
    st.wall_boost_timer = 0.0;
    st.low_friction_stop = WALL_JUMP_FORCE_TIME;
    st.launched = false;
    speed.x = WALL_JUMP_HSPEED * dir as f32;
    speed.y = WALL_JUMP_SPEED;
    st.var_jump_speed = speed.y;
    ruleste_plugin_api::host::emit(
        entity.id,
        ruleste_plugin_api::plugin::event::PLAYER_JUMP,
        &[],
    );
}

/// `SuperWallJump(dir)`: used when dashing mostly straight up into a wall.
fn super_wall_jump(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dir: i32) {
    set_ducking(entity, st, false);
    st.jump_grace = 0.0;
    st.var_jump_timer = SUPER_WALL_JUMP_VAR_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.wall_slide_dir = 0;
    st.wall_boost_timer = 0.0;
    st.launched = true;
    speed.x = SUPER_WALL_JUMP_HSPEED * dir as f32;
    speed.y = SUPER_WALL_JUMP_SPEED;
    st.var_jump_speed = speed.y;
    ruleste_plugin_api::host::emit(
        entity.id,
        ruleste_plugin_api::plugin::event::PLAYER_JUMP,
        &[],
    );
}

/// `SuperJump`: ground dash → 260/(ducked 325) jump, the hyper base.
fn super_jump(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2) {
    st.jump_grace = 0.0;
    st.var_jump_timer = JUMP_VAR_JUMP_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.wall_slide_dir = 0;
    st.wall_boost_timer = 0.0;
    st.dash_attack_timer = 0.0;
    st.launched = true;
    speed.x = SUPER_JUMP_H * st.facing as f32;
    speed.y = SUPER_JUMP_Y;
    if st.ducking {
        set_ducking(entity, st, false);
        speed.x *= DUCK_SUPER_X_MULT;
        speed.y *= DUCK_SUPER_Y_MULT;
    }
    st.var_jump_speed = speed.y;
    ruleste_plugin_api::host::emit(
        entity.id,
        ruleste_plugin_api::plugin::event::PLAYER_JUMP,
        &[],
    );
}

/// `StartDash`: consumes a dash and arms the dash state. `GetAimVector` uses
/// the held axis, falling back to the facing direction.
fn start_dash(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, move_x: f32, move_y: f32) {
    st.dashes = (st.dashes - 1).max(0);
    st.launched = false;
    st.dash_timer = DASH_TIME;
    st.dash_cooldown = DASH_COOLDOWN;
    st.dash_refill_cooldown = DASH_REFILL_COOLDOWN;
    st.dash_attack_timer = DASH_ATTACK_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    st.wall_slide_dir = 0;
    let dir = if move_x != 0.0 || move_y != 0.0 {
        normalize(move_x, move_y)
    } else {
        Vec2::new(st.facing as f32, 0.0)
    };
    st.before_dash_speed = *speed;
    st.dash_dir = dir;
    if dir.x != 0.0 {
        st.facing = if dir.x > 0.0 { 1 } else { -1 };
    }
    // `DashBegin`: airborne duck resumes standing; holding down ducks.
    if !entity.collision.is_grounded() && st.ducking && can_unduck(entity, st.ducking) {
        set_ducking(entity, st, false);
    } else if !st.ducking && move_y > 0.0 {
        set_ducking(entity, st, true);
    }
    // `DashCoroutine` first step: `Speed = DashDir * 240`, preserving a faster
    // pre-dash horizontal run when same-sign.
    *speed = Vec2::new(
        dash_axis(dir.x, st.before_dash_speed.x),
        dash_axis(dir.y, st.before_dash_speed.y),
    );
    Input::consume(input::DASH);
    ruleste_plugin_api::host::emit(
        entity.id,
        ruleste_plugin_api::plugin::event::PLAYER_DASH,
        &[],
    );
}

/// `DashCoroutine` momentum preservation: if the pre-dash speed is same sign
/// and faster on this axis, keep the pre-dash speed.
fn dash_axis(axis: f32, before: f32) -> f32 {
    let target = axis * DASH_SPEED;
    if before.signum() == axis.signum() && before.abs() > target.abs() {
        before
    } else {
        target
    }
}

fn can_dash(st: PlayerState) -> bool {
    st.dashes > 0 && st.dash_cooldown <= 0.0
}

/// `SuperWallJumpAngleCheck`: the dash must be (mostly) straight up.
fn super_wall_jump_angle(st: PlayerState) -> bool {
    if st.dash_dir.x.abs() <= 0.2 {
        return st.dash_dir.y <= -0.75;
    }
    false
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

/// `ClimbCheck(dir, addY)`: a solid within `CLIMB_CHECK_DIST` in `dir`.
fn climb_check(entity: &Entity, facing: i32, add_y: f32) -> bool {
    entity
        .collision
        .check(facing as f32 * CLIMB_CHECK_DIST, add_y)
}

/// True when the player can stand up right now: if ducking, the normal
/// hitbox's extra 5 px of headroom over the duck hitbox must be clear.
fn can_unduck(entity: &Entity, ducking: bool) -> bool {
    !ducking || !entity.collision.check(0.0, -5.0)
}

/// `CanUnDuckAt`: headroom clear at horizontal offset `x`.
fn can_unduck_at(entity: &Entity, x: f32) -> bool {
    !entity.collision.check(x, -5.0)
}

/// `WallJumpCheck(dir)`: a solid within `WALL_JUMP_CHECK_DIST` in `dir`.
fn wall_jump_check(entity: &Entity, dir: i32) -> bool {
    entity
        .collision
        .check(dir as f32 * WALL_JUMP_CHECK_DIST, 0.0)
}

/// Climb is blocked above the head when climbing up.
fn blocked_climb_up(entity: &Entity) -> bool {
    entity.collision.check(0.0, -1.0)
}

/// Approximation of `SlipCheck`: the wall's upper edge is lower than the
/// player, so the character slips down instead of climbing.
fn slip_check(entity: &Entity, facing: i32) -> bool {
    !entity
        .collision
        .check(facing as f32 * CLIMB_CHECK_DIST, -4.0)
}

fn slow_stop_mult(st: &PlayerState) -> f32 {
    if st.low_friction_stop > 0.0 { 0.0 } else { 1.0 }
}

fn set_ducking(entity: &Entity, st: &mut PlayerState, ducking: bool) {
    st.ducking = ducking;
    if ducking {
        entity
            .hitbox
            .set(DUCK_HITBOX.0, DUCK_HITBOX.1, DUCK_HITBOX.2, DUCK_HITBOX.3);
    } else {
        entity.hitbox.set(
            NORMAL_HITBOX.0,
            NORMAL_HITBOX.1,
            NORMAL_HITBOX.2,
            NORMAL_HITBOX.3,
        );
    }
}

/// True when the dashing player overlaps a `crushBlock` entity's hitbox.
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

/// Handles events from other plugins (refills, boosters) before movement.
fn handle_events(id: EntityId) {
    for (_, kind, data) in ruleste_plugin_api::host::drain_events() {
        with_state(id, |st| match kind {
            ruleste_plugin_api::host::EV_REFILL => {
                let two = data.first().copied().unwrap_or(0) != 0;
                st.dashes = if two { 2 } else { 1 };
                st.dash_cooldown = 0.0;
                st.dash_refill_cooldown = 0.0;
                st.stamina = CLIMB_MAX_STAMINA;
            }
            ruleste_plugin_api::host::EV_BOOST => {
                let move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
                let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
                let dir = if move_x != 0.0 || move_y != 0.0 {
                    normalize(move_x, move_y)
                } else {
                    Vec2::new(st.facing as f32, 0.0)
                };
                // `BoostBegin` (simplified): refill dashes and launch along the
                // held aim. The in-booster spiral is an engine feature that
                // the booster entity does not drive yet.
                st.dashes = 1;
                st.launched = true;
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

/// Derives a readable state name and, in debug mode, logs transitions.
fn debug_state_log(id: EntityId, st: &PlayerState) {
    if st.debug_state == st.state {
        return;
    }
    if !ruleste_plugin_api::host::debug_enabled() {
        return;
    }
    let name = match st.state {
        ST_CLIMB => "Climb",
        ST_DASH => "Dash",
        _ => "Normal",
    };
    ruleste_plugin_api::host::log(&format!("player {id} state -> {name} ({})", st.state));
}

// ---------------------------------------------------------------------------
// Hot-reload serialization
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_serialize(id: EntityId, out_len: *mut u32) -> u32 {
    let st = state(id);
    let mut buf = Vec::with_capacity(96);
    // Version 1 fields (unchanged from the earlier plugin, so old state files
    // keep their meaning).
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
    // Version 2 fields.
    push_f32(&mut buf, st.stamina);
    push_f32(&mut buf, st.dash_refill_cooldown);
    push_f32(&mut buf, st.dash_attack_timer);
    push_f32(&mut buf, st.wall_boost_timer);
    push_f32(&mut buf, st.climb_no_move);
    push_f32(&mut buf, st.low_friction_stop);
    push_i32(&mut buf, st.last_climb_move);
    push_f32(&mut buf, st.before_dash_speed.x);
    push_f32(&mut buf, st.before_dash_speed.y);
    push_u8(&mut buf, u8::from(st.launched));
    push_u32(&mut buf, st.state);
    // Version 3 fields (climb-hop intent).
    push_i32(&mut buf, st.hop_wait_x);
    push_f32(&mut buf, st.hop_wait_x_speed);
    push_f32(&mut buf, st.force_move_x);
    push_f32(&mut buf, st.force_move_timer);
    unsafe { *out_len = buf.len() as u32 }
    let ptr = buf.as_ptr() as u32;
    SER_BUF.with(|b| *b.borrow_mut() = buf);
    ptr
}

#[unsafe(no_mangle)]
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
    let mut st = PlayerState {
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
        ..PlayerState::default()
    };
    // Version 2 trailing fields (absent in old buffers → defaults).
    if r.remaining() > 40 {
        st.stamina = r.f32()?;
        st.dash_refill_cooldown = r.f32()?;
        st.dash_attack_timer = r.f32()?;
        st.wall_boost_timer = r.f32()?;
        st.climb_no_move = r.f32()?;
        st.low_friction_stop = r.f32()?;
        st.last_climb_move = r.i32()?;
        st.before_dash_speed = Vec2::new(r.f32()?, r.f32()?);
        st.launched = r.u8()? != 0;
        st.state = r.u32()?;
    }
    // Version 3 trailing fields.
    if r.remaining() >= 8 {
        st.hop_wait_x = r.i32()?;
        st.hop_wait_x_speed = r.f32()?;
    }
    if r.remaining() >= 8 {
        st.force_move_x = r.f32()?;
        st.force_move_timer = r.f32()?;
    }
    Some(st)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Cursor<'a> {
        Cursor { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
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
