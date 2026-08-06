//! A loaded playable map: solid/bg grids plus the entity spawn list.

use std::collections::HashMap;
use std::path::Path;

use ruleste_plugin_api::map::{MapAttr, MapData};

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
    /// Entities to spawn into the world, in map order.
    pub entities: Vec<EntitySpawn>,
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
        let width = root.attr_f32("width", 320.0);
        let height = root.attr_f32("height", 180.0);

        let solids = parse_grid(root.child("solids"));
        let bg = parse_grid(root.child("bg"));

        let mut entities = Vec::new();
        if let Some(ents) = root.child("entities") {
            for child in &ents.children {
                entities.push(EntitySpawn {
                    name: child.name.clone(),
                    data: attrs_to_map(child),
                });
            }
        }

        Ok(Level {
            name,
            solids,
            bg,
            width,
            height,
            entities,
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
    MapData { attrs }
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
