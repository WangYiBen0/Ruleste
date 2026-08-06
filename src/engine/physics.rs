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
pub struct SolidGrid {
    pub width: usize,
    pub height: usize,
    solid: Vec<bool>,
}

impl SolidGrid {
    pub fn from_rows(rows: &[&str]) -> SolidGrid {
        let height = rows.len();
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut solid = vec![false; width * height];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                // The autotiler encodes empty tiles as '0'; anything else is
                // solid by default. The real per-tile solidity is refined once
                // the autotiler is implemented.
                solid[y * width + x] = ch != '0';
            }
        }
        SolidGrid {
            width,
            height,
            solid,
        }
    }

    pub fn solid_at(&self, tx: i32, ty: i32) -> bool {
        if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
            return true;
        }
        self.solid[ty as usize * self.width + tx as usize]
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
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = (x + w - f32::EPSILON).floor() as i32;
        let y1 = (y + h - f32::EPSILON).floor() as i32;
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
                cur = ny;
            }
            world.get_mut(id).expect("alive").position.y = cur;
        }

        flags
    }

    /// True when the entity's hitbox rests on solid ground.
    pub fn is_grounded(&self, world: &World, id: u32) -> bool {
        match world.get(id) {
            Some(e) => self.collide_rect(
                e.position.x + e.hitbox_offset.x,
                e.position.y + e.hitbox_offset.y + e.hitbox.y + 0.5,
                e.hitbox.x,
                1.0,
            ),
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
