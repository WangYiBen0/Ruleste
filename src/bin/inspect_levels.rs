use ruleste::engine::level::Level;
fn main() {
    let base = std::path::Path::new("maps/Celeste/0-Intro.bin");
    let lvl = Level::load(base).unwrap();
    println!(
        "level {} solids {}x{}",
        lvl.name,
        lvl.solids.size().0,
        lvl.solids.size().1
    );
    // print solid tile ids grid rows 16..22
    for ty in 16..23 {
        let mut line = String::new();
        for tx in 0..lvl.solids.size().0 {
            line.push(lvl.solids.tile_id_at(tx as i32, ty).unwrap_or(' '));
        }
        println!("ty{ty}: {line}");
    }
}
