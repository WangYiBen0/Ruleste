//! Camera view into the level.
//!
//! Mirrors Celeste's player-follow camera (`Player.CameraTarget` +
//! `Level.Camera` smoothing): the view target is the player centered on a
//! 320×180 viewport plus the level's camera offset, clamped to the level
//! bounds, and the camera glides toward it with exponential smoothing.
//!
//! Also supports Celeste's `Camera.Shake(intensity, duration)` pattern: any
//! number of shakes can be queued (e.g. on player damage, on death, on a
//! collapsing bridge); each one adds a decaying random offset for its lifetime
//! and is dropped when it expires. `position` is the smoothed base plus the
//! current shake offset, so the renderer can consume it directly.

use ruleste_plugins_api::types::Vec2;

/// Internal rendering resolution in world units.
pub const VIEW_WIDTH: f32 = 320.0;
pub const VIEW_HEIGHT: f32 = 180.0;

/// One queued screen shake. `intensity` is the maximum offset in pixels (the
/// actual offset is `rand_unit() * intensity * (timer / duration)`); `timer`
/// counts down to 0, then the shake is dropped.
#[derive(Debug, Clone, Copy)]
struct Shake {
    timer: f32,
    duration: f32,
    intensity: f32,
}

#[derive(Debug, Clone)]
pub struct Camera {
    /// Smoothed base position (without shake). Updated by `update` toward the
    /// target; used by `target_at` consumers via `base_position`.
    base_position: Vec2,
    /// Top-left corner of the visible area in world units — `base_position` plus
    /// the current shake offset. The renderer reads this.
    pub position: Vec2,
    shakes: Vec<Shake>,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera::new()
    }
}

impl Camera {
    pub fn new() -> Camera {
        Camera {
            base_position: Vec2::ZERO,
            position: Vec2::ZERO,
            shakes: Vec::new(),
        }
    }

    /// The position the camera wants to be at for `player` to stay centered,
    /// clamped so the view never leaves the room's world bounds.
    ///
    /// Matches `Player.CameraTarget`: `player - (160, 90) + camera_offset`,
    /// clamped to `[room_origin, room_origin + room_size - 320×180]`.
    pub fn target_at(
        &self,
        player: Vec2,
        camera_offset: Vec2,
        room_origin: Vec2,
        room_size: Vec2,
    ) -> Vec2 {
        let mut tx = player.x - VIEW_WIDTH * 0.5 + camera_offset.x;
        let mut ty = player.y - VIEW_HEIGHT * 0.5 + camera_offset.y;
        let max_x = (room_origin.x + room_size.x - VIEW_WIDTH).max(room_origin.x);
        let max_y = (room_origin.y + room_size.y - VIEW_HEIGHT).max(room_origin.y);
        tx = tx.clamp(room_origin.x, max_x);
        ty = ty.clamp(room_origin.y, max_y);
        Vec2::new(tx, ty)
    }

    /// Returns the smoothed base position without the per-frame shake offset.
    #[must_use]
    pub fn base_position(&self) -> Vec2 {
        self.base_position
    }

    /// Teleports the camera (no smoothing), dropping any queued shakes. Use
    /// after a room switch so the next `update` glides from the new base.
    pub fn snap_to(&mut self, pos: Vec2) {
        self.base_position = pos;
        self.position = pos;
        self.shakes.clear();
    }

    /// Returns true while at least one shake is still active.
    #[must_use]
    pub fn shaking(&self) -> bool {
        !self.shakes.is_empty()
    }

    /// Queues a screen shake with the given peak `intensity` (pixels) and
    /// `duration` (seconds). Mirrors `Camera.Shake(intensity, duration)`.
    pub fn shake(&mut self, intensity: f32, duration: f32) {
        if duration <= 0.0 || intensity <= 0.0 {
            return;
        }
        self.shakes.push(Shake {
            timer: duration,
            duration,
            intensity,
        });
    }

    /// Glides toward the target with Celeste's exponential smoothing, advances
    /// any queued shakes, and sets `position = base + shake_offset` for the
    /// renderer to consume.
    pub fn update(&mut self, dt: f32, target: Vec2) {
        let t = 1.0 - 0.01f32.powf(dt.max(0.0));
        self.base_position.x += (target.x - self.base_position.x) * t;
        self.base_position.y += (target.y - self.base_position.y) * t;

        let mut offset_x = 0.0;
        let mut offset_y = 0.0;
        for s in &mut self.shakes {
            s.timer -= dt;
            if s.timer <= 0.0 || s.duration <= 0.0 {
                continue;
            }
            // Linear fade from peak at t=0 to 0 at t=duration, plus a random
            // unit step per axis (matches Celeste's shake feel).
            let strength = (s.timer / s.duration).clamp(0.0, 1.0) * s.intensity;
            offset_x += (rand_unit() - 0.5) * 2.0 * strength;
            offset_y += (rand_unit() - 0.5) * 2.0 * strength;
        }
        self.shakes.retain(|s| s.timer > 0.0);

        self.position = Vec2::new(
            self.base_position.x + offset_x,
            self.base_position.y + offset_y,
        );
    }
}

