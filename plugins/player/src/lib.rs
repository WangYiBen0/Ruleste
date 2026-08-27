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

use ruleste_plugins_api::host::Input;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::ruleste_entity_types;
use ruleste_plugins_api::ruleste_meta;
use ruleste_plugins_api::ruleste_noop_destroy;
use ruleste_plugins_api::types::input;
use ruleste_plugins_api::types::{EntityId, Vec2};

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
/// `RedDashCoroutine` launch speed: the sustained red-boost dash.
const RED_DASH_SPEED: f32 = 240.0;
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

// Booster (`Booster.cs` -> `Player.BoostCoroutine`).
const BOOST_TIME: f32 = 0.25;
const BOOST_APPROACH_SPEED: f32 = 80.0;

// Bumper (`Player.ExplodeLaunch`) and BadelineBoost launches (`StLaunch`).
const EXPLODE_LAUNCH_SPEED: f32 = 280.0;
const LAUNCH_FALL_TARGET: f32 = 160.0;
const LAUNCH_Y_ACCEL_RISE: f32 = 450.0;
const LAUNCH_Y_ACCEL_FALL: f32 = 225.0;
const LAUNCH_X_ACCEL: f32 = 200.0;
const LAUNCH_RETURN_SPEED: f32 = 220.0;
const BADELINE_LAUNCH_SPEED: f32 = -330.0;
const BADELINE_APPROACH_X: f32 = 60.0;

// Feather flight (`Player.StStarFly`, mirroring `FlyFeather.cs`).
const STARFLY_TIME: f32 = 2.0;
const STARFLY_BOOST_SPEED: f32 = 250.0;
const STARFLY_TURN_SPEED: f32 = 5.5850534;
const STARFLY_TARGET_NO_INPUT: f32 = 91.0;
const STARFLY_TARGET_SLOW: f32 = 140.0;
const STARFLY_TARGET_FAST: f32 = 190.0;
const STARFLY_SPEED_LERP_TIME: f32 = 1.0;
const STARFLY_SPEED_ACCEL: f32 = 1000.0;
const STARFLY_TRANSFORM_DECEL: f32 = 1000.0;

/// Maximum number of dashes. `MaxDashes` in the original is 1 for Madeline.
const MAX_DASHES: i32 = 1;
/// `Refill.cs` stamina threshold below which a refill is still useful.
const REFILL_STAMINA_MIN: f32 = 20.0;

// Hitboxes (`Player.cs`): size and offset relative to the foot-center anchor.
const NORMAL_HITBOX: (f32, f32, f32, f32) = (8.0, 11.0, -4.0, -11.0);
const DUCK_HITBOX: (f32, f32, f32, f32) = (8.0, 6.0, -4.0, -6.0);

/// State-machine values mirroring `Player.cs`.
const ST_NORMAL: u32 = 0;
const ST_CLIMB: u32 = 1;
const ST_DASH: u32 = 2;
const ST_BOOST: u32 = 4;
const ST_RED_DASH: u32 = 5;
const ST_LAUNCH: u32 = 7;
const ST_SUMMIT_LAUNCH: u32 = 10;
const ST_STARFLY: u32 = 19;

