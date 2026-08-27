#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `crushBlock` entity plugin.
//!
//! Mirrors `CrushBlock.cs`: a solid riding platform that, when the player dashes
//! into it, crushes in the opposite direction of the dash — accelerating to
//! 240 u/s until it hits a wall — then returns along its `returnStack` to where
//! it came from. `axes` restricts crushing to horizontal/vertical/both;
//! `chillout` blocks are the huge ones that may only be pushed from the right,
//! slow down before walls, and never return.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("crush-block");
ruleste_plugins_api::ruleste_entity_types!("crushBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const CRUSH_SPEED: f32 = 240.0;
const CRUSH_ACCEL: f32 = 500.0;
const RETURN_SPEED: f32 = 60.0;
const RETURN_ACCEL: f32 = 160.0;
const SHAKE_TIME: f32 = 0.4;
const STOP_TIME: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axes {
    Both,
    Horizontal,
    Vertical,
}

/// One entry in the return stack: from where the block started crushing in
/// `dir`, used to hop back waypoint-by-waypoint.
#[derive(Debug, Clone, Copy)]
struct MoveState {
    from: Vec2,
    dir: Vec2,
}

/// 0 = idle, 1 = shaking before crush, 2 = crushing, 3 = stopped at wall,
/// 4 = returning to a waypoint.
#[derive(Debug)]
struct CrushState {
    axes: Axes,
    chill_out: bool,
    giant: bool,
    can_activate: bool,
    crush_dir: Vec2,
    speed: f32,
    phase: u32,
    timer: f32,
    anim: f32,
    return_stack: std::vec::Vec<MoveState>,
    /// `AttackSequence.slowing`: latched true once a chillout block first
    /// detects a wall ≤256 px ahead, so it keeps braking even if the wall
    /// briefly moves out of probe range.
    slowing: bool,
}

impl Default for CrushState {
    fn default() -> CrushState {
        CrushState {
            axes: Axes::Both,
            chill_out: false,
            giant: false,
            can_activate: true,
            crush_dir: Vec2::ZERO,
            speed: 0.0,
            phase: 0,
            timer: 0.0,
            anim: 0.0,
            return_stack: std::vec::Vec::new(),
            slowing: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CrushState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CrushState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CrushState::default) as *mut CrushState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn read_dir(data: &[u8]) -> Vec2 {
    // New ABI: payload is two little-endian f32 (dx, dy) — the dash direction.
    if data.len() >= 8 {
        let dx = f32::from_le_bytes(data[0..4].try_into().unwrap());
        let dy = f32::from_le_bytes(data[4..8].try_into().unwrap());
        if dx != 0.0 || dy != 0.0 {
            return Vec2::new(dx.signum(), dy.signum());
        }
    }
    // Legacy payload: a single i8 wall direction.
    if let Some(&b) = data.first() {
        let s = (b as i8).clamp(-1, 1) as f32;
        if s != 0.0 {
            return Vec2::new(-s, 0.0);
        }
    }
    Vec2::ZERO
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0).max(1.0);
    let h = spawn.get_float("height", 8.0).max(1.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.collision.solid(true);
    entity.depth.set(-10000);
    with_state(id, |st| {
        st.axes = match spawn.get_str("axes", "Both").as_str() {
            "Horizontal" => Axes::Horizontal,
            "Vertical" => Axes::Vertical,
            _ => Axes::Both,
        };
        st.chill_out = spawn.get_bool("chillout", false);
        st.giant = w >= 48.0 && h >= 48.0 && st.chill_out;
        st.can_activate = true;
    });
}

/// `CanActivate(direction)`: giant blocks only from the right; a fresh,
/// differently-directed hit; respects the `axes` restriction. The original
/// (`CrushBlock.cs:CanActivate`) only blocks *same* direction — perpendicular
/// retargeting is allowed mid-crush, opposite direction also allowed (so a
/// reverse-aimed dash always re-triggers).
fn can_activate(st: &CrushState, dir: Vec2) -> bool {
    if st.giant && dir.x <= 0.0 {
        return false;
    }
    if !st.can_activate || st.crush_dir == dir {
        return false;
    }
    if dir.x != 0.0 && st.axes == Axes::Vertical {
        return false;
    }
    if dir.y != 0.0 && st.axes == Axes::Horizontal {
        return false;
    }
    true
}

/// Moves the block `amount` units along its crush axis, stopping at solid
/// tiles. Returns `true` when a wall (or level bounds) was hit.
fn move_crush(entity: &Entity, st: &mut CrushState, amount: f32) -> bool {
    let (dx, dy) = (st.crush_dir.x, st.crush_dir.y);
    let (mx, my) = (dx * amount, dy * amount);
    let result = entity.collision.actor_move(mx, my);
    let hit = if dx != 0.0 {
        (mx < 0.0 && result.hit_wall_left) || (mx > 0.0 && result.hit_wall_right)
    } else {
        (my < 0.0 && result.hit_ceiling) || (my > 0.0 && result.on_ground)
    };
    let p = entity.position.get();
    if hit || p.x < -1000.0 || p.y < -1000.0 || p.x > 10000.0 || p.y > 10000.0 {
        return true;
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.anim += dt;
        let entity = Entity::new(id);

        for (_, kind, data) in host::drain_events() {
            if kind == host::EV_CRUSH && st.phase == 0 {
                let dir = read_dir(&data);
                if dir != Vec2::ZERO {
                    // `OnDashed(player, direction)` → `CanActivate(-direction)`
                    // then `Attack(-direction)`.
                    let crush = vec2(-dir.x, -dir.y);
                    if can_activate(st, crush) {
                        attack(st, &entity, crush);
                    }
                }
            }
        }

        match st.phase {
            0 => {}
            1 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    // `AttackSequence`: canActivate re-armed once the shake
                    // finishes (never for chillout).
                    if !st.chill_out {
                        st.can_activate = true;
                    }
                    st.phase = 2;
                }
            }
            2 => {
                // `AttackSequence`: latch the chillout slow-down the moment
                // a wall appears within 256 px so the block stays braking
                // even if the wall briefly moves out of probe range.
                if st.chill_out && !st.slowing && crushes_toward_solid(&entity, st, 256.0) {
                    st.slowing = true;
                }
                let target = if st.chill_out && st.slowing {
                    24.0
                } else {
                    CRUSH_SPEED
                };
                // Chillout uses `500f * 0.25f = 125f` accel only while
                // braking; otherwise (and for non-chillout) the regular
                // 500 u/s² crush acceleration applies.
                let accel = if st.chill_out && st.slowing {
                    CRUSH_ACCEL * 0.25
                } else {
                    CRUSH_ACCEL
                };
                st.speed = approach(st.speed, target, accel * dt);
                if move_crush(&entity, st, st.speed * dt) {
                    // `CollideFirst<FallingBlock>` + impact particles + shake
                    // are cosmetic; the block parks then returns/stays.
                    st.phase = 3;
                    st.timer = STOP_TIME;
                }
            }
            3 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    if st.chill_out {
                        // `if (chillOut) yield break;` — stuck forever.
                        st.phase = 5;
                    } else {
                        st.phase = 4;
                        st.speed = 0.0;
                        st.timer = 0.0;
                    }
                }
            }
            4 => {
                // `while (returnStack.Count > 0)`: rest a beat then glide back
                // toward the top waypoint.
                st.timer -= dt;
                if st.timer > 0.0 {
                    return;
                }
                if st.return_stack.is_empty() {
                    st.crush_dir = Vec2::ZERO;
                    st.phase = 0;
                    return;
                }
                st.speed = (st.speed + RETURN_ACCEL * dt).min(RETURN_SPEED);
                let step = st.speed * dt;
                let top = *st.return_stack.last().expect("non-empty on return");
                let p = entity.position.get();
                let mut cur = p;
                if top.dir.x != 0.0 {
                    let remaining = top.from.x - cur.x;
                    let move_x = remaining.abs().min(step) * remaining.signum();
                    let result = entity.collision.actor_move(move_x, 0.0);
                    let np = entity.position.get();
                    let _ = result;
                    cur = np;
                }
                if top.dir.y != 0.0 {
                    let remaining = top.from.y - cur.y;
                    let move_y = remaining.abs().min(step) * remaining.signum();
                    let result = entity.collision.actor_move(0.0, move_y);
                    let np = entity.position.get();
                    let _ = result;
                    cur = np;
                }
                let x_arrived = top.dir.x == 0.0 || (cur.x - top.from.x).abs() < 0.5;
                let y_arrived = top.dir.y == 0.0 || (cur.y - top.from.y).abs() < 0.5;
                if x_arrived && y_arrived {
                    // Snap exactly onto the waypoint (`MoveTowards` rounding).
                    entity.position.set_xy(top.from.x, cur.y);
                    let _ = entity.position.get();
                    st.return_stack.pop();
                    st.speed = 0.0;
                    if st.return_stack.is_empty() {
                        st.crush_dir = Vec2::ZERO;
                        st.phase = 0;
                    } else {
                        // `yield return 0.2f` between waypoints.
                        st.timer = 0.2;
                    }
                }
            }
            // 5 = chillout, permanently stopped.
            5 => {}
            _ => {}
        }
    });
}

