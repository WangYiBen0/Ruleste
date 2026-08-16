use ruleste::engine::level::Level;
fn main() {
    let base = std::path::Path::new("maps/Celeste/0-Intro.bin");
    let lvl = Level::load(base).unwrap();
    let room = lvl.room();
    println!(
        "level {} room {} solids {}x{}",
        lvl.name,
        room.name,
        room.solids.size().0,
        room.solids.size().1
    );
    for ty in 16..23 {
        let mut line = String::new();
        for tx in 0..room.solids.size().0 {
            line.push(room.solids.tile_id_at(tx as i32, ty).unwrap_or(' '));
        }
        println!("ty{ty}: {line}");
    }
}
