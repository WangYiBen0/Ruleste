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
    let (w, h) = level.solids.size();
    println!(
        "  bounds: {}x{}  solids: {}x{} tiles  bg: {}x{} tiles",
        level.width,
        level.height,
        w,
        h,
        level.bg.size().0,
        level.bg.size().1
    );
    println!(
        "  camera_offset: ({}, {})",
        level.camera_offset.x, level.camera_offset.y
    );

    println!("  solids grid (rows of # = solid):");
    for row in 0..level.solids.size().1 {
        let mut line = String::new();
        for col in 0..level.solids.size().0 {
            let solid = level.solids.solid_at(col as i32, row as i32);
            line.push(if solid { '#' } else { '.' });
        }
        println!("    {row:>3} {line}");
    }

    let mut histogram: HashMap<&str, usize> = HashMap::new();
    for e in &level.entities {
        *histogram.entry(e.name.as_str()).or_default() += 1;
    }
    println!(
        "  entities: {} total, {} distinct types",
        level.entities.len(),
        histogram.len()
    );
    let mut types: Vec<_> = histogram.into_iter().collect();
    types.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (name, count) in &types {
        println!("    {count:>4}  {name}");
    }

    let mut dec_hist: HashMap<&str, usize> = HashMap::new();
    for d in &level.decorations {
        *dec_hist.entry(d.name.as_str()).or_default() += 1;
    }
    if !level.decorations.is_empty() {
        println!(
            "  decorations: {} total, {} distinct types",
            level.decorations.len(),
            dec_hist.len()
        );
        let mut dtypes: Vec<_> = dec_hist.into_iter().collect();
        dtypes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        for (name, count) in &dtypes {
            println!("    {count:>4}  {name}");
        }
    }

    let sample = level
        .entities
        .iter()
        .take(3)
        .map(|e| e.name.as_str())
        .collect::<Vec<_>>();
    println!("  first entity types: {sample:?}");
    for e in &level.entities {
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
