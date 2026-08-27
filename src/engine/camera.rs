//! Camera view into the level.
//!
//! Mirrors Celeste's player-follow camera (`Player.CameraTarget` +
//! `Level.Camera` smoothing): the view target is the player centered on a
//! 320×180 viewport plus the level's camera offset, clamped to the level
//! bounds, and the camera glides toward it with exponential smoothing.

use ruleste_plugins_api::types::Vec2;

/// Internal rendering resolution in world units.
pub const VIEW_WIDTH: f32 = 320.0;
pub const VIEW_HEIGHT: f32 = 180.0;

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Top-left corner of the visible area, in world units.
    pub position: Vec2,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera::new()
    }
}

impl Camera {
    pub fn new() -> Camera {
        Camera {
            position: Vec2::ZERO,
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

    /// Glide toward the target with Celeste's exponential smoothing:
    /// `camera += (target - camera) * (1 - 0.01^dt)`.
    pub fn update(&mut self, dt: f32, target: Vec2) {
        let t = 1.0 - 0.01f32.powf(dt.max(0.0));
        self.position.x += (target.x - self.position.x) * t;
        self.position.y += (target.y - self.position.y) * t;
    }
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
}
