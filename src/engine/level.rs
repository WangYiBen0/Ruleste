//! A loaded playable map: solid/bg grids plus the entity spawn list.

use std::collections::HashMap;
use std::path::Path;

use ruleste_plugin_api::map::{MapAttr, MapData};
use ruleste_plugin_api::types::Vec2;

use crate::data::binary_packer::{Attr, Element, MapBin};
use crate::engine::physics::SolidGrid;

#[derive(Debug, Clone)]
pub struct EntitySpawn {
    pub name: String,
    pub data: MapData,
}

#[derive(Debug)]
pub struct Level {
    /// Map name, e.g. "0-ForsakenCity".
    pub name: String,
    /// Solid collision grid, in tiles.
    pub solids: SolidGrid,
    /// Background grid (rendered, not solid).
    pub bg: SolidGrid,
    /// Width and height in world units.
    pub width: f32,
    pub height: f32,
    /// Camera offset (in world units) applied to the follow target.
    /// Mirrors `Level.CameraOffset = (48, 32) * levelData.CameraOffset`.
    pub camera_offset: Vec2,
    /// Entities to spawn into the world, in map order.
    pub entities: Vec<EntitySpawn>,
    /// Decorative entities without gameplay collision boxes.
    pub decorations: Vec<EntitySpawn>,
}

fn is_decoration(name: &str) -> bool {
    matches!(
        name,
        "wire" | "lamp" | "resortLantern" | "torch" | "hanginglamp" | "floatingDebris" | "cobweb"
    ) || name.starts_with("dec")
        || name.ends_with("dec")
}

impl Level {
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

        let level = levels_el
            .children
            .iter()
            .find(|l| {
                l.child("entities")
                    .is_some_and(|ents| ents.children.iter().any(|e| e.name == "player"))
            })
            .or_else(|| levels_el.children.first())
            .ok_or_else(|| anyhow::anyhow!("map {:?} has no levels", name))?;

        let width = level.attr_f32("width", 320.0);
        let height = level.attr_f32("height", 180.0);

        let camera_offset = Vec2::new(
            48.0 * level.attr_f32("cameraOffsetX", 0.0),
            32.0 * level.attr_f32("cameraOffsetY", 0.0),
        );

        let solids = parse_grid(level.child("solids"));
        let bg = parse_grid(level.child("bg"));

        let mut entities = Vec::new();
        let mut decorations = Vec::new();
        if let Some(ents) = level.child("entities") {
            for child in &ents.children {
                let mut data = attrs_to_map(child);
                // The entity type name is always available to plugins, so a
                // plugin handling several entity types can tell them apart.
                data.attrs
                    .push(("_entity_type".to_string(), MapAttr::Str(child.name.clone())));
                // Entity nodes are the entity element's children, mirroring
                // `LevelData.CreateEntityData` (each child's x/y form a node).
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

        Ok(Level {
            name,
            solids,
            bg,
            width,
            height,
            camera_offset,
            entities,
            decorations,
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
    // Ensure x/y exist so plugins always have a spawn position.
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

/// Associates entity types with a plugin by scanning a list of loaded plugin
/// files. For now the mapping is: a plugin whose `ruleste_plugin_entity_types`
/// reports a given type owns that type.
pub fn index_entities_by_type(spawns: &[EntitySpawn]) -> HashMap<String, Vec<usize>> {
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, spawn) in spawns.iter().enumerate() {
        map.entry(spawn.name.clone()).or_default().push(i);
    }
    map
}
