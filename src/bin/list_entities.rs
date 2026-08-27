use std::collections::BTreeMap;
use std::collections::BTreeSet;

use ruleste::engine::level::Level;

fn main() {
    let dir = std::path::Path::new("maps/Celeste");
    let mut all: BTreeSet<String> = BTreeSet::new();
    let mut per_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let mut bins: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    bins.sort();

    for bin in &bins {
        let name = bin.file_name().unwrap().to_string_lossy().to_string();
        let lvl = match Level::load(bin) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("skip {name}: {e}");
                continue;
            }
        };
        let set = per_map.entry(name.clone()).or_default();
        for room in &lvl.rooms {
            for e in room.entities.iter().chain(&room.decorations) {
                all.insert(e.name.clone());
                set.insert(e.name.clone());
                *per_type.entry(e.name.clone()).or_insert(0) += 1;
            }
        }
    }

    println!("=== entity types referenced across all Celeste maps ({} total) ===", all.len());
    for ty in &all {
        println!("{ty}\t{}", per_type.get(ty).unwrap_or(&0));
    }
    println!("\n=== per map ===");
    for (m, set) in &per_map {
        println!("MAP {m}");
        for t in set {
            println!("  {t}");
        }
    }
}
