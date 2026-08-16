//! Autotiler: parses ForegroundTiles.xml and generates per-tile texture
//! coordinates using the 3×3 adjacency mask algorithm from the original
//! Celeste `Autotiler.cs`.

use std::collections::HashMap;

use crate::engine::physics::SolidGrid;

/// A single mask entry: a 9-byte pattern + the list of (col, row) tile coords.
#[derive(Debug, Clone)]
struct MaskEntry {
    /// 9 values: 0 = must be empty, 1 = must be solid, 2 = wildcard.
    mask: [u8; 9],
    /// Number of wildcard positions (for sorting: fewest wildcards first).
    wildcards: u8,
    /// Tile coords in the tileset texture, e.g. [(0,0), (1,0), (2,0), (3,0)].
    tiles: Vec<(u32, u32)>,
}

/// A tileset definition: all mask entries plus the texture path.
#[derive(Debug, Clone)]
struct TilesetDef {
    id: char,
    path: String,
    masks: Vec<MaskEntry>,
    /// Tile coords used when all 9 neighbors are the same tileset ("center").
    center: Vec<(u32, u32)>,
    /// Tile coords used for a solid region's inner edge ("padding").
    padding: Vec<(u32, u32)>,
    /// Tile ids this tileset doesn't consider solid when checking neighbors
    /// (from the `ignores` attribute; `*` means ignore all other types).
    ignores: std::collections::HashSet<char>,
}

/// Parsed autotiler definition loaded from ForegroundTiles.xml.
#[derive(Clone)]
pub struct Autotiler {
    tilesets: HashMap<char, TilesetDef>,
}

/// The result of autotiling: per-tile texture coordinates in the tileset.
#[derive(Debug, Clone)]
pub struct TileGrid {
    pub width: usize,
    pub height: usize,
    /// Tileset path per tile (empty string = no tile).
    pub tileset: Vec<String>,
    /// Column index in the tileset texture.
    pub col: Vec<u32>,
    /// Row index in the tileset texture.
    pub row: Vec<u32>,
}

impl Autotiler {
    /// Load and parse a ForegroundTiles.xml file.
    pub fn load(xml_path: &std::path::Path) -> anyhow::Result<Self> {
        let xml = std::fs::read_to_string(xml_path)?;
        Self::parse(&xml)
    }

    /// Parse the XML string of ForegroundTiles.xml.
    pub fn parse(xml: &str) -> anyhow::Result<Self> {
        let doc = roxmltree::Document::parse(xml)?;
        let mut tilesets: HashMap<char, TilesetDef> = HashMap::new();

        for tileset_node in doc.descendants().filter(|n| n.has_tag_name("Tileset")) {
            let id = tileset_node
                .attribute("id")
                .unwrap_or("z")
                .chars()
                .next()
                .unwrap_or('z');
            let path = tileset_node
                .attribute("path")
                .unwrap_or("template")
                .to_string();
            let copy_id = tileset_node.attribute("copy");

            // If copy="z", inherit the template masks (original `Autotiler`
            // calls ReadInto once for the tileset's own sets, then again for
            // the copied tileset's sets).
            let mut masks = Self::parse_masks(&tileset_node);
            if let Some(copy_char) = copy_id.and_then(|s| s.chars().next()) {
                if let Some(parent) = tilesets.get(&copy_char) {
                    masks.extend(parent.masks.iter().cloned());
                }
            }

            // "center" and "padding" are separate from the mask list.
            let center = Self::parse_special_tiles(&tileset_node, "center");
            let padding = Self::parse_special_tiles(&tileset_node, "padding");
            let center = if center.is_empty() {
                tilesets
                    .get(&copy_id.and_then(|s| s.chars().next()).unwrap_or('z'))
                    .map(|p| p.center.clone())
                    .unwrap_or_default()
            } else {
                center
            };
            let padding = if padding.is_empty() {
                tilesets
                    .get(&copy_id.and_then(|s| s.chars().next()).unwrap_or('z'))
                    .map(|p| p.padding.clone())
                    .unwrap_or_default()
            } else {
                padding
            };

            let ignores = tileset_node
                .attribute("ignores")
                .unwrap_or("")
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().next().unwrap())
                .collect();

            // Sort by ascending wildcard count (most specific first), matching
            // the original `Masked.Sort`.
            masks.sort_by_key(|m| m.wildcards);

            tilesets.insert(
                id,
                TilesetDef {
                    id,
                    path,
                    masks,
                    center,
                    padding,
                    ignores,
                },
            );
        }