// Spring side bounce (`Player.SideBounce`): `SideBounceSpeed`/`ForceMoveXTime`.
const SIDE_BOUNCE_SPEED: f32 = 240.0;
const SIDE_BOUNCE_FORCE_TIME: f32 = 0.3;
const SUPER_BOUNCE_SPEED: f32 = -185.0;

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
    // Booster (`StBoost`): pull toward the booster center until the timer runs
    // out, then the stored aim fires a dash. `boost_red` marks a red booster:
    // its exit launch is a sustained `StRedDash` instead of a normal dash.
    boost_target: Vec2,
    boost_timer: f32,
    boost_red: bool,
    // Launch (`StLaunch`): optional horizontal approach target while airborne.
    launch_approach_x: f32,
    has_launch_approach: bool,
    // Feather flight (`StStarFly`).
    starfly_timer: f32,
    starfly_transforming: bool,
    starfly_speed_lerp: f32,
    starfly_last_dir: Vec2,
    // Carried by another plugin (BadelineBoost grab): frozen in place with the
    // carrier driving the position each frame.
    carried: bool,
    carry_target: Vec2,
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
            boost_target: Vec2::ZERO,
            boost_timer: 0.0,
            boost_red: false,
            launch_approach_x: 0.0,
            has_launch_approach: false,
            starfly_timer: 0.0,
            starfly_transforming: false,
            starfly_speed_lerp: 0.0,
            starfly_last_dir: Vec2::ZERO,
            carried: false,
            carry_target: Vec2::ZERO,
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

    // On the ground dashes refill — but only once `dashRefillCooldownTimer`
    // expires, otherwise a dash landing instantly recharges. `wallSlideTimer`
    // likewise resets on the ground (`Player.Update`, `state != 1`).
    if grounded && st.dash_refill_cooldown <= 0.0 {
        st.dashes = MAX_DASHES;
        st.jump_grace = JUMP_GRACE_TIME;
    }
    if grounded {
        st.wall_slide_timer = WALL_SLIDE_TIME;
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

    // Carried: the carrier owns the position; everything else is frozen.
    if st.carried {
        entity.position.set_xy(st.carry_target.x, st.carry_target.y);
        entity.speed.set(Vec2::ZERO);
        publish_resources(id, &st);
        STATES.with(|s| {
            s.borrow_mut().insert(id, st);
        });
        return;
    }

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
        ST_BOOST => boost_update(&entity, &mut st, &mut speed, dt),
        ST_RED_DASH => red_dash_update(&entity, &mut st, &mut speed, dt),
        ST_LAUNCH => launch_update(&entity, &mut st, &mut speed, dt),
        ST_SUMMIT_LAUNCH => summit_launch_update(&entity, &mut st, &mut speed, dt),
        ST_STARFLY => starfly_update(&entity, &mut st, &mut speed, dt, move_x, move_y, grounded),
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

    // `StDash`/`StBoost`/`StRedDash`/`StStarFly` move inside their own updates;
    // the other states get the shared `Actor.MoveH/MoveV` pass here.
    if st.state != ST_DASH
        && st.state != ST_BOOST
        && st.state != ST_RED_DASH
        && st.state != ST_STARFLY
    {
        move_and_collide(&entity, &mut st, &mut speed, dt);
    }
    entity.speed.set(speed);
    publish_resources(id, &st);

    debug_state_log(id, &st);
    STATES.with(|s| {
        s.borrow_mut().insert(id, st);
    });
}

/// Publishes the player's dash count and stamina to the host so refills,
/// springs and other plugins can read them back via the FFI.
fn publish_resources(id: EntityId, st: &PlayerState) {
    ruleste_plugins_api::host::set_player_dashes(id, st.dashes);
    ruleste_plugins_api::host::set_player_stamina(id, st.stamina);
    ruleste_plugins_api::host::set_player_state(id, st.state);
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
        if speed.y >= 0.0
            && (toward_wall || grab_wall)
            && st.wall_slide_timer > 0.0
            && can_unduck(entity, st.ducking)
        {
            let dir = if wall_right { 1 } else { -1 };
            st.wall_slide_dir = dir;
            // `wallSlideTimer` decays only while actually sliding
            // (`Player.Update`: `wallSlideDir != 0`), so the slide ramps
            // 20 → 160 px/s as the timer runs out and ends at 0.
            st.wall_slide_timer = (st.wall_slide_timer - dt).max(0.0);
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

    // Dash end: the coroutine's `yield return 0.15f` is the only thing that
    // ends the dash (`DashCoroutine`); a wall/ceiling hit zeroes that axis of
    // speed (like `OnCollideH`/`OnCollideV`) but the dash state persists until
    // the timer runs out. Colliding with a crush/dash block fires their events.
    if result.hit_wall_left || result.hit_wall_right || result.hit_ceiling {
        let hit_axis = if result.hit_wall_left {
            Some(Vec2::new(-1.0, 0.0))
        } else if result.hit_wall_right {
            Some(Vec2::new(1.0, 0.0))
        } else if result.hit_ceiling && d.y < 0.0 {
            // Vertical crush blocks activate from an upward dash into their
            // underside (`OnDashed` gets the dash direction).
            Some(Vec2::new(0.0, -1.0))
        } else if result.on_ground && d.y > 0.0 {
            // `CrushBlock.OnDashed` also fires from a downward dash landing
            // on the block's top — the original fires through `OnCollideV`
            // when `Direction.Y == Math.Sign(DashDir.Y)`.
            Some(Vec2::new(0.0, 1.0))
        } else {
            None
        };
        if let Some(hit_vec) = hit_axis {
            if dash_hits_entity_type(entity, "crushBlock", hit_vec) {
                emit_crush_dir(entity.id, hit_vec);
            }
            if dash_hits_entity_type(entity, "dashBlock", hit_vec) {
                // `OnDashCollide` → `DashBlock.OnDashed`: the block breaks and
                // the player rebounds. The payload carries the dash direction
                // and the state at dash time, so the block can still apply the
                // `canDash` gate (`OnDashed` checks `player.StateMachine.State`).
                emit_dash_block(entity.id, hit_vec, st.state);
            }
        }
        if result.hit_wall_left || result.hit_wall_right {
            speed.x = 0.0;
        }
        if result.hit_ceiling {
            speed.y = 0.0;
            st.var_jump_timer = 0.0;
        }
    }

    st.dash_timer -= dt;
    if st.dash_timer <= 0.0 {
        // `DashCoroutine` end: `Speed = DashDir * 160`, up-dashes get a 0.75x
        // climb, and the state returns to normal on its own.
        *speed = Vec2::new(d.x * END_DASH_SPEED, d.y * END_DASH_SPEED);
        if speed.y < 0.0 {
            speed.y *= END_DASH_UP_MULT;
        }
        return ST_NORMAL;
    }

    ST_DASH
}

/// Emits `EV_CRUSH` toward crush blocks, the payload the dash direction
/// (`±1` on the axis that hit), which the block negates into its crush axis.
fn emit_crush_dir(id: EntityId, dir: Vec2) {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&dir.x.to_le_bytes());
    buf[4..8].copy_from_slice(&dir.y.to_le_bytes());
    ruleste_plugins_api::host::emit(id, ruleste_plugins_api::host::EV_CRUSH, &buf);
}

