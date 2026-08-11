#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `crushBlock` entity plugin.
//!
//! Mirrors `CrushBlock.cs`: a solid riding platform that, when the player dashes
//! into it, crushes in the opposite direction of the dash — accelerating to
//! 240 u/s until it hits a wall — then returns to its origin. `axes` restricts
//! crushing to horizontal/vertical/both; `chillout` blocks are the huge ones
//! that may only be pushed from the right and never reset.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::Color;
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("crush-block");
ruleste_plugin_api::ruleste_entity_types!("crushBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

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

/// 0 = idle, 1 = shaking before crush, 2 = crushing, 3 = stopped at wall,
/// 4 = returning to origin.
#[derive(Debug)]
struct CrushState {
    axes: Axes,
    chill_out: bool,
    giant: bool,
    can_activate: bool,
    origin: (f32, f32),
    crush_dir: (i32, i32),
    speed: f32,
    phase: u32,
    timer: f32,
    anim: f32,
}

impl Default for CrushState {
    fn default() -> CrushState {
        CrushState {
            axes: Axes::Both,
            chill_out: false,
            giant: false,
            can_activate: true,
            origin: (0.0, 0.0),
            crush_dir: (0, 0),
            speed: 0.0,
            phase: 0,
            timer: 0.0,
            anim: 0.0,
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
        st.origin = (x, y);
    });
}

fn can_activate(st: &CrushState, dir: (i32, i32)) -> bool {
    if st.giant && dir.0 <= 0 {
        return false;
    }
    if !st.can_activate || st.crush_dir == dir || st.crush_dir == (-dir.0, -dir.1) {
        return false;
    }
    if dir.0 != 0 && st.axes == Axes::Vertical {
        return false;
    }
    if dir.1 != 0 && st.axes == Axes::Horizontal {
        return false;
    }
    true
}

/// Moves the block `amount` units along its crush axis, stopping at solid
/// tiles. Returns `true` when a wall (or level bounds) was hit.
fn move_crush(entity: &Entity, st: &mut CrushState, amount: f32) -> bool {
    let (dx, dy) = st.crush_dir;
    let (mx, my) = (dx as f32 * amount, dy as f32 * amount);
    let result = entity.collision.actor_move(mx, my);
    let hit = if dx != 0 {
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
            if kind == host::EV_CRUSH && st.phase == 0 && !data.is_empty() {
                // The player dashes into the block; payload is 2 bytes: [h, v]
                // each clamped to ±1, encoding the wall hit direction.
                // `OnDashed` attacks in `-direction` and only when the block
                // can activate along its configured axes.
                let h_dir = (data[0] as i8 as i32).clamp(-1, 1);
                let v_dir = if data.len() > 1 {
                    (data[1] as i8 as i32).clamp(-1, 1)
                } else {
                    0
                };
                let crush = if st.axes == Axes::Horizontal {
                    (-h_dir, 0)
                } else if st.axes == Axes::Vertical {
                    (0, -v_dir)
                } else {
                    if h_dir != 0 { (-h_dir, 0) } else { (0, -v_dir) }
                };
                if can_activate(st, crush) {
                    st.can_activate = false;
                    st.crush_dir = crush;
                    st.phase = 1;
                    st.timer = SHAKE_TIME;
                    st.speed = 0.0;
                }
            }
        }

        match st.phase {
            0 => {}
            1 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 2;
                }
            }
            2 => {
                st.speed = (st.speed + CRUSH_ACCEL * dt).min(CRUSH_SPEED);
                if move_crush(&entity, st, st.speed * dt) {
                    st.phase = 3;
                    st.timer = STOP_TIME;
                }
            }
            3 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    if st.chill_out {
                        st.can_activate = true;
                        st.crush_dir = (0, 0);
                        st.phase = 0;
                    } else {
                        st.phase = 4;
                        st.speed = 0.0;
                    }
                }
            }
            4 => {
                st.speed = (st.speed + RETURN_ACCEL * dt).min(RETURN_SPEED);
                let (dx, dy) = st.crush_dir;
                let step = st.speed * dt;
                if dx != 0 {
                    let p = entity.position.get();
                    let toward = if dx < 0 { -1.0 } else { 1.0 };
                    if (p.x - st.origin.0).abs() > step {
                        let _ = entity.collision.actor_move(toward * step, 0.0);
                    }
                }
                if dy != 0 {
                    let p = entity.position.get();
                    let toward = if dy < 0 { -1.0 } else { 1.0 };
                    if (p.y - st.origin.1).abs() > step {
                        let _ = entity.collision.actor_move(0.0, toward * step);
                    }
                }
                let p = entity.position.get();
                if (p.x - st.origin.0).abs() < 0.5 && (p.y - st.origin.1).abs() < 0.5 {
                    entity.position.set_xy(st.origin.0, st.origin.1);
                    st.can_activate = true;
                    st.crush_dir = (0, 0);
                    st.phase = 0;
                }
            }
            _ => {}
        }
    });
}

/// Face frame selection: idle when at rest, a `hit_<dir>` loop while crushing.
fn face_frame(st: &CrushState) -> String {
    if st.phase == 0 {
        "objects/crushblock/idle_face".to_string()
    } else {
        let (dx, dy) = st.crush_dir;
        let dir = if dx < 0 {
            "left"
        } else if dx > 0 {
            "right"
        } else if dy < 0 {
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