/// A cheap uniform random in `[0, 1)`. Uses a global xorshift so the camera
/// doesn't need RNG state. Celeste seeds its shake from `Calc.Random`, so this
/// is a stand-in — visually similar wobble without determinism guarantees.
fn rand_unit() -> f32 {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};
    thread_local! {
        static STATE: Cell<u64> = Cell::new({
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xdead_beef_cafe_babe)
                | 1
        });
    }
    STATE.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        // Top 24 bits -> [0, 1).
        ((x >> 40) as f32) / ((1u32 << 24) as f32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_centers_player_in_small_level() {
        // 320x184 room at the origin: camera is pinned to [0,0]x[0,4].
        let cam = Camera::new();
        let t = cam.target_at(
            Vec2::new(200.0, 100.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(320.0, 184.0),
        );
        assert_eq!(t.x, 0.0);
        assert_eq!(t.y, 4.0);
    }

    #[test]
    fn target_follows_player_in_wide_level() {
        // 640x184 room with camera offset +48: player at x=400 lands at x=288.
        let cam = Camera::new();
        let t = cam.target_at(
            Vec2::new(400.0, 92.0),
            Vec2::new(48.0, 0.0),
            Vec2::ZERO,
            Vec2::new(640.0, 184.0),
        );
        assert!((t.x - 288.0).abs() < 1e-4);
        assert!((t.y - 2.0).abs() < 1e-4);
    }

    #[test]
    fn target_clamps_to_level_bounds() {
        let cam = Camera::new();
        let t = cam.target_at(
            Vec2::new(600.0, 170.0),
            Vec2::new(48.0, 0.0),
            Vec2::ZERO,
            Vec2::new(640.0, 184.0),
        );
        assert_eq!(t.x, 320.0);
        assert_eq!(t.y, 4.0);
    }

    #[test]
    fn target_follows_into_offset_room() {
        // Room at world (320, 0): the camera must clamp to the room's world
        // bounds, not to [0, ...], so it actually moves with the player.
        let cam = Camera::new();
        let t = cam.target_at(
            Vec2::new(480.0, 92.0),
            Vec2::ZERO,
            Vec2::new(320.0, 0.0),
            Vec2::new(320.0, 184.0),
        );
        assert_eq!(t.x, 320.0);
        assert_eq!(t.y, 2.0);
    }

    #[test]
    fn update_glides_toward_target() {
        let mut cam = Camera::new();
        cam.update(1.0 / 60.0, Vec2::new(300.0, 80.0));
        assert!(cam.position.x > 0.0 && cam.position.x < 300.0);
        assert!(cam.position.y > 0.0 && cam.position.y < 80.0);
    }

    #[test]
    fn shake_offsets_position_and_decays() {
        let mut cam = Camera::new();
        cam.snap_to(Vec2::new(100.0, 50.0));
        cam.shake(5.0, 0.2);
        assert!(cam.shaking());

        // During shake, position should have a non-zero offset around base_position
        cam.update(0.05, Vec2::new(100.0, 50.0));
        assert!(cam.shaking());
        assert!((cam.position.x - 100.0).abs() <= 5.0 + 1e-4);
        assert!((cam.position.y - 50.0).abs() <= 5.0 + 1e-4);

        // After duration expires, shake clears and position matches base_position
        cam.update(0.2, Vec2::new(100.0, 50.0));
        assert!(!cam.shaking());
        assert!((cam.position.x - 100.0).abs() < 1e-4);
        assert!((cam.position.y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn snap_to_clears_shakes_and_resets_base() {
        let mut cam = Camera::new();
        cam.shake(10.0, 1.0);
        assert!(cam.shaking());
        cam.snap_to(Vec2::new(40.0, 80.0));
        assert!(!cam.shaking());
        assert_eq!(cam.base_position(), Vec2::new(40.0, 80.0));
        assert_eq!(cam.position, Vec2::new(40.0, 80.0));
    }
}
