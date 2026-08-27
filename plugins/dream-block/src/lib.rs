#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `dreamBlock` entity plugin.
//!
//! Mirrors `DreamBlock.cs`: a solid block that acts as a rideable platform.
//! - Disabled state (player lacks Dream Dash): dark teal slab with drifting
//!   light-grey particles, wobbling border, corner blocks, light occlude.
//! - Active state (player has Dream Dash): black slab with colorful particles,
//!   white border, no occlude. Moves between nodes in a YoyoLooping SineInOut
//!   pattern when a node is present.
//! - Activation sequence: 1s delay → shake → white fill (CubeIn) → activate
//!   → burst particles/shake → fade white fill.
//! - `oneUse`: destroys itself after player exits (OnPlayerExit).
//! - `BlockedCheck`: pushes TheoCrystal/Player up up to 4px if trapped.

use ruleste_plugins_api::event;
use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("dream-block");
ruleste_plugins_api::ruleste_entity_types!("dreamBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

// --- Colors (match original hex) ---
const DISABLED_BACK: Color = Color::new(0x1f, 0x2e, 0x2d, 255);
const ACTIVE_BACK: Color = Color::new(0x00, 0x00, 0x00, 255);
const DISABLED_LINE: Color = Color::new(0x6a, 0x84, 0x80, 255);
const ACTIVE_LINE: Color = Color::new(0xff, 0xff, 0xff, 255);

// Layer 0 colors (disabled: light grey; active: yellow/pink/green)
const LAYER0_COLORS: [Color; 3] = [
    Color::new(0xff, 0xef, 0x11, 200), // FFEF11
    Color::new(0xff, 0x00, 0xd0, 200), // FF00D0
    Color::new(0x08, 0xa3, 0x10, 200), // 08a310
];
// Layer 1 colors
const LAYER1_COLORS: [Color; 3] = [
    Color::new(0x5f, 0xcd, 0xe4, 200), // 5fcde4
    Color::new(0x7f, 0xb2, 0x5e, 200), // 7fb25e
    Color::new(0xe0, 0x56, 0x4c, 200), // E0564C
];
// Layer 2 colors
const LAYER2_COLORS: [Color; 3] = [
    Color::new(0x5b, 0x6e, 0xe1, 200), // 5b6ee1
    Color::new(0xcc, 0x3b, 0x3b, 200), // CC3B3B
    Color::new(0x7d, 0xaa, 0x64, 200), // 7daa64
];

// Particle frame indices for each layer (matches original particleTextures array)
// layer 0: animates 3→2→1→0 (4 frames)
// layer 1: alternates 1↔2 (2 frames)
// layer 2: static frame 2
const PARTICLE_FRAMES: [[&str; 4]; 3] = [
    [
        "objects/dreamblock/particles03",
        "objects/dreamblock/particles02",
        "objects/dreamblock/particles01",
        "objects/dreamblock/particles00",
    ],
    [
        "objects/dreamblock/particles01",
        "objects/dreamblock/particles02",
        "objects/dreamblock/particles01",
        "objects/dreamblock/particles02",
    ],
    [
        "objects/dreamblock/particles02",
        "objects/dreamblock/particles02",
        "objects/dreamblock/particles02",
        "objects/dreamblock/particles02",
    ],
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum DreamPhase {
    Disabled,   // player lacks DreamDash
    Active,     // player has DreamDash, fully active
    Activating, // running activation sequence
    Breaking,   // oneUse: fading out after player exit
}

#[derive(Debug)]
struct DreamParticle {
    x: f32,
    y: f32,
    layer: u8, // 0, 1, 2
    time_offset: f32,
    color: Color,
}

#[derive(Debug)]
struct DreamState {
    w: f32,
    h: f32,
    node: Option<Vec2>,
    start_pos: Vec2,
    fast_moving: bool,
    one_use: bool,
    below: bool,
    phase: DreamPhase,
    timer: f32,
    // Activation sequence sub-state
    activate_stage: u8, // 0=delay, 1=shake+whitefill, 2=burst, 3=fade
    activate_timer: f32,
    white_fill: f32,
    white_height: f32,
    shake_x: f32,
    shake_y: f32,
    // Movement tween
    move_progress: f32, // 0..1 for SineInOut yoyo
    move_dir: f32,      // 1 or -1
    move_duration: f32, // full cycle duration
    // Wobble
    wobble_from: f32,
    wobble_to: f32,
    wobble_ease: f32,
    anim_timer: f32,
    particles: Vec<DreamParticle>,
    // Player state
    player_has_dream_dash: bool,
    // oneUse: player exit trigger
    player_was_inside: bool,
}

impl Default for DreamState {
    fn default() -> DreamState {
        DreamState {
            w: 8.0,
            h: 8.0,
            node: None,
            start_pos: Vec2::ZERO,
            fast_moving: false,
            one_use: false,
            below: false,
            phase: DreamPhase::Disabled,
            timer: 0.0,
            activate_stage: 0,
            activate_timer: 0.0,
            white_fill: 0.0,
            white_height: 1.0,
            shake_x: 0.0,
            shake_y: 0.0,
            move_progress: 0.0,
            move_dir: 1.0,
            move_duration: 1.0,
            wobble_from: 0.0,
            wobble_to: 0.0,
            wobble_ease: 0.0,
            anim_timer: 0.0,
            particles: Vec::new(),
            player_has_dream_dash: false,
            player_was_inside: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DreamState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DreamState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DreamState::default) as *mut DreamState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// Deterministic LCG for reproducible particle layout across hot reloads.
fn lcg(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 24) as f32
}

fn make_particles(st: &mut DreamState, player_has_dream_dash: bool) {
    let count = (st.w / 8.0 * (st.h / 8.0) * 0.7).round() as usize;
    let mut seed = 0x2d_5a_11u32;
    st.particles.clear();
    st.particles.reserve(count);

    for _ in 0..count {
        let x = lcg(&mut seed) * st.w;
        let y = lcg(&mut seed) * st.h;
        let r = lcg(&mut seed);
        let layer = if r < 1.0 / 6.0 {
            0
        } else if r < 3.0 / 6.0 {
            1
        } else {
            2
        };
        let time_offset = lcg(&mut seed);

        let color = if player_has_dream_dash {
            let c = lcg(&mut seed);
            let idx = (c * 3.0).floor() as usize;
            match layer {
                0 => LAYER0_COLORS[idx.min(2)],
                1 => LAYER1_COLORS[idx.min(2)],
                _ => LAYER2_COLORS[idx.min(2)],
            }
        } else {
            // Disabled: LightGray * (0.5 + layer/2 * 0.5)
            let v = (0.5 + layer as f32 / 2.0 * 0.5) * 255.0;
            Color::new(v as u8, v as u8, v as u8, 200)
        };

        st.particles.push(DreamParticle {
            x,
            y,
            layer: layer as u8,
            time_offset,
            color,
        });
    }
}

/// SineInOut easing
fn ease_sine_in_out(t: f32) -> f32 {
    -(f32::cos(t * std::f32::consts::PI) - 1.0) * 0.5
}

/// Line amplitude for wobble border (matches original LineAmplitude)
fn line_amplitude(seed: f32, index: f32) -> f32 {
    ((seed + index / 16.0 + (seed * 2.0 + index / 32.0).sin() * 2.0 * std::f32::consts::PI).sin()
        + 1.0)
        * 1.5
}

/// Wobble line drawing (approximated with line segments)
fn draw_wobble_line(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    offset: f32,
    wobble_from: f32,
    wobble_to: f32,
    wobble_ease: f32,
    color: Color,
    back_color: Color,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 4.0 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    // Perpendicular vector
    let px = uy;
    let py = -ux;

    let mut prev_amplitude = 0.0;
    let step = 16.0;
    let mut i = 2.0;
    while i < len - 2.0 {
        let a_from = line_amplitude(wobble_from + offset, i);
        let a_to = line_amplitude(wobble_to + offset, i);
        let amplitude = a_from + (a_to - a_from) * wobble_ease;
        let next_i = (i + step).min(len - 2.0);
        let _seg_len = next_i - i;

        // Two back lines (offset by 1px and 2px perpendicular) then main line
        let x_start = x1 + ux * i + px * prev_amplitude;
        let y_start = y1 + uy * i + py * prev_amplitude;
        let x_end = x1 + ux * next_i + px * amplitude;
        let y_end = y1 + uy * next_i + py * amplitude;

        // Back line 1 (1px offset)
        host::draw_line(
            x_start - px,
            y_start - py,
            x_end - px,
            y_end - py,
            back_color,
        );
        // Back line 2 (2px offset)
        host::draw_line(
            x_start - px * 2.0,
            y_start - py * 2.0,
            x_end - px * 2.0,
            y_end - py * 2.0,
            back_color,
        );
        // Main line
        host::draw_line(x_start, y_start, x_end, y_end, color);

        prev_amplitude = amplitude;
        i = next_i;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let fast_moving = spawn.get_bool("fastMoving", false);
    let one_use = spawn.get_bool("oneUse", false);
    let below = spawn.get_bool("below", false);

    // Node position from entity's node list (first node)
    let node = spawn.nodes().get(0).copied();

    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(if below { 5000 } else { -11000 });
    entity.collision.solid(true);

    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.node = node;
        st.start_pos = Vec2::new(x, y);
        st.fast_moving = fast_moving;
        st.one_use = one_use;
        st.below = below;
        st.phase = DreamPhase::Disabled;
        st.player_has_dream_dash = false;
        make_particles(st, false);

        // Setup movement duration if node exists
        if let Some(node_pos) = node {
            let dist = ((node_pos.x - x).powi(2) + (node_pos.y - y).powi(2)).sqrt();
            let mut duration = dist / 12.0;
            if fast_moving {
                duration /= 3.0;
            }
            st.move_duration = duration.max(0.01);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    // Drain events first (e.g., DREAM_DASH_GRANTED)
    let mut got_dream_dash = false;
    for (_, kind, _) in host::drain_events() {
        if kind == event::DREAM_DASH_GRANTED {
            got_dream_dash = true;
        }
    }

    with_state(id, |st| {
        st.timer += dt;
        st.anim_timer += dt * 6.0;

        // Handle dream dash acquisition
        if got_dream_dash && !st.player_has_dream_dash {
            st.player_has_dream_dash = true;
            // If we have a node and were disabled, start activation
            if st.node.is_some() && st.phase == DreamPhase::Disabled {
                st.phase = DreamPhase::Activating;
                st.activate_stage = 0;
                st.activate_timer = 0.0;
                st.white_fill = 0.0;
                st.white_height = 1.0;
                st.shake_x = 0.0;
                st.shake_y = 0.0;
            }
            make_particles(st, true);
        }

        match st.phase {
            DreamPhase::Activating => {
                // Stage 0: 1 second delay
                if st.activate_stage == 0 {
                    st.activate_timer += dt;
                    if st.activate_timer >= 1.0 {
                        st.activate_stage = 1;
                        st.activate_timer = 0.0;
                        // Start shake
                        st.shake_x = (st.timer * 123.456).sin() * 2.0;
                        st.shake_y = (st.timer * 234.567).cos() * 2.0;
                    }
                }
                // Stage 1: White fill with CubeIn easing over ~1s
                else if st.activate_stage == 1 {
                    st.activate_timer += dt;
                    let p = (st.activate_timer / 1.0).min(1.0);
                    st.white_fill = p * p * p; // CubeIn
                    // Shake
                    st.shake_x = (st.timer * 123.456).sin() * 2.0;
                    st.shake_y = (st.timer * 234.567).cos() * 2.0;
                    if p >= 1.0 {
                        st.activate_stage = 2;
                        st.activate_timer = 0.0;
                        st.white_height = 1.0;
                    }
                }
                // Stage 2: Burst particles & shake for 0.5s
                else if st.activate_stage == 2 {
                    st.activate_timer += dt;
                    st.white_height = 1.0 - st.activate_timer / 0.5;
                    if st.activate_timer >= 0.5 {
                        st.activate_stage = 3;
                        st.activate_timer = 0.0;
                        st.white_fill = 1.0;
                    }
                }
                // Stage 3: Fade white fill over ~0.33s
                else if st.activate_stage == 3 {
                    st.activate_timer += dt;
                    st.white_fill -= dt * 3.0;
                    if st.white_fill <= 0.0 {
                        st.white_fill = 0.0;
                        st.phase = DreamPhase::Active;
                    }
                }
            }
            DreamPhase::Active => {
                // Wobble ease update
                st.wobble_ease += dt * 2.0;
                if st.wobble_ease > 1.0 {
                    st.wobble_ease = 0.0;
                    st.wobble_from = st.wobble_to;
                    st.wobble_to = (st.timer * 7.123).sin() * 2.0 * std::f32::consts::PI;
                }

                // Node movement (YoyoLooping SineInOut)
                let mut moved = false;
                if let Some(node_pos) = st.node {
                    let cycle_progress = (st.timer / st.move_duration) % 2.0;
                    if cycle_progress <= 1.0 {
                        st.move_progress = ease_sine_in_out(cycle_progress);
                        st.move_dir = 1.0;
                    } else {
                        st.move_progress = ease_sine_in_out(2.0 - cycle_progress);
                        st.move_dir = -1.0;
                    }
                    let target_x =
                        st.start_pos.x + (node_pos.x - st.start_pos.x) * st.move_progress;
                    let target_y =
                        st.start_pos.y + (node_pos.y - st.start_pos.y) * st.move_progress;
                    let entity = Entity::new(id);
                    entity.position.set_xy(target_x, target_y);
                    moved = true;
                }

                // If moving, check for blocked entities and push them up
                if moved {
                    blocked_check(id, st);
                }

                // Check for player inside (for oneUse)
                let entity = Entity::new(id);
                let pos = entity.position.get();
                // Simple AABB check for player overlap
                let player_ids = host::entities_by_type("player");
                let player_inside = player_ids.iter().any(|&pid| {
                    let p_ent = Entity::new(pid);
                    let p_pos = p_ent.position.get();
                    let (p_w, p_h, _, _) = p_ent.hitbox.get();
                    p_pos.x + p_w > pos.x
                        && p_pos.x < pos.x + st.w
                        && p_pos.y + p_h > pos.y
                        && p_pos.y < pos.y + st.h
                });

                // oneUse: if player was inside and now left, trigger break
                if st.one_use && st.player_was_inside && !player_inside {
                    st.phase = DreamPhase::Breaking;
                    st.white_fill = 1.0;
                }
                st.player_was_inside = player_inside;
            }
            DreamPhase::Breaking => {
                // Fade out and remove
                st.white_fill -= dt * 2.0;
                if st.white_fill <= 0.0 {
                    host::remove(id);
                }
            }
            DreamPhase::Disabled => {}
        }
    });
}

/// Check if block is colliding with TheoCrystal or Player and try to wiggle them up
fn blocked_check(id: EntityId, st: &mut DreamState) -> bool {
    let entity = Entity::new(id);
    let pos = entity.position.get();

    // Check TheoCrystal
    let theo_ids = host::entities_by_type("theo");
    for tid in theo_ids {
        let t_ent = Entity::new(tid);
        let t_pos = t_ent.position.get();
        let (t_w, t_h, _, _) = t_ent.hitbox.get();
        if t_pos.x + t_w > pos.x
            && t_pos.x < pos.x + st.w
            && t_pos.y + t_h > pos.y
            && t_pos.y < pos.y + st.h
        {
            // Try wiggle up 1-4 pixels
            let t_coll = host::Collision::new(tid);
            for i in 1..=4 {
                if !t_coll.check(0.0, -i as f32) {
                    // Move theo up
                    let new_y = t_pos.y - i as f32;
                    t_ent.position.set_xy(t_pos.x, new_y);
                    return true;
                }
            }
            return true; // Blocked
        }
    }

    // Check Player
    let player_ids = host::entities_by_type("player");
    for pid in player_ids {
        let p_ent = Entity::new(pid);
        let p_pos = p_ent.position.get();
        let (p_w, p_h, _, _) = p_ent.hitbox.get();
        if p_pos.x + p_w > pos.x
            && p_pos.x < pos.x + st.w
            && p_pos.y + p_h > pos.y
            && p_pos.y < pos.y + st.h
        {
            let p_coll = host::Collision::new(pid);
            for i in 1..=4 {
                if !p_coll.check(0.0, -i as f32) {
                    let new_y = p_pos.y - i as f32;
                    p_ent.position.set_xy(p_pos.x, new_y);
                    return true;
                }
            }
            return true;
        }
    }

    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let pos = entity.position.get();
        let x = pos.x + st.shake_x;
        let y = pos.y + st.shake_y;
        let w = st.w;
        let h = st.h;

        // Background fill
        let back_color = if st.player_has_dream_dash {
            ACTIVE_BACK
        } else {
            DISABLED_BACK
        };
        host::draw_rect(x, y, w, h, back_color);

        // White fill during activation
        if st.white_fill > 0.0 {
            let white = Color::new(255, 255, 255, (255.0 * st.white_fill) as u8);
            host::draw_rect(x, y, w, h * st.white_height, white);
        }

        // Particles
        let cam_x = 0.0; // Camera position not exposed; assume 0 for now
        let cam_y = 0.0;
        for p in &st.particles {
            let layer = p.layer as usize;
            let px = p.x + cam_x * (0.3 + 0.25 * layer as f32);
            let py = p.y + cam_y * (0.3 + 0.25 * layer as f32);

            // Wrap inside block (PutInside equivalent)
            let mut wx = px;
            let mut wy = py;
            while wx < 0.0 {
                wx += w;
            }
            while wx > w {
                wx -= w;
            }
            while wy < 0.0 {
                wy += h;
            }
            while wy > h {
                wy -= h;
            }

            // Keep 2px margin from edges
            if wx >= 2.0 && wy >= 2.0 && wx < w - 2.0 && wy < h - 2.0 {
                // Frame index based on anim_timer and layer
                let frame_idx = match layer {
                    0 => ((p.time_offset * 4.0 + st.anim_timer) % 4.0) as usize,
                    1 => ((p.time_offset * 2.0 + st.anim_timer) % 2.0) as usize,
                    _ => 0,
                };
                let frame = PARTICLE_FRAMES[layer][frame_idx];
                host::draw_image_color(frame, x + wx, y + wy, p.color);
            }
        }

        // Border lines (wobble)
        let line_color = if st.player_has_dream_dash {
            ACTIVE_LINE
        } else {
            DISABLED_LINE
        };
        let back_line_color = back_color;

        // Top edge
        draw_wobble_line(
            x,
            y,
            x + w,
            y,
            0.0,
            st.wobble_from,
            st.wobble_to,
            st.wobble_ease,
            line_color,
            back_line_color,
        );
        // Right edge
        draw_wobble_line(
            x + w,
            y,
            x + w,
            y + h,
            0.7,
            st.wobble_from,
            st.wobble_to,
            st.wobble_ease,
            line_color,
            back_line_color,
        );
        // Bottom edge
        draw_wobble_line(
            x + w,
            y + h,
            x,
            y + h,
            1.5,
            st.wobble_from,
            st.wobble_to,
            st.wobble_ease,
            line_color,
            back_line_color,
        );
        // Left edge
        draw_wobble_line(
            x,
            y + h,
            x,
            y,
            2.5,
            st.wobble_from,
            st.wobble_to,
            st.wobble_ease,
            line_color,
            back_line_color,
        );

        // Corner blocks (2x2)
        host::draw_rect(x, y, 2.0, 2.0, line_color);
        host::draw_rect(x + w - 2.0, y, 2.0, 2.0, line_color);
        host::draw_rect(x, y + h - 2.0, 2.0, 2.0, line_color);
        host::draw_rect(x + w - 2.0, y + h - 2.0, 2.0, 2.0, line_color);
    });
}
