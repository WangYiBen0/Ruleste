//! A loaded playable map: solid/bg grids plus the entity spawn list.

use std::collections::HashMap;
use std::path::Path;

use ruleste_plugins_api::map::{MapAttr, MapData};
use ruleste_plugins_api::types::Vec2;

use crate::data::binary_packer::{Attr, Element, MapBin};
use crate::engine::backdrops::{self, Backdrop};
use crate::engine::physics::{JumpThru, SolidGrid, TILE};

/// `JumpThru`'s `Hitbox(width, 5f)` height.
const JUMPTHRU_HEIGHT: f32 = 5.0;

#[derive(Debug, Clone)]
pub struct EntitySpawn {
    pub name: String,
    pub data: MapData,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub camera_offset: Vec2,
    pub solids: SolidGrid,
    pub bg: SolidGrid,
    pub entities: Vec<EntitySpawn>,
    pub decorations: Vec<EntitySpawn>,
}

#[derive(Debug)]
pub struct Level {
    /// Map name, e.g. "0-ForsakenCity".
    pub name: String,
    /// Loaded rooms in the map. Each room carries its own local solid/background
    /// grids and entity spawns; the active room (see `current_room`) drives the
    /// camera bounds, backdrop parallax and which entities are spawned.
    pub rooms: Vec<Room>,
    /// Index of the active room.
    pub current_room: usize,
    /// Level-wide solid grid in world coordinates: every room's local grid is
    /// composited (anchored at `min_tx`/`min_ty`) into one continuous field, exactly
    /// like the original game. The player collides against this single grid and so
    /// can walk across room boundaries (a room edge is only a wall where the level
    /// actually places one). Room origins are all 8px-aligned, so there is no
    /// sub-tile rounding drift between tiles and entity spawns.
    pub solids: SolidGrid,
    /// Level-wide background-tile grid in world coordinates.
    pub bg: SolidGrid,
    /// Parallax background layers drawn behind the world.
    pub backgrounds: Vec<Backdrop>,
    /// Parallax foreground layers drawn in front of the world.
    pub foregrounds: Vec<Backdrop>,
}

/// Decoration marker file: every entity type name listed here is treated
/// as a pure visual prop and routed to `room.decorations` instead of
/// `room.entities`. The actual rendering is owned by
/// `ruleste-plugin-decorations`; this engine code only consumes the marker
/// so gameplay logic stays out of the engine.
const DECORATION_MARKER: &str = include_str!("../../plugins/decorations/marker.toml");

fn decoration_set() -> &'static std::collections::HashSet<&'static str> {
    use std::sync::OnceLock;
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut out = std::collections::HashSet::new();
        // Tiny ad-hoc parser: each `"foo",` token inside the
        // `decoration_types = [ ... ]` array becomes one entry. Avoids
        // pulling in a full TOML crate at host compile time.
        let body = DECORATION_MARKER
            .split_once('[')
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(arr, _)| arr)
            .unwrap_or("");
        for raw in body.split(',') {
            let t = raw.trim();
            if let Some(stripped) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                if !stripped.is_empty() {
                    out.insert(stripped);
                }
            }
        }
        out
    })
}

fn is_decoration(name: &str) -> bool {
    decoration_set().contains(name)
}

impl Level {
    pub fn room(&self) -> &Room {
        &self.rooms[self.current_room]
    }

    pub fn room_mut(&mut self) -> &mut Room {
        &mut self.rooms[self.current_room]
    }

    pub fn load(path: &Path) -> anyhow::Result<Level> {
        let bin = MapBin::from_file(path)
            .map_err(|e| anyhow::anyhow!("failed to parse map {}: {e}", path.display()))?;
        Self::from_bin(bin)
    }

