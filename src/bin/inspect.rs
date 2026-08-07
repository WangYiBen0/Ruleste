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

    let sample = level
        .entities
        .iter()
        .take(3)
        .map(|e| e.name.as_str())
        .collect::<Vec<_>>();
    println!("  first entity types: {sample:?}");
    if let Some(first) = level.entities.first() {
        println!("  first entity attrs: {:?}", first.data.attrs);
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
    for id in ids.iter().take(12) {
        println!("    {id}");
    }
    if ids.len() > 12 {
        println!("    ... {} more", ids.len() - 12);
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
