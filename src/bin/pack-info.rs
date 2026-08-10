//! Prints installed packs (and their chapters) from the `resources/` tree.
//!
//! Usage: `pack-info [resources-root]` — defaults to `./resources`.

use std::path::Path;

use ruleste::data::pack::PackMeta;

fn main() {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "resources".to_string());
    let packs = PackMeta::scan_all(Path::new(&root));
    println!("packs in {root}/: {}", packs.len());
    for p in packs {
        println!(
            "- {} (pack={} ns={} v{})",
            p.title, p.pack, p.namespace, p.version
        );
        for c in &p.chapters {
            let song = if c.song.is_empty() { "-" } else { &c.song };
            println!("    {} [{song}] {}", c.id, c.map.display());
        }
    }
}
