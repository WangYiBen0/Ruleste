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
    /// World-space position of tile `(0, 0)`, in pixels. Collision queries use
    /// world coordinates, so tile indices are translated by this origin before
    /// indexing the local grid. A composited level-wide grid carries the
    /// level's minimum corner; a per-room grid uses `(0, 0)`.
    pub origin_x: f32,
    pub origin_y: f32,
}

impl SolidGrid {
    /// An empty `width`×`height` grid anchored at world `(0, 0)`.
    pub fn empty(width: usize, height: usize) -> SolidGrid {
        SolidGrid {
            width,
            height,
            solid: vec![false; width * height],
            tile_ids: vec![None; width * height],
            jumpthru: Vec::new(),
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }

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
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }

    /// Copies `src`'s solid tiles and tile ids into this grid at local offset
    /// `(dx, dy)`, and appends `src`'s jump-thrus (which are already stored in
    /// world coordinates). Used to composite per-room grids into one level-wide
    /// grid.
    pub fn blit(&mut self, src: &SolidGrid, dx: i32, dy: i32) {
        for ty in 0..src.height as i32 {
            for tx in 0..src.width as i32 {
                let (ex, ey) = (dx + tx, dy + ty);
                if ex < 0 || ey < 0 || ex >= self.width as i32 || ey >= self.height as i32 {
                    continue;
                }
                let sidx = ty as usize * src.width + tx as usize;
                let didx = ey as usize * self.width + ex as usize;
                self.solid[didx] = src.solid[sidx];
                self.tile_ids[didx] = src.tile_ids[sidx];
            }
        }
        for jt in &src.jumpthru {
            self.jumpthru.push(jt.clone());
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

    /// True when the top surface of a dynamic solid-platform entity lies
    /// within `slop` below the horizontal span `(x, x+w)` at height `y`.
    fn platform_top(&self, world: &World, x: f32, y: f32, w: f32, exclude: u32) -> bool {
        self.platform_top_y(world, x, y, w, exclude).is_some()
    }

    /// Same, but returns the platform's top `y`.
    fn platform_top_y(&self, world: &World, x: f32, y: f32, w: f32, exclude: u32) -> Option<f32> {
        world
            .solid_platforms
            .iter()
            .chain(world.solid_entities.iter())
            .filter(|&&pid| pid != exclude && world.is_alive(pid))
            .find_map(|&pid| {
                let e = world.get(pid).expect("alive platform");
                let top = e.position.y + e.hitbox_offset.y;
                let px = e.position.x + e.hitbox_offset.x;
                let pw = e.hitbox.x;
                ((y - top).abs() <= 0.5 && x < px + pw && x + w > px).then_some(top)
            })
    }

    /// Returns the top `y` of a dynamic platform the horizontal span would
    /// land on while falling across `[bottom, bottom + step]`, if any.
    fn platform_landing(
        &self,
        world: &World,
        exclude: u32,
        x: f32,
        bottom: f32,
        w: f32,
        step: f32,
    ) -> Option<f32> {
        world
            .solid_platforms
            .iter()
            .filter(|&&pid| pid != exclude && world.is_alive(pid))
            .find_map(|&pid| {
                let e = world.get(pid).expect("alive platform");
                let top = e.position.y + e.hitbox_offset.y;
                let px = e.position.x + e.hitbox_offset.x;
                let pw = e.hitbox.x;
                if x < px + pw && x + w > px && top >= bottom && top <= bottom + step + 0.01 {
                    Some(top)
                } else {
                    None
                }
            })
    }

    /// Marks an entity as a standable dynamic platform (or unmarks it).
    pub fn mark_solid_platform(world: &mut World, id: u32, on: bool) {
        if on {
            world.solid_platforms.insert(id);
        } else {
            world.solid_platforms.remove(&id);
        }
    }

    /// Marks an entity as a fully solid block (or unmarks it): actors collide
    /// with every face, stand on top, and ride along with its movement.
    pub fn mark_solid_entity(world: &mut World, id: u32, on: bool) {
        if on {
            world.solid_entities.insert(id);
        } else {
            world.solid_entities.remove(&id);
        }
    }

    /// The hitbox of a solid entity overlapping `(x, y, w, h)`, if any.
    /// `exclude` skips an entity (itself, when it moves through `actor_move`).
    fn solid_entity_hitbox(
        &self,
        world: &World,
        exclude: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        world
            .solid_entities
            .iter()
            .filter(|&&id| id != exclude && world.is_alive(id))
            .find_map(|&id| {
                let e = world.get(id).expect("alive solid entity");
                let (sx, sy) = (
                    e.position.x + e.hitbox_offset.x,
                    e.position.y + e.hitbox_offset.y,
                );
                let (sw, sh) = (e.hitbox.x, e.hitbox.y);
                (x < sx + sw && x + w > sx && y < sy + sh && y + h > sy).then_some((sx, sy, sw, sh))
            })
    }

    /// Every other entity whose bottom edge rests on this platform's top
    /// surface (they get carried along with it).
    fn platform_riders(&self, world: &World, platform_id: u32) -> Vec<u32> {
        let Some(p) = world.get(platform_id) else {
            return Vec::new();
        };
        let top = p.position.y + p.hitbox_offset.y;
        let px = p.position.x + p.hitbox_offset.x;
        let pw = p.hitbox.x;
        world
            .iter()
            .filter(|e| {
                e.id != platform_id
                    && (e.position.y + e.hitbox_offset.y + e.hitbox.y - top).abs() <= 0.5
                    && e.position.x + e.hitbox_offset.x < px + pw
                    && e.position.x + e.hitbox_offset.x + e.hitbox.x > px
            })
            .map(|e| e.id)
            .collect()
    }

    /// Moves an entity by `(dx, dy)` in world units without any collision.
    fn shift(&self, world: &mut World, id: u32, dx: f32, dy: f32) {
        if let Some(e) = world.get_mut(id) {
            e.position.x += dx;
            e.position.y += dy;
        }
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
        let ltx = tx - (self.origin_x / TILE).round() as i32;
        let lty = ty - (self.origin_y / TILE).round() as i32;
        if ltx < 0 || lty < 0 || ltx >= self.width as i32 || lty >= self.height as i32 {
            return true;
        }
        self.solid[lty as usize * self.width + ltx as usize]
    }

    /// Returns true when the straight line from `(x1, y1)` to `(x2, y2)`
    /// (world units) is unobstructed by solid tiles — a coarse grid-stepped
    /// sight test used for AI line-of-sight (e.g. `Seeker.CanSeePlayer`).
    /// Off-grid samples count as blocking.
    #[must_use]
    pub fn line_of_sight(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dist = (dx * dx + dy * dy).sqrt();
        // Step at half a tile so we can't skip a thin wall.
        let steps = (dist / (TILE * 0.5)).ceil().max(1.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = x1 + dx * t;
            let y = y1 + dy * t;
            if self.solid_at((x / TILE).round() as i32, (y / TILE).round() as i32) {
                return false;
            }
        }
        true
    }

    /// Returns the tile character at `(tx, ty)`, or `None` if empty / out of bounds.
    pub fn tile_id_at(&self, tx: i32, ty: i32) -> Option<char> {
        let ltx = tx - (self.origin_x / TILE).round() as i32;
        let lty = ty - (self.origin_y / TILE).round() as i32;
        if ltx < 0 || lty < 0 || ltx >= self.width as i32 || lty >= self.height as i32 {
            return None;
        }
        self.tile_ids[lty as usize * self.width + ltx as usize]
    }

    /// Like [`SolidGrid::tile_id_at`] but `tx`/`ty` are already local grid
    /// indices (no world-origin translation). Used by the autotiler, which
    /// operates purely on a grid's own tiles.
    pub fn tile_id_at_local(&self, tx: i32, ty: i32) -> Option<char> {
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
    ///
    /// The far edge is exclusive: a rect resting exactly flush against a tile
    /// boundary (e.g. the player standing on top of a solid tile) does not
    /// collide with the tile on the other side. The epsilon must scale with
    /// the coordinate magnitude — `f32::EPSILON` alone is smaller than the
    /// float ULP at tile-scale coordinates (e.g. at y=136 the ULP is ~1.5e-5),
    /// so `136 - EPSILON` rounds back to `136.0` and the ground tile sneaks in.
    pub fn collide_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        if w <= 0.0 || h <= 0.0 {
            return false;
        }
        let eps_x = f32::EPSILON * x.abs().max(w).max(TILE) * 4.0;
        let eps_y = f32::EPSILON * y.abs().max(h).max(TILE) * 4.0;
        let x0 = (x / TILE).floor() as i32;
        let y0 = (y / TILE).floor() as i32;
        let x1 = ((x + w - eps_x) / TILE).floor() as i32;
        let y1 = ((y + h - eps_y) / TILE).floor() as i32;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if self.solid_at(tx, ty) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if the given circle (centre `cx`/`cy`, radius `r`) overlaps
    /// any solid tile in this grid. Uses a bounding-box prefilter (every tile
    /// whose bounding box intersects the circle's AABB is checked); the per-tile test
    /// uses the closest-point-on-AABB-to-centre distance.
    ///
    /// Mirrors `CircleCollider.Check(Rect)` / `Grid.Check` for circles.
    #[must_use]
    pub fn collide_circle(&self, cx: f32, cy: f32, r: f32) -> bool {
        if r <= 0.0 {
            return false;
        }
        let x0 = ((cx - r) / TILE).floor() as i32;
        let y0 = ((cy - r) / TILE).floor() as i32;
        let x1 = ((cx + r) / TILE).ceil() as i32;
        let y1 = ((cy + r) / TILE).ceil() as i32;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if !self.solid_at(tx, ty) {
                    continue;
                }
                let tx0 = (tx as f32) * TILE;
                let ty0 = (ty as f32) * TILE;
                let cx_tile = cx.clamp(tx0, tx0 + TILE);
                let cy_tile = cy.clamp(ty0, ty0 + TILE);
                let dx = cx - cx_tile;
                let dy = cy - cy_tile;
                if dx * dx + dy * dy <= r * r {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if a circle overlaps an AABB at `(x, y, w, h)`.
    /// Mirrors `CircleToRect` in the original `Collide` class.
    #[must_use]
    pub fn circle_to_rect(cx: f32, cy: f32, r: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
        if r <= 0.0 || w <= 0.0 || h <= 0.0 {
            return false;
        }
        let cx_closest = cx.clamp(x, x + w);
        let cy_closest = cy.clamp(y, y + h);
        let dx = cx - cx_closest;
        let dy = cy - cy_closest;
        dx * dx + dy * dy <= r * r
    }

    /// Returns true if two circles overlap.
    #[must_use]
    pub fn circle_to_circle(cx1: f32, cy1: f32, r1: f32, cx2: f32, cy2: f32, r2: f32) -> bool {
        if r1 <= 0.0 || r2 <= 0.0 {
            return false;
        }
        let dx = cx1 - cx2;
        let dy = cy1 - cy2;
        let min_dist = r1 + r2;
        dx * dx + dy * dy <= min_dist * min_dist
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
            let riders_before =
                if world.solid_platforms.contains(&id) || world.solid_entities.contains(&id) {
                    self.platform_riders(world, id)
                } else {
                    Vec::new()
                };
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
                if let Some((sx, _, sw, _)) =
                    self.solid_entity_hitbox(world, id, nx + ox, e.position.y + oy, w, h)
                {
                    cur = if sign > 0.0 {
                        sx - w - ox
                    } else {
                        sx + sw - ox
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
            let moved = cur - e.position.x;
            world.get_mut(id).expect("alive").position.x = cur;
            if moved != 0.0 {
                for rider in &riders_before {
                    self.shift(world, *rider, moved, 0.0);
                }
            }
        }

        if world.is_alive(id) && dy != 0.0 {
            let riders_before =
                if world.solid_platforms.contains(&id) || world.solid_entities.contains(&id) {
                    self.platform_riders(world, id)
                } else {
                    Vec::new()
                };
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
                // Fully solid blocks: land on their top, bump their underside.
                if sign > 0.0 {
                    if let Some((_, sy, _, _)) =
                        self.solid_entity_hitbox(world, id, pos.x + ox, ny + oy, w, h)
                    {
                        cur = sy - oy - h;
                        flags |= MOVE_GROUND;
                        break;
                    }
                } else if let Some((_, sy, _, sh)) =
                    self.solid_entity_hitbox(world, id, pos.x + ox, ny + oy, w, h)
                {
                    cur = sy + sh - oy;
                    flags |= MOVE_CEILING;
                    break;
                }
                // One-way platforms: landing on top while falling.
                if sign > 0.0 {
                    if let Some(top) = self.jumpthru_landing(pos.x + ox, ny + oy + h, w, step) {
                        cur = top - oy - h;
                        flags |= MOVE_GROUND;
                        break;
                    }
                    // Dynamic solid platforms behave like one-way platforms.
                    if let Some(top) =
                        self.platform_landing(world, id, pos.x + ox, ny + oy + h, w, step)
                    {
                        cur = top - oy - h;
                        flags |= MOVE_GROUND;
                        break;
                    }
                }
                cur = ny;
            }
            let moved = cur - pos.y;
            world.get_mut(id).expect("alive").position.y = cur;
            if moved != 0.0 {
                for rider in &riders_before {
                    self.shift(world, *rider, 0.0, moved);
                }
            }
        }

        flags
    }

    /// True when the entity's hitbox rests on solid ground, a jump-thru
    /// platform top surface, or a dynamic solid-platform entity.
    pub fn is_grounded(&self, world: &World, id: u32) -> bool {
        match world.get(id) {
            Some(e) => {
                let x = e.position.x + e.hitbox_offset.x;
                let y = e.position.y + e.hitbox_offset.y + e.hitbox.y;
                self.collide_rect(x, y + 0.5, e.hitbox.x, 1.0)
                    || self.jumpthru_top(x, y, e.hitbox.x)
                    || self.platform_top(world, x, y, e.hitbox.x, id)
            }
            None => false,
        }
    }

    /// True when the entity's hitbox, offset by `(dx, dy)` in world units,
    /// overlaps a solid tile or a fully solid entity.
    pub fn entity_collide(&self, world: &World, id: u32, dx: f32, dy: f32) -> bool {
        match world.get(id) {
            Some(e) => {
                let x = e.position.x + e.hitbox_offset.x + dx;
                let y = e.position.y + e.hitbox_offset.y + dy;
                self.collide_rect(x, y, e.hitbox.x, e.hitbox.y)
                    || self
                        .solid_entity_hitbox(world, id, x, y, e.hitbox.x, e.hitbox.y)
                        .is_some()
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruleste_plugins_api::types::Vec2;

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

    fn platform_world() -> (SolidGrid, World, u32, u32) {
        let rows = vec!["0".repeat(16); 24];
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let g = SolidGrid::from_rows(&rows);
        let mut world = World::new();
        // Dynamic platform: 32x4 hitbox at (40, 120) with offset (0,-4), so
        // its top surface is y = 116.
        let platform = world.spawn();
        {
            let e = world.get_mut(platform).unwrap();
            e.position = Vec2::new(40.0, 120.0);
            e.hitbox = Vec2::new(32.0, 4.0);
            e.hitbox_offset = Vec2::new(0.0, -4.0);
        }
        SolidGrid::mark_solid_platform(&mut world, platform, true);
        // A rider falling from above (bottom = 111 initially).
        let rider = world.spawn();
        {
            let e = world.get_mut(rider).unwrap();
            e.position = Vec2::new(52.0, 100.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        (g, world, platform, rider)
    }

    #[test]
    fn actor_lands_on_dynamic_platform_top() {
        let (g, mut world, platform, rider) = platform_world();
        // Move the rider down through the platform top (y=116).
        let flags = g.actor_move(&mut world, rider, 0.0, 40.0);
        assert_ne!(flags & MOVE_GROUND, 0, "expected ground hit on platform");
        let e = world.get(rider).unwrap();
        assert!(
            (e.position.y + 11.0 - 116.0).abs() < 0.01,
            "bottom={}",
            e.position.y + 11.0
        );
        let _ = platform;
    }

    #[test]
    fn platform_movement_carries_rider() {
        let (g, mut world, platform, rider) = platform_world();
        // First land the rider on the platform, then move the platform up.
        g.actor_move(&mut world, rider, 0.0, 40.0);
        g.actor_move(&mut world, platform, 0.0, -10.0);
        let e = world.get(rider).unwrap();
        assert!(
            (e.position.y - 95.0).abs() < 0.01,
            "rider y={}",
            e.position.y
        );
    }

    #[test]
    fn platform_movement_carries_rider_horizontally() {
        let (g, mut world, platform, rider) = platform_world();
        g.actor_move(&mut world, rider, 0.0, 40.0);
        g.actor_move(&mut world, platform, 8.0, 0.0);
        let e = world.get(rider).unwrap();
        assert!(
            (e.position.x - 60.0).abs() < 0.01,
            "rider x={}",
            e.position.x
        );
    }

    #[test]
    fn is_grounded_true_on_dynamic_platform() {
        let (g, mut world, _platform, rider) = platform_world();
        g.actor_move(&mut world, rider, 0.0, 40.0);
        assert!(g.is_grounded(&world, rider));
    }

    #[test]
    fn not_grounded_when_platform_moves_away() {
        let (g, mut world, platform, rider) = platform_world();
        // Platform moves down far enough that the rider no longer rests on it.
        g.actor_move(&mut world, platform, 0.0, 20.0);
        assert!(!g.is_grounded(&world, rider));
    }

    /// A solid-block world: a 16x16 block at (40, 100) and a rider that can
    /// approach its left, top, and bottom faces.
    fn solid_world() -> (SolidGrid, World, u32, u32) {
        let rows = vec!["0".repeat(16); 24];
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let g = SolidGrid::from_rows(&rows);
        let mut world = World::new();
        // Solid block: 16x16 at (40, 100), top surface y = 100.
        let block = world.spawn();
        {
            let e = world.get_mut(block).unwrap();
            e.position = Vec2::new(40.0, 100.0);
            e.hitbox = Vec2::new(16.0, 16.0);
            e.hitbox_offset = Vec2::ZERO;
        }
        SolidGrid::mark_solid_entity(&mut world, block, true);
        // Rider: 8x11 with offset (-4, 0), so its left edge = x - 4.
        let rider = world.spawn();
        {
            let e = world.get_mut(rider).unwrap();
            e.position = Vec2::new(20.0, 100.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        (g, world, block, rider)
    }

    #[test]
    fn solid_block_blocks_rider_from_the_side() {
        let (g, mut world, _block, rider) = solid_world();
        // Rider's right edge (x + 4) starts at 24; push it into the block's
        // left edge (x = 40). It should stop at x = 36 (right edge = 40).
        let flags = g.actor_move(&mut world, rider, 60.0, 0.0);
        assert_ne!(
            flags & MOVE_WALL_RIGHT,
            0,
            "expected wall hit on block side"
        );
        let e = world.get(rider).unwrap();
        assert!(
            (e.position.x - 36.0).abs() < 0.01,
            "rider x={}",
            e.position.x
        );
    }

    #[test]
    fn solid_block_lands_rider_on_top() {
        let (g, mut world, block, _rider) = solid_world();
        // Spawn a rider above the block's top (y = 100) and drop it down.
        let faller = world.spawn();
        {
            let e = world.get_mut(faller).unwrap();
            e.position = Vec2::new(48.0, 80.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let _ = block;
        let flags = g.actor_move(&mut world, faller, 0.0, 30.0);
        assert_ne!(flags & MOVE_GROUND, 0, "expected landing on block top");
        let e = world.get(faller).unwrap();
        assert!(
            (e.position.y + 11.0 - 100.0).abs() < 0.01,
            "faller bottom={}",
            e.position.y + 11.0
        );
    }

    #[test]
    fn solid_block_blocks_rider_from_below() {
        let (g, mut world, block, _rider) = solid_world();
        // Spawn a rider below the block and push it up into the block bottom
        // (y = 116). It should stop with its top edge at the block bottom.
        let jumper = world.spawn();
        {
            let e = world.get_mut(jumper).unwrap();
            e.position = Vec2::new(48.0, 130.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        let _ = block;
        let flags = g.actor_move(&mut world, jumper, 0.0, -30.0);
        assert_ne!(flags & MOVE_CEILING, 0, "expected ceiling hit on block");
        let e = world.get(jumper).unwrap();
        assert!(
            (e.position.y - 116.0).abs() < 0.01,
            "jumper top={}",
            e.position.y
        );
    }

    #[test]
    fn solid_block_carries_rider_on_top() {
        let (g, mut world, block, _rider) = solid_world();
        // Land a rider on the block, then move the block horizontally: the
        // rider must be carried along (its x changes by the same amount).
        let stander = world.spawn();
        {
            let e = world.get_mut(stander).unwrap();
            e.position = Vec2::new(48.0, 80.0);
            e.hitbox = Vec2::new(8.0, 11.0);
            e.hitbox_offset = Vec2::new(-4.0, 0.0);
        }
        g.actor_move(&mut world, stander, 0.0, 30.0);
        // Rider should now be on the block top (y + 11 == 100).
        assert!(g.is_grounded(&world, stander));
        g.actor_move(&mut world, block, 5.0, 0.0);
        let e = world.get(stander).unwrap();
        assert!(
            (e.position.x - 53.0).abs() < 0.01,
            "stander x={} (should carry with block)",
            e.position.x
        );
    }

    #[test]
    fn solid_block_is_grounded_when_standing_on_top() {
        let (g, mut world, _block, rider) = solid_world();
        // Move the rider up onto the block top: first approach from the left,
        // then have it stand on top by setting position.
        {
            let e = world.get_mut(rider).unwrap();
            e.position = Vec2::new(48.0, 89.0);
        }
        g.actor_move(&mut world, rider, 0.0, 30.0);
        assert!(
            g.is_grounded(&world, rider),
            "rider standing on solid top should be grounded"
        );
    }

    #[test]
    fn entity_collide_detects_solid_entity_side() {
        // Mirror `ClimbCheck`: a rider facing a solid block must report a
        // collision when probing `CLIMB_CHECK_DIST` (2 px) ahead, so the
        // player can grab the slab and climb up its side.
        let (g, mut world, _block, rider) = solid_world();
        // Place the rider flush against the block's left face (right edge at
        // x = 40), so probing 2 px to the right overlaps the block.
        {
            let e = world.get_mut(rider).unwrap();
            e.position = Vec2::new(36.0, 100.0);
        }
        assert!(
            g.entity_collide(&world, rider, 2.0, 0.0),
            "climb probe into a solid block's side should collide"
        );
        assert!(
            !g.entity_collide(&world, rider, -2.0, 0.0),
            "probing away from the block should not collide"
        );
    }

    /// A 3-tile wide, 1-tile tall solid floor at y=64..72 (tile row 8..9), with
    /// no walls or platforms around it.
    fn floor_grid() -> SolidGrid {
        let mut rows: Vec<String> = (0..20).map(|_| "0".repeat(16)).collect();
        // Solid at row 8, columns 0..2 only (x in 0..24, y in 64..72).
        for (i, r) in rows.iter_mut().enumerate() {
            if i == 8 {
                let mut chars: Vec<char> = r.chars().collect();
                for c in chars.iter_mut().take(3) {
                    *c = '1';
                }
                *r = chars.into_iter().collect();
            }
        }
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        SolidGrid::from_rows(&rows)
    }

    #[test]
    fn circle_hits_solid_tile() {
        let g = floor_grid();
        // Centre at (12, 68) — sits right on top of the solid at (0,8)→(24,16).
        assert!(g.collide_circle(12.0, 68.0, 4.0));
    }

    #[test]
    fn circle_misses_when_far_above() {
        let g = floor_grid();
        // Centre at (12, 20), radius 4 — bounding box touches row 2..4, no solids.
        assert!(!g.collide_circle(12.0, 20.0, 4.0));
    }

    #[test]
    fn circle_touches_corner_without_intersection() {
        let g = floor_grid();
        // Centre at (28, 76), radius 4 — closest tile corner is (24, 72),
        // distance is sqrt(16+16)≈5.66 > 4, so no hit.
        assert!(!g.collide_circle(28.0, 76.0, 4.0));
    }

    #[test]
    fn circle_clips_tile_edge() {
        let g = floor_grid();
        // Centre at (26, 72), radius 4 — right edge of solid row.
        // Closest point on tile (24..32, 64..72) to (26, 72) is (26, 72), distance 0.
        assert!(g.collide_circle(26.0, 72.0, 4.0));
    }

    #[test]
    fn circle_zero_radius_does_not_collide() {
        let g = floor_grid();
        assert!(!g.collide_circle(12.0, 68.0, 0.0));
        assert!(!g.collide_circle(12.0, 68.0, -1.0));
    }

    #[test]
    fn circle_to_rect_basic() {
        // Circle (5, 5) r=3 vs rect (8, 8, 4, 4) — distance to (8,8) is sqrt(18)≈4.24 > 3.
        assert!(!SolidGrid::circle_to_rect(
            5.0, 5.0, 3.0, 8.0, 8.0, 4.0, 4.0
        ));
        // Same circle vs rect (4, 4, 4, 4) — distance to (4,4) is sqrt(2)≈1.41 < 3.
        assert!(SolidGrid::circle_to_rect(5.0, 5.0, 3.0, 4.0, 4.0, 4.0, 4.0));
        // Centre inside rect.
        assert!(SolidGrid::circle_to_rect(5.0, 5.0, 3.0, 4.0, 4.0, 2.0, 2.0));
    }

    #[test]
    fn circle_to_circle_basic() {
        // Two circles separated by 4 with radii 2+2=4: touching counts.
        assert!(SolidGrid::circle_to_circle(0.0, 0.0, 2.0, 4.0, 0.0, 2.0));
        // Separated by 5 with radii 2+2=4: not touching.
        assert!(!SolidGrid::circle_to_circle(0.0, 0.0, 2.0, 5.0, 0.0, 2.0));
        // Zero radius on either side: no hit.
        assert!(!SolidGrid::circle_to_circle(0.0, 0.0, 0.0, 0.0, 0.0, 2.0));
    }

    #[test]
    fn line_of_sight_blocked_by_solid() {
        let g = floor_grid();
        // The solid row (cols 0..2, x 0..24, y 64..72) sits between (12, 50)
        // and (12, 80) → no clear line of sight.
        assert!(!g.line_of_sight(12.0, 50.0, 12.0, 80.0));
        // Both points well above the floor (row 8 = y 64..72), clear of any
        // solid → clear sight.
        assert!(g.line_of_sight(12.0, 50.0, 12.0, 58.0));
        // A horizontal ray clear of the floor column is unobstructed.
        assert!(g.line_of_sight(40.0, 68.0, 80.0, 68.0));
    }
}
