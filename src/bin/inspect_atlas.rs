use ruleste::data::atlas::Atlas;
use std::path::Path;
fn main() {
    let meta = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "resources/Celeste/Celeste/textures/Atlases/Gameplay.meta".to_string());
    let atlas = Atlas::load(Path::new(&meta)).unwrap();
    let (pi, fi) = atlas.frame_index.get("tilesets/dirt").unwrap();
    let page = &atlas.pages[*pi];
    let f = &page.frames[*fi];
    let w = page.width as usize;
    let rgba = &page.rgba;
    let (fx, fy) = (f.clip.x as usize, f.clip.y as usize);
    let (cw, ch) = (f.clip.w as usize, f.clip.h as usize);
    let mut out = Vec::new();
    out.extend_from_slice(format!("P6\n{cw} {ch}\n255\n").as_bytes());
    for ty in 0..ch {
        for tx in 0..cw {
            let i = ((fy + ty) * w + (fx + tx)) * 4;
            out.extend_from_slice(&[rgba[i], rgba[i + 1], rgba[i + 2]]);
        }
    }
    std::fs::write("/tmp/opencode/dirt.png", out).unwrap();
    println!("wrote /tmp/opencode/dirt.png (48x120)");
}