/// Emits `EV_DASH_BLOCK` toward a dashe block: the hit face direction (`±1` on
/// the axis) plus the player state at dash time, so the block can apply the
/// `canDash` gate (`OnDashed` reads `player.StateMachine.State`).
fn emit_dash_block(id: EntityId, dir: Vec2, state: u32) {
    let mut buf = Vec::with_capacity(12);
    buf.extend_from_slice(&dir.x.to_le_bytes());
    buf.extend_from_slice(&dir.y.to_le_bytes());
    buf.push(state as u8);
    ruleste_plugins_api::host::emit(id, ruleste_plugins_api::host::EV_DASH_BLOCK, &buf);
}

// ---------------------------------------------------------------------------
// Boost / launch / star fly state updates (`StBoost`, `StLaunch`,
// `StSummitLaunch`, `StStarFly`)
// ---------------------------------------------------------------------------

/// `StBoost` (`Player.BoostUpdate`): pulled toward the booster center for
/// `BOOST_TIME`, then a dash fires in the held aim — a normal dash for green
/// boosters, a sustained red dash for `red` ones. `BoostBegin` refills.
fn boost_update(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dt: f32) -> u32 {
    let aim_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
    let aim_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
    // `BoostTarget - Collider.Center + aim * 3`, the collider center being
    // `(0, -5.5)` for the normal 8×11 hitbox.
    let p = entity.position.get();
    let target = Vec2::new(
        st.boost_target.x + aim_x * 3.0,
        st.boost_target.y + 5.5 + aim_y * 3.0,
    );
    let moved = approach_point(p, target, BOOST_APPROACH_SPEED * dt);
    entity.position.set_xy(moved.x, moved.y);

    // `BoostUpdate` fires the launch as soon as the dash button is pressed;
    // the `BOOST_TIME` timer is only the fallback (`BoostCoroutine`).
    if Input::pressed(input::DASH) {
        if st.boost_red {
            red_dash_build(st, speed, aim_x, aim_y);
            return ST_RED_DASH;
        }
        start_dash(entity, st, speed, aim_x, aim_y);
        return ST_DASH;
    }
    st.boost_timer -= dt;
    if st.boost_timer <= 0.0 {
        // `BoostCoroutine` exit: if the player hasn't already fired a dash, it
        // does here — a normal dash, or the sustained red dash for red pads.
        if st.boost_red {
            red_dash_build(st, speed, aim_x, aim_y);
            return ST_RED_DASH;
        }
        start_dash(entity, st, speed, aim_x, aim_y);
        return ST_DASH;
    }
    ST_BOOST
}

/// Builds the `StRedDash` launch: dash direction from the aimed input (facing
/// if no aim), full 240 speed, and a `RedDashBegin`-style cooldown so the
/// player can't immediately dash again.
fn red_dash_build(st: &mut PlayerState, speed: &mut Vec2, aim_x: f32, aim_y: f32) {
    let dir = if aim_x != 0.0 || aim_y != 0.0 {
        normalize(aim_x, aim_y)
    } else {
        Vec2::new(st.facing as f32, 0.0)
    };
    st.dash_dir = dir;
    if dir.x != 0.0 {
        st.facing = if dir.x > 0.0 { 1 } else { -1 };
    }
    // `RedDashBegin`: `DashDir = (Speed = Vector2.Zero)`, then the coroutine
    // sets `Speed = CorrectDashPrecision(lastAim) * 240f`.
    st.dash_cooldown = DASH_COOLDOWN;
    st.dash_refill_cooldown = DASH_REFILL_COOLDOWN;
    st.dash_attack_timer = DASH_ATTACK_TIME;
    st.wall_slide_timer = WALL_SLIDE_TIME;
    *speed = Vec2::new(dir.x * RED_DASH_SPEED, dir.y * RED_DASH_SPEED);
    Input::consume(input::DASH);
}