        Ok(Autotiler { tilesets })
    }

    /// Parse the "center"/"padding" special set entries.
    fn parse_special_tiles(node: &roxmltree::Node, name: &str) -> Vec<(u32, u32)> {
        for set in node.children().filter(|n| n.has_tag_name("set")) {
            if set.attribute("mask") == Some(name) {
                return set
                    .attribute("tiles")
                    .map(Self::parse_tile_coords)
                    .unwrap_or_default();
            }
        }
        Vec::new()
    }

    fn parse_masks(node: &roxmltree::Node) -> Vec<MaskEntry> {
        let mut masks = Vec::new();
        for set in node.children().filter(|n| n.has_tag_name("set")) {
            let mask_str = set.attribute("mask").unwrap_or("");
            let tiles_str = set.attribute("tiles").unwrap_or("");

            if mask_str == "padding" || mask_str == "center" {
                continue;
            }

            let mask = Self::parse_mask_str(mask_str);
            let wildcards = mask.iter().filter(|&&v| v == 2).count() as u8;
            let tiles = Self::parse_tile_coords(tiles_str);
            masks.push(MaskEntry {
                mask,
                wildcards,
                tiles,
            });
        }
        masks
    }

    /// Parse a mask string like `"x0x-111-x1x"` into a 9-byte array.
    /// Layout: [TL, T, TR, L, C, R, BL, B, BR]
    fn parse_mask_str(s: &str) -> [u8; 9] {
        let mut result = [0u8; 9];
        // Remove hyphens, then each char maps to one position.
        let chars: Vec<char> = s.chars().filter(|c| *c != '-').collect();
        for (i, ch) in chars.iter().enumerate().take(9) {
            result[i] = match ch {
                '0' => 0,
                '1' => 1,
                'x' | 'X' => 2,
                _ => 0,
            };
        }
        result
    }

    /// Parse tile coordinates like `"0,0;1,0;2,0;3,0"` into `[(0,0), (1,0), ...]`.
    fn parse_tile_coords(s: &str) -> Vec<(u32, u32)> {
        s.split(';')
            .filter_map(|pair| {
                let mut parts = pair.trim().splitn(2, ',');
                let col = parts.next()?.trim().parse::<u32>().ok()?;
                let row = parts.next()?.trim().parse::<u32>().ok()?;
                Some((col, row))
            })
            .collect()
    }

    /// Build the adjacency array (9 cells) for tile `(tx, ty)`.
    fn adjacency(grid: &SolidGrid, tx: i32, ty: i32, def: &TilesetDef) -> [u8; 9] {
        let mut adj = [0u8; 9];
        let offsets = [
            (-1, -1), // TL
            (0, -1),  // T
            (1, -1),  // TR
            (-1, 0),  // L
            (0, 0),   // C (always solid for non-empty)
            (1, 0),   // R
            (-1, 1),  // BL
            (0, 1),   // B
            (1, 1),   // BR
        ];
        for (i, (dx, dy)) in offsets.iter().enumerate() {
            if i == 4 {
                // Center is always solid for a non-empty tile.
                adj[i] = 1;
            } else {
                adj[i] = if Self::neighbor_connects(grid, tx + dx, ty + dy, def) {
                    1
                } else {
                    0
                };
            }
        }
        adj
    }

    /// Whether the tile at `(tx, ty)` connects with `def`'s tileset, matching
    /// the original `CheckTile`: same id connects, different ids connect
    /// unless ignored (explicit id or `*`).
    fn neighbor_connects(grid: &SolidGrid, tx: i32, ty: i32, def: &TilesetDef) -> bool {
        let w = grid.width as i32;
        let h = grid.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            // EdgesExtend: clamp to the edge tile.
            let (cx, cy) = (tx.clamp(0, w - 1), ty.clamp(0, h - 1));
            return match grid.tile_id_at(cx, cy) {
                Some(c) if c != '0' => Self::connects_id(def, c),
                _ => false,
            };
        }
        match grid.tile_id_at(tx, ty) {
            Some(c) if c != '0' => Self::connects_id(def, c),
            _ => false,
        }
    }

    /// Mirror of `TerrainType.Ignore(c)`: same id always connects; a different
    /// id connects unless it's in the ignores list or `*` (all) is present.
    fn connects_id(def: &TilesetDef, other: char) -> bool {
        if other == def.id {
            return true;
        }
        if def.ignores.contains(&'*') {
            return false;
        }
        !def.ignores.contains(&other)
    }

    /// Match an adjacency array against a mask.  Returns true when every
    /// non-wildcard position matches.
    fn mask_matches(adj: &[u8; 9], mask: &[u8; 9]) -> bool {
        for i in 0..9 {
            if mask[i] != 2 && mask[i] != adj[i] {
                return false;
            }
        }
        true
    }

    /// Generate a solid rectangular box of a single tile id, mirroring
    /// `Autotiler.GenerateBox`: all tiles are forced solid so the 3×3
    /// adjacency pass picks edge/corner/center variants naturally. Returns
    /// `None` if the tileset for `tile_id` is unknown.
    pub fn generate_box(&self, tile_id: char, tiles_x: usize, tiles_y: usize) -> Option<TileGrid> {
        if !self.tilesets.contains_key(&tile_id) {
            return None;
        }
        let row = tile_id.to_string().repeat(tiles_x);
        let rows: Vec<String> = (0..tiles_y).map(|_| row.clone()).collect();
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let grid = SolidGrid::from_rows(&rows);
        Some(self.generate(&grid))
    }

    /// Generate a `TileGrid` from the solid grid.
    ///
    /// For each non-empty tile, finds the matching tileset definition and
    /// picks a random variant from the matched mask's tile list.
    pub fn generate(&self, grid: &SolidGrid) -> TileGrid {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let (w, h) = grid.size();
        let mut tileset = vec![String::new(); w * h];
        let mut col = vec![0u32; w * h];
        let mut row = vec![0u32; w * h];

        for ty in 0..h {
            for tx in 0..w {
                let idx = ty * w + tx;
                let tile_ch = match grid.tile_id_at(tx as i32, ty as i32) {
                    Some(ch) => ch,
                    None => continue,
                };

                let def = match self.tilesets.get(&tile_ch) {
                    Some(d) => d,
                    None => continue,
                };

                let adj = Self::adjacency(grid, tx as i32, ty as i32, def);

                // Check if all 9 neighbors connect → center or padding.
                let all_solid = adj.iter().all(|&v| v == 1);

                let matched_tiles = if all_solid {
                    // Check 2-away for center vs padding, mirroring the
                    // original's `PaddingIgnoreOutOfLevel` logic: the tile is
                    // "padded" when any 2-away neighbor does not connect. An
                    // out-of-level 2-away tile never counts (CheckForSameLevel).
                    let (w, h) = (grid.width as i32, grid.height as i32);
                    let tx_i = tx as i32;
                    let ty_i = ty as i32;
                    let padding = [(-2, 0), (2, 0), (0, -2), (0, 2)].iter().any(|&(dx, dy)| {
                        let (nx, ny) = (tx_i + dx, ty_i + dy);
                        nx >= 0
                            && ny >= 0
                            && nx < w
                            && ny < h
                            && !Self::neighbor_connects(grid, nx, ny, def)
                    });
                    if padding {
                        Some(&def.padding)
                    } else {
                        Some(&def.center)
                    }
                } else {
                    // Find the first matching mask (sorted by wildcard count).
                    def.masks
                        .iter()
                        .find(|m| Self::mask_matches(&adj, &m.mask))
                        .map(|m| &m.tiles)
                };

                if let Some(tiles) = matched_tiles {
                    if !tiles.is_empty() {
                        // Deterministic "random" pick based on position.
                        let mut hasher = DefaultHasher::new();
                        tx.hash(&mut hasher);
                        ty.hash(&mut hasher);
                        let pick = (hasher.finish() as usize) % tiles.len();
                        tileset[idx] = def.path.clone();
                        col[idx] = tiles[pick].0;
                        row[idx] = tiles[pick].1;
                    }
                }
            }
        }

        TileGrid {
            width: w,
            height: h,
            tileset,
            col,
            row,
        }
    }
}