fn approach(value: f32, target: f32, max_move: f32) -> f32 {
    let diff = target - value;
    if diff.abs() <= max_move {
        target
    } else {
        value + diff.signum() * max_move
    }
}

fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn neg(v: Vec2) -> Vec2 {
    vec2(-v.x, -v.y)
}

/// `Attack(direction)`: arm the crush, add a return waypoint (deduping the
/// same/opposed direction), reset state.
fn attack(st: &mut CrushState, entity: &Entity, dir: Vec2) {
    st.can_activate = false;
    st.crush_dir = dir;
    st.phase = 1;
    st.timer = SHAKE_TIME;
    st.speed = 0.0;
    st.slowing = false;
    // `returnStack`: only add a waypoint whose crush direction differs from the
    // top of the stack (`Attack` logic).
    let flag = match st.return_stack.last() {
        Some(top) => !(top.dir == dir || top.dir == neg(dir)),
        None => true,
    };
    if flag {
        let p = entity.position.get();
        st.return_stack.push(MoveState { from: p, dir });
    }
}

/// `CollideCheck<SolidTiles>(position + crushDir * 256)` — used by chillout
/// blocks to start braking before a wall.
fn crushes_toward_solid(entity: &Entity, st: &CrushState, dist: f32) -> bool {
    entity
        .collision
        .check(st.crush_dir.x * dist, st.crush_dir.y * dist)
}

/// Face frame selection: idle when at rest, a `hit_<dir>` loop while crushing.
fn face_frame(st: &CrushState) -> String {
    if st.phase == 0 {
        "objects/crushblock/idle_face".to_string()
    } else {
        let (dx, dy) = (st.crush_dir.x, st.crush_dir.y);
        let dir = if dx < 0.0 {
            "left"
        } else if dx > 0.0 {
            "right"
        } else if dy < 0.0 {
            "up"
        } else {
            "down"
        };
        let idx = (st.anim * 12.0) as usize % 2;
        format!("objects/crushblock/hit_{dir}{idx:02}")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let (w, h, ox, oy) = entity.hitbox.get();
        let x = p.x + ox;
        let y = p.y + oy;
        host::draw_rect(
            x + 2.0,
            y + 2.0,
            w - 4.0,
            h - 4.0,
            Color::new(98, 34, 43, 255),
        );
        host::draw_image(&face_frame(st), x + w * 0.5, y + h * 0.5, 0.0, 1.0, 1.0);
    });
}