/// `StRedDash` (`Player.RedDashUpdate`): a sustained 240-speed dash that
/// persists until the player hits a wall/ceiling, dashes again, or jumps out
/// (`SuperWallJump`). The speed is rewritten each frame so the boost never
/// decays while airborne.
fn red_dash_update(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dt: f32) -> u32 {
    // A fresh dash input re-dashes with the held aim (`CanDash`).
    if can_dash(*st) && Input::pressed(input::DASH) {
        let mx = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
        let my = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
        start_dash(entity, st, speed, mx, my);
        return ST_DASH;
    }
    // Super/wall jumps leave the red dash the same as from a normal dash.
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

    let d = st.dash_dir;
    let result = entity.collision.actor_move(speed.x * dt, speed.y * dt);
    if result.hit_wall_left || result.hit_wall_right || result.hit_ceiling {
        let face = if result.hit_wall_left {
            Some(Vec2::new(-1.0, 0.0))
        } else if result.hit_wall_right {
            Some(Vec2::new(1.0, 0.0))
        } else if result.hit_ceiling && d.y < 0.0 {
            Some(Vec2::new(0.0, -1.0))
        } else {
            None
        };
        if let Some(face) = face {
            if dash_hits_entity_type(entity, "dashBlock", face) {
                // `StRedDash` drives through a breaking dash block (`OnCollideH`
                // forces the `OnDashCollide` result to `Ignore` for state 5);
                // `OnDashed` still breaks it.
                emit_dash_block(entity.id, face, ST_RED_DASH);
                return ST_RED_DASH;
            }
        }
        // `OnCollideH/V` → `StHitSquash`: the boost stops dead at a surface.
        st.dash_dir = Vec2::ZERO;
        *speed = Vec2::ZERO;
        return ST_NORMAL;
    }
    if result.on_ground {
        st.jump_grace = JUMP_GRACE_TIME;
    }
    // Constant red-boost speed (`RedDashUpdate` sets `Speed = DashDir*240`).
    *speed = Vec2::new(d.x * RED_DASH_SPEED, d.y * RED_DASH_SPEED);
    ST_RED_DASH
}

/// `StLaunch` (`Player.LaunchUpdate`): gravity approach and horizontal decay
/// until the speed falls under the return threshold.
fn launch_update(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, dt: f32) -> u32 {
    if st.has_launch_approach {
        let p = entity.position.get();
        let nx = approach(p.x, st.launch_approach_x, BADELINE_APPROACH_X * dt);
        entity.position.set_xy(nx, p.y);
    }
    if can_dash(*st) && Input::pressed(input::DASH) {
        let move_x = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
        let move_y = Input::axis(input::MOVE_DOWN) - Input::axis(input::MOVE_UP);
        start_dash(entity, st, speed, move_x, move_y);
        return ST_DASH;
    }
    if speed.y < 0.0 {
        speed.y = approach(speed.y, LAUNCH_FALL_TARGET, LAUNCH_Y_ACCEL_RISE * dt);
    } else {
        speed.y = approach(speed.y, LAUNCH_FALL_TARGET, LAUNCH_Y_ACCEL_FALL * dt);
    }
    speed.x = approach(speed.x, 0.0, LAUNCH_X_ACCEL * dt);
    if speed.length() < LAUNCH_RETURN_SPEED {
        st.launched = false;
        st.has_launch_approach = false;
        return ST_NORMAL;
    }
    ST_LAUNCH
}

/// `StSummitLaunch` (`Player.SummitLaunchUpdate`): straight up at 240 while
/// drifting toward the target X.
fn summit_launch_update(entity: &Entity, st: &mut PlayerState, speed: &mut Vec2, _dt: f32) -> u32 {
    st.facing = 1;
    let p = entity.position.get();
    let nx = approach(p.x, st.launch_approach_x, 20.0 * 1.0);
    entity.position.set_xy(nx, p.y);
    *speed = Vec2::new(0.0, -240.0);
    ST_SUMMIT_LAUNCH
}

