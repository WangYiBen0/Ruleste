//! Custom draw commands submitted by plugins during their `ruleste_entity_draw`
//! hook. The host collects them per frame and renders them with the entity
//! sprites; scenery like the `wire` entity draws procedural geometry this way.

use ruleste_plugin_api::types::Color;

/// A straight line segment in world coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub color: Color,
}

/// A filled axis-aligned rectangle in world coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

/// A single atlas frame blitted at an arbitrary position in world coordinates.
/// Used by entities that compose several tiles (spike rows) or need rotation
/// and scaling (spinners, lanterns) that the one-sprite-per-entity model can't
/// express.
#[derive(Debug, Clone)]
pub struct Image {
    /// Atlas frame id, e.g. `danger/spikes/default_up00`.
    pub frame_id: String,
    /// World position of the frame's center (after the atlas frame offset).
    pub x: f32,
    pub y: f32,
    /// Rotation in degrees, clockwise.
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub color: Color,
}

/// An autotiled box of 8x8 tiles (mirrors `Autotiler.GenerateBox`, e.g. the
/// introCrusher slab). The host fills `col`/`row` from the solid-grid
/// adjacency pass; the renderer blits each cell from the tileset frame.
#[derive(Debug, Clone)]
pub struct TileBox {
    /// Atlas frame id of the tileset, e.g. `tilesets/snow`.
    pub frame_id: String,
    /// Top-left corner in world coordinates.
    pub x: f32,
    pub y: f32,
    pub width: usize,
    pub height: usize,
    /// Per-cell column index in the tileset texture (row-major).
    pub col: Vec<u32>,
    /// Per-cell row index in the tileset texture (row-major).
    pub row: Vec<u32>,
}
