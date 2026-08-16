#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `spinner` hazard plugin.
//!
//! Mirrors `DustStaticSpinner.cs` and `CrystalStaticSpinner.cs`.
//! - Dust mode (Area 3, 7 d-chapters): spinning dust crystal (`danger/dustcreature/...`)
//! - Crystal mode (Default, Area 5/6/9/10 or `color` attribute): blue/red/purple/rainbow crystal (`danger/crystal/...`)

use ruleste_plugin_api::host::{self, die, draw_image, draw_image_color, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("spinner");
ruleste_plugin_api::ruleste_entity_types!("spinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const KILL_W: f32 = 16.0;
const KILL_H: f32 = 12.0;
const CORNERS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

const CENTER_FRAMES: [&str; 1] = ["danger/dustcreature/center00"];
const BASE_FRAMES: [&str; 3] = [
    "danger/dustcreature/base00",
    "danger/dustcreature/base01",
    "danger/dustcreature/base02",
];
const OVERLAY_FRAMES: [&str; 3] = [
    "danger/dustcreature/overlay00",
    "danger/dustcreature/overlay01",
    "danger/dustcreature/overlay02",
];

const CENTER_SPIN: f32 = 0.6; // radians/sec
const NODE_SPIN: f32 = 0.5; // radians/sec

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrystalColor {
    Blue,
    Red,
    Purple,
    Rainbow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpinnerKind {
    Dust,
    Crystal(CrystalColor),
}

#[derive(Debug)]
struct SpinnerState {
    kind: SpinnerKind,
    attach_to_solid: bool,
    center_timer: f32,
    node_rot: [f32; 4],
    center_idx: usize,
    base_idx: usize,
    overlay_idx: usize,
    timer: f32,
}

impl Default for SpinnerState {
    fn default() -> SpinnerState {
        SpinnerState {
            kind: SpinnerKind::Dust,
            attach_to_solid: false,
            center_timer: 0.0,
            node_rot: [0.0; 4],
            center_idx: 0,
            base_idx: 0,
            overlay_idx: 0,
            timer: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SpinnerState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SpinnerState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SpinnerState::default) as *mut SpinnerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn hsv_to_color(h: f32, s: f32, v: f32) -> Color {
    let h_norm = (h % 1.0 + 1.0) % 1.0;
    let c = v * s;
    let x = c * (1.0 - ((h_norm * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h_norm * 6.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::new(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
        255,
    )
}

fn get_hue(p_x: f32, p_y: f32, timer: f32) -> Color {
    let len = (p_x * p_x + p_y * p_y).sqrt();
    let num = 280.0;
    let val = (len + timer * 50.0) % num / num;
    // YoYo easing
    let yoyo = if val < 0.5 {
        val * 2.0
    } else {
        (1.0 - val) * 2.0
    };
    hsv_to_color(0.4 + yoyo * 0.4, 0.4, 0.9)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let color_str = spawn.get_str("color", "");
    let attach = spawn.get_bool("attachToSolid", false);

    let kind = match color_str.as_str() {
        "dust" => SpinnerKind::Dust,
        "red" => SpinnerKind::Crystal(CrystalColor::Red),
        "purple" => SpinnerKind::Crystal(CrystalColor::Purple),
        "rainbow" => SpinnerKind::Crystal(CrystalColor::Rainbow),
        "blue" => SpinnerKind::Crystal(CrystalColor::Blue),
        _ => {
            // Default: if no color specified, use Blue crystal (or Dust if explicitly requested elsewhere)
            SpinnerKind::Crystal(CrystalColor::Blue)
        }
    };

    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(8.0, 8.0, -4.0, -4.0);

    match kind {
        SpinnerKind::Dust => entity.depth.set(-50),
        SpinnerKind::Crystal(_) => entity.depth.set(-8500),
    }

    with_state(id, |st| {
        st.kind = kind;
        st.attach_to_solid = attach;
        st.center_idx = (id as usize) % CENTER_FRAMES.len();
        st.base_idx = (id as usize) % BASE_FRAMES.len();
        st.overlay_idx = (id as usize + 1) % OVERLAY_FRAMES.len();
        for (i, rot) in st.node_rot.iter_mut().enumerate() {
            *rot = (id as f32 + i as f32) * 0.7;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        st.center_timer += dt * CENTER_SPIN;
        for rot in &mut st.node_rot {
            *rot += dt * NODE_SPIN;
        }

        let entity = Entity::new(id);
        let p = entity.position.get();

        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();

            // 128px distance culling check for crystal spinners
            if matches!(st.kind, SpinnerKind::Crystal(_)) {
                let dx = (pp.x - p.x).abs();
                let dy = (pp.y - p.y).abs();
                if dx > 128.0 || dy > 128.0 {
                    continue;
                }
            }

            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let px = pp.x + pox;
            let py = pp.y + poy;
            let overlap = px < p.x + KILL_W * 0.5
                && px + pw > p.x - KILL_W * 0.5
                && py < p.y + KILL_H * 0.5
                && py + ph > p.y - KILL_H * 0.5;
            if overlap {
                die();
            }
            break;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();

        match st.kind {
            SpinnerKind::Dust => {
                let center_rot_deg = st.center_timer.to_degrees();
                draw_image(
                    CENTER_FRAMES[st.center_idx],
                    p.x,
                    p.y,
                    center_rot_deg,
                    1.0,
                    1.0,
                );

                for (i, (cx, cy)) in CORNERS.iter().enumerate() {
                    if entity.collision.check(cx * 4.0, cy * 4.0) {
                        continue;
                    }
                    let mut vx = 1.0;
                    let mut vy = 1.0;
                    if entity.collision.check(cx * 16.0, cy * 4.0) {
                        vx = 5.0;
                    }
                    if entity.collision.check(cx * 4.0, cy * 16.0) {
                        vy = 5.0;
                    }
                    let n = (cx * cx + cy * cy).sqrt();
                    let ax = cx / n * vx;
                    let ay = cy / n * vy;

                    let rot = st.node_rot[i].to_degrees();
                    draw_image(BASE_FRAMES[st.base_idx], p.x + ax, p.y + ay, rot, 1.0, 1.0);
                    draw_image(
                        OVERLAY_FRAMES[st.overlay_idx],
                        p.x + ax,
                        p.y + ay,
                        -rot,
                        1.0,
                        1.0,
                    );
                }
            }
            SpinnerKind::Crystal(color) => {
                let (fg_frame, bg_frame) = match color {
                    CrystalColor::Blue => ("danger/crystal/fg_blue00", "danger/crystal/bg_blue00"),
                    CrystalColor::Red => ("danger/crystal/fg_red00", "danger/crystal/bg_red00"),
                    CrystalColor::Purple => {
                        ("danger/crystal/fg_purple00", "danger/crystal/bg_purple00")
                    }
                    CrystalColor::Rainbow => {
                        ("danger/crystal/fg_white00", "danger/crystal/bg_white00")
                    }
                };

                // Draw crystal background connectors to neighboring spinners (< 24px)
                let spinners = entities_by_type("spinner");
                for &other_id in &spinners {
                    if other_id <= id {
                        continue; // drawn once per pair
                    }
                    let other_p = host::Position::new(other_id).get();
                    let dx = other_p.x - p.x;
                    let dy = other_p.y - p.y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 24.0 * 24.0 {
                        let mid_x = (p.x + other_p.x) * 0.5;
                        let mid_y = (p.y + other_p.y) * 0.5;
                        if color == CrystalColor::Rainbow {
                            let hue = get_hue(mid_x, mid_y, st.timer);
                            draw_image_color(bg_frame, mid_x, mid_y, hue);
                        } else {
                            draw_image(bg_frame, mid_x, mid_y, 0.0, 1.0, 1.0);
                        }
                    }
                }

                // Draw main crystal sprite
                if color == CrystalColor::Rainbow {
                    let hue = get_hue(p.x, p.y, st.timer);
                    draw_image_color(fg_frame, p.x, p.y, hue);
                } else {
                    draw_image(fg_frame, p.x, p.y, 0.0, 1.0, 1.0);
                }
            }
        }
    });
}