/// `StStarFly` (`Player.StarFlyUpdate`, simplified): rotate the current speed
/// toward the feather input, ramp speed between 140/190, exit with a normal
/// jump or when the timer expires.
#[allow(clippy::too_many_arguments)]
fn starfly_update(
    entity: &Entity,
    st: &mut PlayerState,
    speed: &mut Vec2,
    dt: f32,
    move_x: f32,
    move_y: f32,
    grounded: bool,
) -> u32 {
    let _ = grounded;
    if st.starfly_transforming {
        // `startStarFly` morph: coast to a halt first.
        *speed = approach_vec(*speed, Vec2::ZERO, STARFLY_TRANSFORM_DECEL * dt);
        if speed.length() <= 1.0 {
            st.starfly_transforming = false;
            st.starfly_timer = STARFLY_TIME;
            // Refills happen at morph end (`StarFlyCoroutine`).
            st.dashes = MAX_DASHES;
            st.stamina = CLIMB_MAX_STAMINA;
            let dir = if move_x != 0.0 || move_y != 0.0 {
                normalize(move_x, move_y)
            } else {
                Vec2::new(st.facing as f32, 0.0)
            };
            st.starfly_last_dir = dir;
            *speed = Vec2::new(dir.x * STARFLY_BOOST_SPEED, dir.y * STARFLY_BOOST_SPEED);
        }
        return ST_STARFLY;
    }

    let aim = if move_x != 0.0 || move_y != 0.0 {
        normalize(move_x, move_y)
    } else {
        Vec2::ZERO
    };
    let no_input = aim.x == 0.0 && aim.y == 0.0;
    let cur = safe_normalize(speed);
    let dir = if cur != Vec2::ZERO {
        rotate_toward(cur, aim, STARFLY_TURN_SPEED * dt)
    } else {
        aim
    };
    st.starfly_last_dir = dir;
    let target = if no_input {
        st.starfly_speed_lerp = 0.0;
        STARFLY_TARGET_NO_INPUT
    } else if dir != Vec2::ZERO && dot(cur, aim) >= 0.45 {
        st.starfly_speed_lerp = approach(st.starfly_speed_lerp, 1.0, dt / STARFLY_SPEED_LERP_TIME);
        lerp(
            STARFLY_TARGET_SLOW,
            STARFLY_TARGET_FAST,
            st.starfly_speed_lerp,
        )
    } else {
        st.starfly_speed_lerp = 0.0;
        STARFLY_TARGET_SLOW
    };
    let val = speed.length();
    let val = approach(val, target, STARFLY_SPEED_ACCEL * dt);
    *speed = Vec2::new(dir.x * val, dir.y * val);

    // Jump cancels into a normal/wall jump; grab into a climb.
    if Input::pressed(input::JUMP) {
        if entity.collision.is_grounded() {
            jump(entity, st, speed, move_x);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
        if wall_jump_check(entity, -1) {
            wall_jump(entity, st, speed, 1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
        if wall_jump_check(entity, 1) {
            wall_jump(entity, st, speed, -1);
            Input::consume(input::JUMP);
            return ST_NORMAL;
        }
    }
    if Input::button(input::CLIMB) && !st.ducking {
        if climb_check(entity, 1, 0.0) {
            climb_begin(entity, st, speed);
            return ST_CLIMB;
        }
        if climb_check(entity, -1, 0.0) {
            climb_begin(entity, st, speed);
            return ST_CLIMB;
        }
    }
    if can_dash(*st) && Input::pressed(input::DASH) {
        start_dash(entity, st, speed, move_x, move_y);
        return ST_DASH;
    }

    // Move and bounce off walls/ceilings (`OnCollideH` starfly branch).
    let result = entity.collision.actor_move(speed.x * dt, speed.y * dt);
    if result.hit_wall_left || result.hit_wall_right {
        if st.starfly_timer < 0.2 {
            speed.x = 0.0;
        } else {
            speed.x *= -0.5;
        }
    }
    if result.hit_ceiling && speed.y < 0.0 {
        speed.y *= -0.5;
    }

    st.starfly_timer -= dt;
    if st.starfly_timer <= 0.0 {
        // `StarFlyEnd`: damp exit, clamp horizontal speed, jump input carries.
        if move_y < 0.0 {
            speed.y = -100.0;
        }
        if speed.y > 0.0 {
            speed.y = 0.0;
        }
        if speed.x.abs() > 140.0 {
            speed.x = 140.0 * speed.x.signum();
        }
        return ST_NORMAL;
    }
    ST_STARFLY
}

fn approach_vec(value: Vec2, target: Vec2, max_move: f32) -> Vec2 {
    let dx = target.x - value.x;
    let dy = target.y - value.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= max_move {
        target
    } else {
        Vec2::new(value.x + dx / len * max_move, value.y + dy / len * max_move)
    }
}

fn approach_point(value: Vec2, target: Vec2, max_move: f32) -> Vec2 {
    approach_vec(value, target, max_move)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// `Vector2.RotateTowards(target, maxRadiansDelta)`.
fn rotate_toward(from: Vec2, to: Vec2, max_delta: f32) -> Vec2 {
    if to == Vec2::ZERO {
        return from;
    }
    let from_angle = from.y.atan2(from.x);
    let to_angle = to.y.atan2(to.x);
    let mut diff = to_angle - from_angle;
    // Wrap to [-PI, PI].
    while diff > std::f32::consts::PI {
        diff -= 2.0 * std::f32::consts::PI;
    }
    while diff < -std::f32::consts::PI {
        diff += 2.0 * std::f32::consts::PI;
    }
    let angle = from_angle + diff.clamp(-max_delta, max_delta);
    Vec2::new(angle.cos(), angle.sin())
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
    ruleste_plugins_api::host::emit(
        entity.id,
        ruleste_plugins_api::plugin::event::PLAYER_JUMP,
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
    ruleste_plugins_api::host::emit(
        entity.id,
        ruleste_plugins_api::plugin::event::PLAYER_JUMP,
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
    ruleste_plugins_api::host::emit(
        entity.id,
        ruleste_plugins_api::plugin::event::PLAYER_JUMP,
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
    ruleste_plugins_api::host::emit(
        entity.id,
        ruleste_plugins_api::plugin::event::PLAYER_JUMP,
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
    ruleste_plugins_api::host::emit(
        entity.id,
        ruleste_plugins_api::plugin::event::PLAYER_DASH,
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

/// True when the dashing player occupies the hit face of a `type_name` entity:
/// the hitbox, pushed `PROBE` px toward `face`, overlaps the entity. The probe
/// matters because `actor_move` snaps the player flush against the solid, so a
/// strict overlap test can't see it (`OnCollideH` compares the face against
/// `sign(DashDir.X)` the same way).
fn dash_hits_entity_type(entity: &Entity, type_name: &str, face: Vec2) -> bool {
    const PROBE: f32 = 3.0;
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    let px = p.x + ox + face.x * PROBE;
    let py = p.y + oy + face.y * PROBE;
    for eid in ruleste_plugins_api::host::entities_by_type(type_name) {
        if !ruleste_plugins_api::host::entity_alive(eid) {
            continue;
        }
        let bp = ruleste_plugins_api::host::Position::new(eid).get();
        let (bw, bh, box_, boy) = ruleste_plugins_api::host::Hitbox::new(eid).get();
        let bx = bp.x + box_;
        let by = bp.y + boy;
        if px < bx + bw && px + w > bx && py < by + bh && py + h > by {
            return true;
        }
    }
    false
}

/// Handles events from other plugins (refills, boosters, bumpers, springs,
/// feathers, BadelineBoost) before movement. Resource-affecting events consult
/// the player's current dashes/stamina so a full player is not refilled.
fn handle_events(id: EntityId) {
    let mut new_state: Option<u32> = None;
    let mut speed_override: Option<Vec2> = None;
    for (_, kind, data) in ruleste_plugins_api::host::drain_events() {
        match kind {
            ruleste_plugins_api::host::EV_REFILL => {
                with_state(id, |st| {
                    let two = data.first().copied().unwrap_or(0) != 0;
                    let want = if two { 2 } else { MAX_DASHES };
                    let used = st.dashes < want || st.stamina < REFILL_STAMINA_MIN;
                    if used {
                        st.dashes = want;
                        st.dash_cooldown = 0.0;
                        st.dash_refill_cooldown = 0.0;
                        st.stamina = CLIMB_MAX_STAMINA;
                    }
                });
            }
            ruleste_plugins_api::host::EV_BOOST => {
                // Payload: booster center (Vec2, world units) then a red flag.
                // `Boost`/`RedBoost` keep dashes refilled (`BoostBegin`).
                let target = read_vec2(&data);
                let red = data.get(8).copied().unwrap_or(0) != 0;
                with_state(id, |st| {
                    st.dashes = MAX_DASHES;
                    st.stamina = CLIMB_MAX_STAMINA;
                    st.boost_target = target;
                    st.boost_red = red;
                    st.boost_timer = BOOST_TIME;
                    st.wall_slide_dir = 0;
                    new_state = Some(ST_BOOST);
                });
            }
            ruleste_plugins_api::host::EV_LAUNCH => {
                let from_dir = safe_normalize(&read_vec2(&data));
                with_state(id, |st| {
                    st.launched = true;
                    st.dashes = MAX_DASHES;
                    st.stamina = CLIMB_MAX_STAMINA;
                    st.dash_cooldown = DASH_COOLDOWN;
                    st.has_launch_approach = false;
                    speed_override = Some(Vec2::new(
                        from_dir.x * EXPLODE_LAUNCH_SPEED,
                        from_dir.y * EXPLODE_LAUNCH_SPEED,
                    ));
                    new_state = Some(ST_LAUNCH);
                });
            }
            ruleste_plugins_api::host::EV_SIDE_BOUNCE => {
                // Payload: `[dir u8][from_x f32][from_y f32]`; the spring face
                // (`base.Right`/`base.Left`) and `base.CenterY` of `Spring.cs`.
                let dir = match data.first() {
                    Some(b) => *b as i8 as i32,
                    None => 1,
                };
                let from_x = read_f32(&data[1..]).unwrap_or(0.0);
                let from_y = read_f32(&data[5..]).unwrap_or(0.0);
                with_state(id, |st| {
                    if speed_of(id).x.abs() > SIDE_BOUNCE_SPEED
                        && (speed_of(id).x.signum() as i32) == dir
                    {
                        // `SideBounce` early-out: too fast in the same direction.
                        return;
                    }
                    // `MoveV(Clamp(fromY - base.Bottom, -4, 4))` snaps the
                    // player's feet to the spring center height, and `MoveH`
                    // snaps the near edge onto the spring face.
                    let p = Entity::new(id).position.get();
                    let dy = (from_y - p.y).clamp(-4.0, 4.0);
                    // Normal hitbox 8x11 at (-4,-11): bottom = y, left = x-4,
                    // right = x+4.
                    let nx = if dir > 0 { from_x + 4.0 } else { from_x - 4.0 };
                    Entity::new(id).position.set_xy(nx, p.y + dy);
                    st.dashes = MAX_DASHES;
                    st.stamina = CLIMB_MAX_STAMINA;
                    st.jump_grace = 0.0;
                    st.var_jump_timer = 0.2;
                    st.dash_attack_timer = 0.0;
                    st.wall_slide_timer = WALL_SLIDE_TIME;
                    st.launched = false;
                    st.force_move_x = dir as f32;
                    st.force_move_timer = SIDE_BOUNCE_FORCE_TIME;
                    speed_override = Some(Vec2::new(SIDE_BOUNCE_SPEED * dir as f32, -140.0));
                    new_state = Some(ST_NORMAL);
                });
            }
            ruleste_plugins_api::host::EV_SUPER_BOUNCE => {
                let from_y = read_f32(&data).unwrap_or(0.0);
                with_state(id, |st| {
                    // `Player.SuperBounce(fromY)`: snap to the spring top, refill
                    // everything, then launch straight up at -185.
                    st.dashes = MAX_DASHES;
                    st.stamina = CLIMB_MAX_STAMINA;
                    st.jump_grace = 0.0;
                    st.var_jump_timer = 0.2;
                    st.var_jump_speed = SUPER_BOUNCE_SPEED;
                    st.dash_attack_timer = 0.0;
                    st.wall_slide_timer = WALL_SLIDE_TIME;
                    st.launched = false;
                    st.dash_refill_cooldown = 0.0;
                    let p = Entity::new(id).position.get();
                    Entity::new(id).position.set_xy(p.x, from_y);
                    speed_override = Some(Vec2::new(0.0, SUPER_BOUNCE_SPEED));
                    new_state = Some(ST_NORMAL);
                });
            }
            ruleste_plugins_api::host::EV_STARFLY => {
                let strength = read_f32(&data).unwrap_or(STARFLY_TIME);
                with_state(id, |st| {
                    if st.state == ST_STARFLY {
                        // Already flying: top up the timer (`StartStarFly`).
                        st.starfly_timer = st.starfly_timer.max(strength);
                    } else {
                        st.starfly_timer = strength;
                        st.starfly_transforming = true;
                        st.starfly_speed_lerp = 0.0;
                        st.jump_grace = 0.0;
                        new_state = Some(ST_STARFLY);
                    }
                });
            }
            ruleste_plugins_api::host::EV_BADELINE_BOOST => {
                let at_x = read_f32(&data).unwrap_or(0.0);
                let final_boost = data.get(4).copied().unwrap_or(0) != 0;
                with_state(id, |st| {
                    st.launched = true;
                    st.dashes = MAX_DASHES;
                    st.stamina = CLIMB_MAX_STAMINA;
                    st.dash_cooldown = DASH_COOLDOWN;
                    st.has_launch_approach = true;
                    st.launch_approach_x = at_x;
                    if final_boost {
                        st.launch_approach_x = at_x;
                        new_state = Some(ST_SUMMIT_LAUNCH);
                    } else {
                        new_state = Some(ST_LAUNCH);
                    }
                    speed_override = Some(Vec2::new(0.0, BADELINE_LAUNCH_SPEED));
                });
            }
            ruleste_plugins_api::host::EV_CARRIED => {
                let on = data.get(8).copied().unwrap_or(0) != 0;
                let pos = read_vec2(&data);
                with_state(id, |st| {
                    st.carried = on;
                    st.carry_target = pos;
                    if on {
                        speed_override = Some(Vec2::ZERO);
                    }
                });
            }
            _ => {}
        }
    }
    if let Some(s) = new_state {
        with_state(id, |st| st.state = s);
    }
    if let Some(v) = speed_override {
        ruleste_plugins_api::host::Speed::new(id).set(v);
    }
}

/// Reads the current speed of an entity (used for event guards).
fn speed_of(id: EntityId) -> Vec2 {
    ruleste_plugins_api::host::Speed::new(id).get()
}

fn read_f32(data: &[u8]) -> Option<f32> {
    data.get(0..4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(f32::from_le_bytes)
}

/// Reads a `Vec2` from the first 8 bytes of an event payload.
fn read_vec2(data: &[u8]) -> Vec2 {
    let x = read_f32(data).unwrap_or(0.0);
    let y = data
        .get(4..8)
        .map(|s| f32::from_le_bytes(s.try_into().ok().unwrap_or([0; 4])))
        .unwrap_or(0.0);
    Vec2::new(x, y)
}

/// Derives a readable state name and, in debug mode, logs transitions.
fn debug_state_log(id: EntityId, st: &PlayerState) {
    if st.debug_state == st.state {
        return;
    }
    if !ruleste_plugins_api::host::debug_enabled() {
        return;
    }
    let name = match st.state {
        ST_CLIMB => "Climb",
        ST_DASH => "Dash",
        ST_BOOST => "Boost",
        ST_RED_DASH => "RedDash",
        ST_LAUNCH => "Launch",
        ST_SUMMIT_LAUNCH => "SummitLaunch",
        ST_STARFLY => "StarFly",
        _ => "Normal",
    };
    ruleste_plugins_api::host::log(&format!("player {id} state -> {name} ({})", st.state));
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
    // Version 4 fields (boost / launch / star fly / carried).
    push_f32(&mut buf, st.boost_target.x);
    push_f32(&mut buf, st.boost_target.y);
    push_f32(&mut buf, st.boost_timer);
    push_f32(&mut buf, st.launch_approach_x);
    push_u8(&mut buf, u8::from(st.has_launch_approach));
    push_f32(&mut buf, st.starfly_timer);
    push_u8(&mut buf, u8::from(st.starfly_transforming));
    push_f32(&mut buf, st.starfly_speed_lerp);
    push_f32(&mut buf, st.starfly_last_dir.x);
    push_f32(&mut buf, st.starfly_last_dir.y);
    push_u8(&mut buf, u8::from(st.carried));
    push_f32(&mut buf, st.carry_target.x);
    push_f32(&mut buf, st.carry_target.y);
    // Version 5 field: red booster flag.
    push_u8(&mut buf, u8::from(st.boost_red));
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
    // Version 4 trailing fields (29 bytes: 2 f32 + 1 f32 + 1 f32 + 1 u8 +
    // 1 f32 + 1 u8 + 1 f32 + 2 f32 + 1 u8 + 2 f32).
    if r.remaining() >= 29 {
        st.boost_target = Vec2::new(r.f32()?, r.f32()?);
        st.boost_timer = r.f32()?;
        st.launch_approach_x = r.f32()?;
        st.has_launch_approach = r.u8()? != 0;
        st.starfly_timer = r.f32()?;
        st.starfly_transforming = r.u8()? != 0;
        st.starfly_speed_lerp = r.f32()?;
        st.starfly_last_dir = Vec2::new(r.f32()?, r.f32()?);
        st.carried = r.u8()? != 0;
        st.carry_target = Vec2::new(r.f32()?, r.f32()?);
    }
    // Version 5 trailing field (red booster flag).
    if r.remaining() >= 1 {
        st.boost_red = r.u8()? != 0;
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

fn safe_normalize(v: &Vec2) -> Vec2 {
    let len = v.length();
    if len < 1e-6 {
        Vec2::ZERO
    } else {
        Vec2::new(v.x / len, v.y / len)
    }
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}