    pub fn from_bin(bin: MapBin) -> anyhow::Result<Level> {
        let root = &bin.root;
        let name = root.attr_str("name", "unknown");

        let levels_el = root
            .child("levels")
            .ok_or_else(|| anyhow::anyhow!("map {:?} has no levels", name))?;

        let mut rooms = Vec::new();
        for level in &levels_el.children {
            let room_name = level.attr_str("name", "room");
            let rx = level.attr_f32("x", 0.0);
            let ry = level.attr_f32("y", 0.0);
            let width = level.attr_f32("width", 320.0);
            let height = level.attr_f32("height", 180.0);

            let camera_offset = Vec2::new(
                48.0 * level.attr_f32("cameraOffsetX", 0.0),
                32.0 * level.attr_f32("cameraOffsetY", 0.0),
            );

            let mut solids = parse_grid(level.child("solids"));
            let bg = parse_grid(level.child("bg"));

            let mut entities = Vec::new();
            let mut decorations = Vec::new();
            if let Some(ents) = level.child("entities") {
                for child in &ents.children {
                    if child.name == "jumpThru" {
                        solids.add_jumpthru(JumpThru {
                            x: child.attr_f32("x", 0.0),
                            y: child.attr_f32("y", 0.0),
                            w: child.attr_f32("width", 8.0),
                            h: JUMPTHRU_HEIGHT,
                        });
                        continue;
                    }
                    let mut data = attrs_to_map(child);
                    data.attrs
                        .push(("_entity_type".to_string(), MapAttr::Str(child.name.clone())));
                    data.nodes = child
                        .children
                        .iter()
                        .map(|n| Vec2::new(n.attr_f32("x", 0.0), n.attr_f32("y", 0.0)))
                        .collect();
                    let spawn = EntitySpawn {
                        name: child.name.clone(),
                        data,
                    };
                    if is_decoration(&child.name) {
                        decorations.push(spawn);
                    } else {
                        entities.push(spawn);
                    }
                }
            }

            rooms.push(Room {
                name: room_name,
                x: rx,
                y: ry,
                width,
                height,
                camera_offset,
                solids,
                bg,
                entities,
                decorations,
            });
        }

        if rooms.is_empty() {
            return Err(anyhow::anyhow!("map {:?} has no valid rooms", name));
        }

        let current_room = rooms
            .iter()
            .position(|r| r.x <= 0.0 && r.y <= 0.0 && r.x + r.width > 0.0 && r.y + r.height > 0.0)
            .or_else(|| {
                rooms
                    .iter()
                    .position(|r| r.entities.iter().any(|e| e.name == "player"))
            })
            .unwrap_or(0);

        let (backgrounds, foregrounds) = backdrops::parse(root.child("Style"));

        // Composite every room's local grid into one level-wide grid anchored at
        // the level's minimum corner, so collision and rendering operate in a
        // single world coordinate space (matching the original game, where the
        // whole level is one continuous solid field and rooms are only logical
        // subdivisions for entity spawning and camera bounds). Room origins are
        // 8px-aligned, so `floor(r.x / TILE)` is exact and there is no sub-tile
        // drift between tiles and entity spawns.
        let min_tx = rooms
            .iter()
            .map(|r| (r.x / TILE).floor() as i32)
            .min()
            .unwrap_or(0);
        let min_ty = rooms
            .iter()
            .map(|r| (r.y / TILE).floor() as i32)
            .min()
            .unwrap_or(0);
        let max_tx = rooms
            .iter()
            .map(|r| ((r.x + r.width) / TILE).ceil() as i32)
            .max()
            .unwrap_or(0);
        let max_ty = rooms
            .iter()
            .map(|r| ((r.y + r.height) / TILE).ceil() as i32)
            .max()
            .unwrap_or(0);
        let full_w = (max_tx - min_tx).max(0) as usize;
        let full_h = (max_ty - min_ty).max(0) as usize;

        let mut solids = SolidGrid::empty(full_w, full_h);
        solids.origin_x = min_tx as f32 * TILE;
        solids.origin_y = min_ty as f32 * TILE;
        let mut bg = SolidGrid::empty(full_w, full_h);
        bg.origin_x = min_tx as f32 * TILE;
        bg.origin_y = min_ty as f32 * TILE;
        for r in &rooms {
            let dx = (r.x / TILE).floor() as i32 - min_tx;
            let dy = (r.y / TILE).floor() as i32 - min_ty;
            solids.blit(&r.solids, dx, dy);
            bg.blit(&r.bg, dx, dy);
        }

        Ok(Level {
            name,
            rooms,
            current_room,
            solids,
            bg,
            backgrounds,
            foregrounds,
        })
    }
}

fn parse_grid(el: Option<&Element>) -> SolidGrid {
    match el.and_then(|e| e.attr("innerText")) {
        Some(Attr::RleString(s)) => {
            let rows: Vec<&str> = s.lines().collect();
            SolidGrid::from_rows(&rows)
        }
        _ => SolidGrid::from_rows(&[]),
    }
}

/// Converts binary-packer attributes into plugin spawn data, keeping the same
/// keys/values but fully resolving lookup-table strings.
fn attrs_to_map(el: &Element) -> MapData {
    let mut attrs = Vec::with_capacity(el.attrs.len());
    for (key, value) in &el.attrs {
        let map = match value {
            Attr::Bool(b) => MapAttr::Bool(*b),
            Attr::Byte(b) => MapAttr::Byte(*b),
            Attr::Short(s) => MapAttr::Short(*s),
            Attr::Int(i) => MapAttr::Int(*i),
            Attr::Float(f) => MapAttr::Float(*f),
            Attr::String(s) | Attr::RleString(s) => MapAttr::Str(s.clone()),
        };
        attrs.push((key.clone(), map));
    }
    let has_xy = attrs.iter().any(|(k, _)| k == "x");
    if !has_xy {
        attrs.push(("x".to_string(), MapAttr::Float(0.0)));
        attrs.push(("y".to_string(), MapAttr::Float(0.0)));
    }
    MapData {
        attrs,
        nodes: Vec::new(),
    }
}

pub fn index_entities_by_type(spawns: &[EntitySpawn]) -> HashMap<String, Vec<usize>> {
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, spawn) in spawns.iter().enumerate() {
        map.entry(spawn.name.clone()).or_default().push(i);
    }
    map
}
