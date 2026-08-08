//! Grid-based collision, mirroring the original `Grid` collider and
//! `Actor.MoveH/MoveV`. Tile size is 8x8 world units.

use crate::engine::ecs::World;

pub const TILE: f32 = 8.0;

/// Bit flags returned by [`SolidGrid::actor_move`], matching
/// `ActorMoveResult` in the plugin API.
pub const MOVE_GROUND: u32 = 1;
pub const MOVE_WALL_LEFT: u32 = 2;
pub const MOVE_WALL_RIGHT: u32 = 4;
pub const MOVE_CEILING: u32 = 8;

#[derive(Debug, Clone)]
pub struct JumpThru {
    /// Top-left corner and size in world units. The top edge `y` is the
    /// standing surface; the thin hitbox is `(width, 5)` like the original.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone)]
pub struct SolidGrid {
    pub width: usize,
    pub height: usize,
    solid: Vec<bool>,
    tile_ids: Vec<Option<char>>,
    /// One-way platforms: solid from above, passable from below/through.
    jumpthru: Vec<JumpThru>,
}

impl SolidGrid {
    pub fn from_rows(rows: &[&str]) -> SolidGrid {
        let height = rows.len();
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut solid = vec![false; width * height];
        let mut tile_ids = vec![None; width * height];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let idx = y * width + x;
                if ch != '0' {
                    solid[idx] = true;
                    tile_ids[idx] = Some(ch);
                }
            }
        }
        SolidGrid {
            width,
            height,
            solid,
            tile_ids,
            jumpthru: Vec::new(),
        }
    }

    /// Registers a one-way platform region.
    pub fn add_jumpthru(&mut self, jt: JumpThru) {
        self.jumpthru.push(jt);
    }

    /// True when the top edge of a jump-thru platform lies within `slop`
    /// below the given horizontal span, i.e. the entity can stand on it.
    pub fn jumpthru_top(&self, x: f32, y: f32, w: f32) -> bool {
        self.jumpthru
            .iter()
            .any(|jt| (y - jt.y).abs() <= 0.5 && x < jt.x + jt.w && x + w > jt.x)
    }

    /// Returns the `y` of the jump-thru top surface the horizontal span is
    /// about to cross while moving down, if any.
    fn jumpthru_landing(&self, x: f32, bottom: f32, w: f32, step: f32) -> Option<f32> {
        self.jumpthru.iter().find_map(|jt| {
            let top = jt.y;
            if x < jt.x + jt.w && x + w > jt.x && top >= bottom && top <= bottom + step + 0.01 {
                Some(top)
            } else {
                None
            }
        })
    }

    pub fn solid_at(&self, tx: i32, ty: i32) -> bool {
        if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
            return true;
        }
        self.solid[ty as usize * self.width + tx as usize]
    }

    /// Returns the tile character at `(tx, ty)`, or `None` if empty / out of bounds.
    pub fn tile_id_at(&self, tx: i32, ty: i32) -> Option<char> {
        if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
            return None;
        }
        self.tile_ids[ty as usize * self.width + tx as usize]
    }

    #[must_use]
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// True when the axis-aligned rect (in world units) overlaps a solid tile.
    pub fn collide_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if w <= 0.0 || h <= 0.0 {
            return false;
        }
        let x0 = (x / TILE).floor() as i32;
        let y0 = (y / TILE).floor() as i32;
        let x1 = ((x + w - f32::EPSILON) / TILE).floor() as i32;
        let y1 = ((y + h - f32::EPSILON) / TILE).floor() as i32;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if self.solid_at(tx, ty) {
                    return true;
                }
            }
        }
        false
    }

    /// Moves an entity by `(dx, dy)` resolving against solids, in the spirit of
    /// `Actor.MoveH`/`MoveV`. Horizontal and vertical movement are applied
    /// separately; on collision the entity snaps to the tile boundary.
    ///
    /// Returns the flags describing what was hit.
    pub fn actor_move(&self, world: &mut World, id: u32, dx: f32, dy: f32) -> u32 {
        let mut flags = 0;
        if !world.is_alive(id) {
            return flags;
        }
        let e = world.get(id).expect("alive");
        let (ox, oy, w, h) = (e.hitbox_offset.x, e.hitbox_offset.y, e.hitbox.x, e.hitbox.y);

        if dx != 0.0 {
            let sign = dx.signum();
            let target = e.position.x + dx;
            let mut cur = e.position.x;
            loop {
                let remaining = (target - cur).abs();
                if remaining <= 0.0 {
                    break;
                }
                let step = remaining.min(1.0);
                let nx = cur + sign * step;
                if self.collide_rect(nx + ox, e.position.y + oy, w, h) {
                    cur = if sign > 0.0 {
                        ((nx + ox + w) / TILE).floor() * TILE - w - ox
                    } else {
                        ((nx + ox) / TILE).ceil() * TILE - ox
                    };
                    flags |= if sign > 0.0 {
                        MOVE_WALL_RIGHT
                    } else {
                        MOVE_WALL_LEFT
                    };
                    break;
                }
                cur = nx;
            }
            world.get_mut(id).expect("alive").position.x = cur;
        }

        if world.is_alive(id) && dy != 0.0 {
            let pos = world.get(id).expect("alive").position;
            let sign = dy.signum();
            let target = pos.y + dy;
            let mut cur = pos.y;
            loop {
                let remaining = (target - cur).abs();
                if remaining <= 0.0 {
                    break;
                }
                let step = remaining.min(1.0);
                let ny = cur + sign * step;
                if self.collide_rect(pos.x + ox, ny + oy, w, h) {
                    cur = if sign > 0.0 {
                        ((ny + oy + h) / TILE).floor() * TILE - h - oy
                    } else {
                        ((ny + oy) / TILE).ceil() * TILE - oy
                    };
                    flags |= if sign > 0.0 {
                        MOVE_GROUND
                    } else {
                        MOVE_CEILING
                    };
                    break;
                }
                // One-way platforms: landing on top while falling.
                if sign > 0.0 {
                    if let Some(top) = self.jumpthru_landing(pos.x + ox, ny + oy + h, w, step) {
                        cur = top - oy - h;
                        flags |= MOVE_GROUND;
                        break;
                    }
                }
                cur = ny;
            }
            world.get_mut(id).expect("alive").position.y = cur;
        }

        flags
    }

    /// True when the entity's hitbox rests on solid ground or a jump-thru
    /// platform top surface.
    pub fn is_grounded(&self, world: &World, id: u32) -> bool {
        match world.get(id) {
            Some(e) => {
                let x = e.position.x + e.hitbox_offset.x;
                let y = e.position.y + e.hitbox_offset.y + e.hitbox.y;
                self.collide_rect(x, y + 0.5, e.hitbox.x, 1.0)
                    || self.jumpthru_top(x, y, e.hitbox.x)
            }
            None => false,
        }
    }

    /// True when the entity's hitbox, offset by `(dx, dy)` in world units,
    /// overlaps a solid tile.
    pub fn entity_collide(&self, world: &World, id: u32, dx: f32, dy: f32) -> bool {
        match world.get(id) {
            Some(e) => self.collide_rect(
                e.position.x + e.hitbox_offset.x + dx,
                e.position.y + e.hitbox_offset.y + dy,
                e.hitbox.x,
                e.hitbox.y,
            ),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruleste_plugin_api::types::Vec2;

    fn grid_with_jumpthru() -> SolidGrid {
        // 16x20 empty tile grid (128x160 px), platform top at y=120.
        let rows = vec!["0".repeat(16); 20];
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut g = SolidGrid::from_rows(&rows);
        g.add_jumpthru(JumpThru {
            x: 40.0,
            y: 120.0,
            w: 32.0,
            h: 5.0,
        });
        g
    }

    #[test]
    fn actor_move_stops_on_jumpthru_top() {
        let mut world = World::new();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.position = Vec2::new(48.0, 100.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let g = grid_with_jumpthru();
        // Fall straight down far enough to cross the platform top (y=120).
        let flags = g.actor_move(&mut world, id, 0.0, 60.0);
        assert_ne!(flags & MOVE_GROUND, 0, "expected ground hit on jumpthru");
        let e = world.get(id).unwrap();
        // Player bottom must rest exactly on the platform top: y + h == 120.
        assert!(
            (e.position.y + 11.0 - 120.0).abs() < 0.01,
            "bottom={}",
            e.position.y + 11.0
        );
    }

    #[test]
    fn actor_move_passes_through_jumpthru_from_below() {
        let mut world = World::new();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.position = Vec2::new(48.0, 140.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let g = grid_with_jumpthru();
        // Moving up through the platform must not collide.
        let flags = g.actor_move(&mut world, id, 0.0, -60.0);
        assert_eq!(
            flags & MOVE_CEILING,
            0,
            "jumpthru must not block upward motion"
        );
        let e = world.get(id).unwrap();
        assert!((e.position.y - 80.0).abs() < 0.01);
    }

    #[test]
    fn is_grounded_true_on_jumpthru() {
        let mut world = World::new();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.position = Vec2::new(48.0, 109.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let g = grid_with_jumpthru();
        assert!(g.is_grounded(&world, id));
    }

    #[test]
    fn not_grounded_below_jumpthru() {
        let mut world = World::new();
        let id = world.spawn();
        {
            let e = world.get_mut(id).unwrap();
            e.position = Vec2::new(48.0, 130.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let g = grid_with_jumpthru();
        assert!(!g.is_grounded(&world, id));
    }
}