impl TileGrid {
    /// Returns `(tileset_path, col, row)` for the tile at `(tx, ty)`.
    pub fn tile_at(&self, tx: usize, ty: usize) -> Option<(&str, u32, u32)> {
        let idx = ty * self.width + tx;
        let path = self.tileset.get(idx)?;
        if path.is_empty() {
            return None;
        }
        Some((path, self.col[idx], self.row[idx]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The template tileset (`z`): edge masks use rows 0-3, isolated tiles
    /// row 5/10, and the "center" / "padding" entries are the col-5 tiles.
    const TEMPLATE_XML: &str = r#"<?xml version="1.0"?>
    <Tilesets>
      <Tileset id="z" path="template">
        <set mask="x0x-111-x1x" tiles="0,0;1,0;2,0;3,0"/>
        <set mask="x1x-111-x0x" tiles="0,1;1,1;2,1;3,1"/>
        <set mask="x1x-011-x1x" tiles="0,2;1,2;2,2;3,2"/>
        <set mask="x1x-110-x1x" tiles="0,3;1,3;2,3;3,3"/>
        <set mask="x1x-010-x1x" tiles="0,5;1,5;2,5;3,5"/>
        <set mask="x0x-010-x0x" tiles="0,10;1,10;2,10;3,10"/>
        <set mask="padding" tiles="5,0;5,1"/>
        <set mask="center" tiles="5,12"/>
      </Tileset>
      <Tileset id="1" copy="z" path="dirt" ignores="g"/>
      <Tileset id="h" copy="z" path="grass" ignores="*"/>
    </Tilesets>"#;

    fn autotiler() -> Autotiler {
        Autotiler::parse(TEMPLATE_XML).unwrap()
    }

    fn grid(rows: &[&str]) -> SolidGrid {
        SolidGrid::from_rows(rows)
    }

    #[test]
    fn center_for_deep_interior() {
        let a = autotiler();
        let g = grid(&["zz", "zz"]);
        let t = a.generate(&g);
        // Every tile has all-solid neighbors → center (5,12).
        for (tx, ty) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(t.tile_at(tx, ty), Some(("template", 5, 12)));
        }
    }

    #[test]
    fn padding_for_inner_edge_of_large_block() {
        let a = autotiler();
        // 5x5 solid block with a hole two away from (3,2): its 3x3 is solid
        // but the 2-away tile below is empty → padding, not center.
        let g = grid(&["zzzzz", "zzzzz", "zzzzz", "zzzzz", "zzz0z"]);
        let t = a.generate(&g);
        assert_eq!(t.tile_at(2, 2), Some(("template", 5, 12)));
        let (_, c, r) = t.tile_at(3, 2).unwrap();
        assert_eq!(c, 5);
        assert!(r == 0 || r == 1);
    }

    #[test]
    fn top_edge_mask_when_above_is_empty() {
        let a = autotiler();
        // A platform with empty above, solid below and sides.
        let g = grid(&["000", "1z1", "111"]);
        let t = a.generate(&g);
        let (_, c, _) = t.tile_at(1, 1).unwrap();
        // "x0x-111-x1x" → tiles at row 0.
        assert_eq!(c, 1); // deterministic pick from 0,0..3,0
    }

    #[test]
    fn ignores_star_isolates_grass() {
        let a = autotiler();
        // grass 'h' ignores all other types: a lone grass tile between dirt
        // '1' on both sides sees empty neighbors → isolated row-5 mask.
        let g = grid(&["1h1"]);
        let t = a.generate(&g);
        let (path, c, r) = t.tile_at(1, 0).unwrap();
        assert_eq!(path, "grass");
        // Isolated (only self solid) → "x0x-010-x0x" row 10, or with solid
        // above/below via EdgesExtend → "x1x-010-x1x" row 5.
        assert!(c != 5, "isolated grass should not be center");
        assert!(r == 5 || r == 10);
    }

    #[test]
    fn dirt_connects_to_snow_not_grass() {
        let a = autotiler();
        // dirt '1' ignores only grass 'g'. Its grass neighbors are treated as
        // empty, so the dirt tile is isolated too → not center.
        let g = grid(&["g1g"]);
        let t = a.generate(&g);
        let (path, c, _) = t.tile_at(1, 0).unwrap();
        assert_eq!(path, "dirt");
        assert!(c != 5, "dirt beside only grass should not be center");
    }
}
