#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `resortLantern` decoration plugin.
//!
//! Mirrors `Celeste.ResortLantern.cs`: a hanging lantern drawn from the
//! `objects/resortLantern/*` atlas frames (holder + a looping lantern glow).
//! It is purely decorative — its 8x8 hitbox only senses the player; bumping
//! it (while the player moves) starts a Monocle `Wiggler` that swings the
//! lantern by up to 30° with a decaying sine, mirroring the original timing
//! (period 2.5s, frequency 1.2).
//!
//! The lantern is flipped horizontally when there is solid ground directly to
//! its right (mirroring `Awake`'s `CollideCheck<Solid>`).

use ruleste_plugin_api::host::{self, draw_image_flipped, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("resortLantern");
ruleste_plugin_api::ruleste_entity_types!("resortLantern");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Player-sensing hitbox: 8x8 centered on the entity position.
const SENSE_W: f32 = 8.0;
const SENSE_H: f32 = 8.0;
/// Bump cooldown, mirroring `collideTimer` in `ResortLantern.cs`.
const BUMP_COOLDOWN: f32 = 0.5;
/// Max swing amplitude in degrees: `Value * mult * 30`.
const SWING_DEG: f32 = 30.0;
/// Wiggler constants (`Wiggler.Create(2.5f, 1.2f, ...)`).
const WIGGLE_DURATION: f32 = 2.5;
const WIGGLE_FREQUENCY: f32 = 1.2;
/// Lantern glow animation: `AddLoop("light", "lantern", 0.3f, 0, 0, 1, 2, 1)`.
const GLOW_FRAMES: [&str; 5] = [
    "lantern00",
    "lantern00",
    "lantern01",
    "lantern02",
    "lantern01",
];
const GLOW_DELAY: f32 = 0.3;

#[derive(Debug)]
struct LanternState {
    collide_timer: f32,
    /// Swing direction chosen at each bump (+/-1).
    mult: i32,
    /// Wiggler state.
    wiggle_counter: f32,
    wiggle_sine: f32,
    wiggling: bool,
    /// Mirrors `Awake`'s wall-flip check.
    flipped: bool,
    /// Glow animation frame.
    frame_timer: f32,
    frame_idx: usize,
    /// Tiny LCG so consecutive bumps alternate direction like the original's
    /// `Calc.Random.Choose(1, -1)`.
    rng: u32,
}

impl Default for LanternState {
    fn default() -> LanternState {
        LanternState {
            collide_timer: 0.0,
            mult: 1,
            wiggle_counter: 0.0,
            wiggle_sine: 0.0,
            wiggling: false,
            flipped: false,
            frame_timer: 0.0,
            frame_idx: 0,
            rng: 0x2f6e2b1,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<LanternState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut LanternState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, LanternState::default) as *mut LanternState;
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
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity
        .hitbox
        .set(SENSE_W, SENSE_H, -SENSE_W * 0.5, -SENSE_H * 0.5);
    entity.depth.set(2000);
    // `Awake`: flip the lantern when solid ground sits directly to the right.
    let flipped = entity.collision.check(8.0, 0.0);
    with_state(id, |st| {
        st.flipped = flipped;
        st.rng = id.wrapping_add(0x5eed);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);

        if st.collide_timer > 0.0 {
            st.collide_timer -= dt;
        }

        // Sense the player, mirroring `PlayerCollider(OnPlayer)`.
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let p = entity.position.get();
            let overlap = pp.x + pox < p.x + SENSE_W * 0.5
                && pp.x + pox + pw > p.x - SENSE_W * 0.5
                && pp.y + poy < p.y + SENSE_H * 0.5
                && pp.y + poy + ph > p.y - SENSE_H * 0.5;
            if overlap {
                let vel = host::Speed::new(player_id).get();
                if vel.x != 0.0 || vel.y != 0.0 {
                    if st.collide_timer <= 0.0 {
                        st.rng = st.rng.wrapping_mul(1664525).wrapping_add(1013904223);
                        st.mult = if (st.rng >> 16) & 1 == 1 { 1 } else { -1 };
                        st.wiggle_counter = 1.0;
                        st.wiggle_sine = std::f32::consts::FRAC_PI_2;
                        st.wiggling = true;
                        host::play_sound("event:/game/03_resort/lantern_bump");
                    }
                    st.collide_timer = BUMP_COOLDOWN;
                }
            }
            break;
        }

        // Advance the Wiggler (`Wiggler.Update`).
        if st.wiggling {
            let increment = 1.0 / WIGGLE_DURATION;
            let sine_add = std::f32::consts::TAU * WIGGLE_FREQUENCY;
            st.wiggle_sine += sine_add * dt;
            st.wiggle_counter -= increment * dt;
            if st.wiggle_counter <= 0.0 {
                st.wiggle_counter = 0.0;
                st.wiggling = false;
            }
        }

        // Advance the lantern glow loop.
        st.frame_timer += dt;
        while st.frame_timer >= GLOW_DELAY {
            st.frame_timer -= GLOW_DELAY;
            st.frame_idx = (st.frame_idx + 1) % GLOW_FRAMES.len();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let flip = st.flipped;
        // `Wiggler.Value = cos(sineCounter) * Counter`.
        let value = if st.wiggling {
            (st.wiggle_sine.cos() * st.wiggle_counter).max(0.0)
        } else {
            0.0
        };
        let rotation = value * st.mult as f32 * SWING_DEG;

        draw_image_flipped(
            "objects/resortLantern/holder",
            p.x,
            p.y,
            0.0,
            1.0,
            1.0,
            flip,
            false,
        );

        // Lantern hangs below-left of the holder center; flipped it shifts
        // 2px right (`lantern.X += 2f`), and its glow is offset by 1px.
        let lx = p.x - 1.0 + if flip { 2.0 } else { 0.0 };
        let ly = p.y - 5.0;
        draw_image_flipped(
            &format!("objects/resortLantern/{}", GLOW_FRAMES[st.frame_idx]),
            lx,
            ly,
            rotation,
            1.0,
            1.0,
            flip,
            false,
        );
    });
}
