//! Asset inspection tool. Parses a real asset file and prints a summary so the
//! format parsers can be validated against the original `Content/` files.
//!
//! Usage:
//!   ruleste-inspect map    <map.bin>        dump level/entity summary
//!   ruleste-inspect atlas  <atlas.meta>     dump page/frame summary
//!   ruleste-inspect sprite <Sprites.xml>    dump sprite/animation summary
//!
//! Paths are passed on the command line; the tool never hardcodes any
//! asset location.

use std::collections::HashMap;
use std::path::Path;

use ruleste::data::atlas::Atlas;
use ruleste::data::binary_packer::MapBin;
use ruleste::data::spritebank::SpriteBank;
use ruleste::engine::level::Level;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| usage());
    let path = args.next().unwrap_or_else(|| usage());
    match cmd.as_str() {
        "map" => dump_map(Path::new(&path)),
        "atlas" => dump_atlas(Path::new(&path)),
        "sprite" => dump_sprite(Path::new(&path)),
        other => {
            eprintln!("unknown command: {other}");
            usage()
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: ruleste-inspect <map|atlas|sprite> <path>");
    std::process::exit(2);
}

fn dump_map(path: &Path) -> anyhow::Result<()> {
    let bin =
        MapBin::from_file(path).map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
    println!("map: {}", path.display());
    println!("  package: {}", bin.package);
    println!(
        "  root: {} ({} attrs, {} children)",
        bin.root.name,
        bin.root.attrs.len(),
        bin.root.children.len()
    );
    for (k, v) in &bin.root.attrs {
        println!("    root.{k} = {v:?}");
    }
    for c in &bin.root.children {
        println!(
            "    child {:?}: {} attrs, {} children",
            c.name,
            c.attrs.len(),
            c.children.len()
        );
    }

    let level = Level::from_bin(bin)?;
    println!("  rooms: {} total", level.rooms.len());
    for (idx, room) in level.rooms.iter().enumerate() {
        let (w, h) = room.solids.size();
        println!(
            "    room #{idx} ({}): bounds {}x{} at ({},{}), solids {}x{} tiles",
            room.name, room.width, room.height, room.x, room.y, w, h
        );
    }
    let room = level.room();
    let mut histogram: HashMap<&str, usize> = HashMap::new();
    for e in &room.entities {
        *histogram.entry(e.name.as_str()).or_default() += 1;
    }
    println!(
        "  current room entities: {} total, {} distinct types",
        room.entities.len(),
        histogram.len()
    );
    let mut types: Vec<_> = histogram.into_iter().collect();
    types.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (name, count) in &types {
        println!("    {count:>4}  {name}");
    }

    let room = level.room();
    let sample = room
        .entities
        .iter()
        .take(3)
        .map(|e| e.name.as_str())
        .collect::<Vec<_>>();
    println!("  first entity types: {sample:?}");
    for e in &room.entities {
        let x = e.data.get_float("x", 0.0);
        let y = e.data.get_float("y", 0.0);
        println!("    entity {:<12} at ({x:>6.1}, {y:>6.1})", e.name);
        for (k, v) in &e.data.attrs {
            if k != "x" && k != "y" {
                println!("        {k} = {v:?}");
            }
        }
        for (i, n) in e.data.nodes().iter().enumerate() {
            println!("        node[{i}] = ({}, {})", n.x, n.y);
        }
    }
    for d in &room.decorations {
        let x = d.data.get_float("x", 0.0);
        let y = d.data.get_float("y", 0.0);
        println!("    decoration {:<12} at ({x:>6.1}, {y:>6.1})", d.name);
        for (k, v) in &d.data.attrs {
            if k != "x" && k != "y" {
                println!("        {k} = {v:?}");
            }
        }
    }
    Ok(())
}

fn dump_atlas(path: &Path) -> anyhow::Result<()> {
    let atlas = Atlas::load(path).map_err(|e| anyhow::anyhow!("load {}: {e}", path.display()))?;
    println!("atlas: {}", path.display());
    for page in &atlas.pages {
        println!(
            "  page {}: {}x{} px, {} frames",
            page.name,
            page.width,
            page.height,
            page.frames.len()
        );
    }
    println!("  total frames indexed: {}", atlas.frame_index.len());

    let mut ids: Vec<&String> = atlas.frame_index.keys().collect();
    ids.sort();
    let filter = std::env::var("RULESTE_ATLAS_FILTER").unwrap_or_default();
    let shown: Vec<&&String> = ids
        .iter()
        .filter(|id| filter.is_empty() || id.contains(&filter))
        .take(40)
        .collect();
    if shown.is_empty() {
        println!("    (no frames match filter {filter:?})");
    }
    for id in shown {
        println!("    {id}");
    }
    if ids.len() > 40 {
        println!("    ... {} more", ids.len() - 40);
    }

    let player_id = "characters/player/idle00";
    match atlas.frame_clip(player_id) {
        Some(clip) => {
            let rgba = atlas.frame_rgba_into(player_id).unwrap_or_default();
            println!(
                "  sample frame {player_id}: clip {clip:?}, rgba {} bytes",
                rgba.len()
            );
        }
        None => println!("  sample frame {player_id}: not found"),
    }

    for fid in [
        "danger/dustcreature/center00",
        "danger/dustcreature/base01",
        "danger/dustcreature/overlay02",
        "util/lightbeam",
        "objects/dreamblock/particles",
        "objects/hanginglamp",
        "objects/crushblock/block00",
        "objects/crushblock/block01",
        "objects/crushblock/block02",
        "objects/crushblock/block03",
        "objects/booster/booster00",
        "objects/refill/idle00",
        "objects/refillTwo/idle00",
        "objects/checkpoint/highlight00",
        "objects/checkpoint/highlight09",
        "objects/checkpoint/flash00",
        "objects/checkpoint/bg/6",
        "objects/clouds/cloud00",
        "objects/clouds/fragile00",
        "objects/clouds/fragile06",
        "objects/temple/torch00",
        "objects/temple/torch04",
        "objects/temple/litTorch00",
    ] {
        let Some(clip) = atlas.frame_clip(fid) else {
            println!("  {fid}: not found");
            continue;
        };
        let rgba = atlas.frame_rgba_into(fid).unwrap_or_default();
        let mut opaque = 0;
        let mut colored = 0;
        let mut hist: HashMap<(u8, u8, u8), usize> = HashMap::new();
        let mut alpha_hist: HashMap<u8, usize> = HashMap::new();
        for px in rgba.chunks_exact(4) {
            let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
            if a > 8 {
                opaque += 1;
                *hist.entry((r, g, b)).or_default() += 1;
                *alpha_hist.entry(a).or_default() += 1;
                if a >= 16 {
                    colored += 1;
                }
            }
        }
        println!("  {fid}: clip {clip:?} alpha>8 = {opaque} px, alpha>=16 = {colored} px");
        let mut al: Vec<_> = alpha_hist.into_iter().collect();
        al.sort_by_key(|a| std::cmp::Reverse(a.1));
        println!("      alpha histogram (top 4): {al:?}");
        let mut top: Vec<_> = hist.into_iter().collect();
        top.sort_by_key(|a| std::cmp::Reverse(a.1));
        for ((r, g, b), n) in top.iter().take(6) {
            println!("      ({r},{g},{b}) x{n}");
        }
    }
    Ok(())
}

fn dump_sprite(path: &Path) -> anyhow::Result<()> {
    let bank = SpriteBank::load(path)?;
    println!("spritebank: {}", path.display());
    println!("  sprites: {}", bank.sprites.len());

    let mut names: Vec<&String> = bank.sprites.keys().collect();
    names.sort();
    for name in &names {
        let s = &bank.sprites[*name];
        println!(
            "  {}: path={:?} start={:?} origin={:?} animations={}",
            s.name,
            s.path,
            s.start,
            s.origin,
            s.animations.len()
        );
    }

    if let Some(player) = bank.sprite("player") {
        for anim in ["idle", "runSlow", "runFast", "jumpSlow", "dash", "duck"] {
            if let Some(a) = player.animation(anim) {
                println!(
                    "  player.{anim}: path={:?} delay={} loop={} goto={:?} frames={:?}",
                    a.path, a.delay, a.is_loop, a.goto, a.frames
                );
            }
        }
    }
    Ok(())
}
