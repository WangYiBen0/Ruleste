//! Custom draw commands submitted by plugins during their `ruleste_entity_draw`
//! hook. The host collects them per frame and renders them with the entity
//! sprites; scenery like the `wire` entity draws procedural geometry this way.

use ruleste_plugin_api::types::Color;

#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub color: Color,
}
