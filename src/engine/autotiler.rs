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
    path: String,
    masks: Vec<MaskEntry>,
}

/// Parsed autotiler definition loaded from ForegroundTiles.xml.
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

            // If copy="z", inherit the template masks.
            let masks = if let Some(copy_char) = copy_id.and_then(|s| s.chars().next()) {
                if let Some(parent) = tilesets.get(&copy_char) {
                    parent.masks.clone()
                } else {
                    Self::parse_masks(&tileset_node)
                }
            } else {
                Self::parse_masks(&tileset_node)
            };

            tilesets.insert(id, TilesetDef { path, masks });
        }

        Ok(Autotiler { tilesets })
    }

    fn parse_masks(node: &roxmltree::Node) -> Vec<MaskEntry> {
        let mut masks = Vec::new();
        for set in node.children().filter(|n| n.has_tag_name("set")) {
            let mask_str = set.attribute("mask").unwrap_or("");
            let tiles_str = set.attribute("tiles").unwrap_or("");

            if mask_str == "padding" || mask_str == "center" {
                // Special entries: "padding" and "center" use a sentinel mask
                // that matches when all 9 neighbors are solid.
                let tiles = Self::parse_tile_coords(tiles_str);
                let mask = [1u8; 9]; // all solid
                masks.push(MaskEntry {
                    mask,
                    wildcards: 0,
                    tiles,
                });
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

        // Sort by ascending wildcard count (most specific first).
        masks.sort_by_key(|m| m.wildcards);
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

    /// Check if a tile at `(tx, ty)` should be considered "solid" for adjacency
    /// purposes.  Some tilesets (e.g. dirt with `ignores="g"`) ignore certain
    /// other tile types when computing neighbors.
    fn is_solid_for(grid: &SolidGrid, tx: i32, ty: i32, _tile_id: char) -> bool {
        grid.solid_at(tx, ty)
    }

    /// Build the adjacency array (9 cells) for tile `(tx, ty)`.
    fn adjacency(grid: &SolidGrid, tx: i32, ty: i32) -> [u8; 9] {
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
                adj[i] = if Self::is_solid_for(grid, tx + dx, ty + dy, '0') {
                    1
                } else {
                    0
                };
            }
        }
        adj
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

                let adj = Self::adjacency(grid, tx as i32, ty as i32);

                // Check if all 9 neighbors are solid → center or padding.
                let all_solid = adj.iter().all(|&v| v == 1);

                let matched_tiles = if all_solid {
                    // Check 2-away for center vs padding.
                    let two_away = [(-2, 0), (2, 0), (0, -2), (0, 2)];
                    let deep_center = two_away.iter().all(|&(dx, dy)| {
                        Self::is_solid_for(grid, tx as i32 + dx, ty as i32 + dy, tile_ch)
                    });
                    if deep_center {
                        // "center" mask (all 1s, fewest wildcards = 0).
                        def.masks.last().map(|m| &m.tiles)
                    } else {
                        // "padding" mask (also all 1s, but second-to-last).
                        def.masks.iter().rev().nth(1).map(|m| &m.tiles)
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
