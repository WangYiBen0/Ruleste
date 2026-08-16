//! `ruleste-plugin-decorations` — every purely atmospheric / visual entity.
//!
//! Consolidates the decoration plugins that do not affect other entities:
//! birds, campfires, cliff flag lines, cobwebs, floating debris, lamps,
//! lightbeams, resort lanterns, sound markers, summit/tower background
//! managers, torches and zip-mover wires. None of these alter gameplay; each
//! just draws scenery, sways, or remembers attributes for later systems.
//!
//! Each former plugin is a module exposing `init`/`update`/`draw`; the single
//! crate-level dispatch tags every entity with its `Kind` at spawn time.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod bird;
mod bonfire;
mod cliffflag;
mod cobweb;
mod debris;
mod flutterbird;
mod hanginglamp;
mod lamp;
mod lightbeam;
mod resort_lantern;
mod soundsource;
mod summitbackground;
mod torch;
mod towerviewer;
mod wire;

use std::cell::RefCell;

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("decorations");
ruleste_plugin_api::ruleste_entity_types!(
    "bird",
    "bonfire",
    "cliffflag",
    "cobweb",
    "floatingDebris",
    "foregroundDebris",
    "flutterbird",
    "hanginglamp",
    "lamp",
    "lightbeam",
    "resortLantern",
    "soundSource",
    "SummitBackgroundManager",
    "torch",
    "towerviewer",
    "wire",
);
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Which decoration module owns an entity.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Bird,
    Bonfire,
    CliffFlag,
    Cobweb,
    Debris,
    FlutterBird,
    HangingLamp,
    Lamp,
    LightBeam,
    ResortLantern,
    SoundSource,
    SummitBackground,
    Torch,
    TowerViewer,
    Wire,
}

thread_local! {
    static KINDS: RefCell<EntityState<Kind>> = RefCell::new(EntityState::new());
}

fn with_kind<R>(id: EntityId, f: impl FnOnce(&mut Kind) -> R) -> Option<R> {
    KINDS.with(|s| s.borrow_mut().get_mut(id).map(f))
}

fn kind_for_type(t: &str) -> Option<Kind> {
    Some(match t {
        "bird" => Kind::Bird,
        "bonfire" => Kind::Bonfire,
        "cliffflag" => Kind::CliffFlag,
        "cobweb" => Kind::Cobweb,
        "floatingDebris" | "foregroundDebris" => Kind::Debris,
        "flutterbird" => Kind::FlutterBird,
        "hanginglamp" => Kind::HangingLamp,
        "lamp" => Kind::Lamp,
        "lightbeam" => Kind::LightBeam,
        "resortLantern" => Kind::ResortLantern,
        "soundSource" => Kind::SoundSource,
        "SummitBackgroundManager" => Kind::SummitBackground,
        "torch" => Kind::Torch,
        "towerviewer" => Kind::TowerViewer,
        "wire" => Kind::Wire,
        _ => return None,
    })
}

fn spawn_type(data: *const u8, len: u32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    spawn.get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let Some(kind) = kind_for_type(&spawn_type(data, len)) else {
        return;
    };
    KINDS.with(|s| s.borrow_mut().insert(id, kind));
    match kind {
        Kind::Bird => bird::init(id, data, len),
        Kind::Bonfire => bonfire::init(id, data, len),
        Kind::CliffFlag => cliffflag::init(id, data, len),
        Kind::Cobweb => cobweb::init(id, data, len),
        Kind::Debris => debris::init(id, data, len),
        Kind::FlutterBird => flutterbird::init(id, data, len),
        Kind::HangingLamp => hanginglamp::init(id, data, len),
        Kind::Lamp => lamp::init(id, data, len),
        Kind::LightBeam => lightbeam::init(id, data, len),
        Kind::ResortLantern => resort_lantern::init(id, data, len),
        Kind::SoundSource => soundsource::init(id, data, len),
        Kind::SummitBackground => summitbackground::init(id, data, len),
        Kind::Torch => torch::init(id, data, len),
        Kind::TowerViewer => towerviewer::init(id, data, len),
        Kind::Wire => wire::init(id, data, len),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let Some(kind) = with_kind(id, |k| *k) else {
        return;
    };
    match kind {
        Kind::Bird => bird::update(id, dt),
        Kind::Bonfire => bonfire::update(id, dt),
        Kind::CliffFlag => cliffflag::update(id, dt),
        Kind::Cobweb => cobweb::update(id, dt),
        Kind::Debris => debris::update(id, dt),
        Kind::FlutterBird => flutterbird::update(id, dt),
        Kind::HangingLamp => hanginglamp::update(id, dt),
        Kind::Lamp => lamp::update(id, dt),
        Kind::LightBeam => lightbeam::update(id, dt),
        Kind::ResortLantern => resort_lantern::update(id, dt),
        Kind::SoundSource => soundsource::update(id, dt),
        Kind::SummitBackground => summitbackground::update(id, dt),
        Kind::Torch => torch::update(id, dt),
        Kind::TowerViewer => towerviewer::update(id, dt),
        Kind::Wire => wire::update(id, dt),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let Some(kind) = with_kind(id, |k| *k) else {
        return;
    };
    match kind {
        Kind::Bird => bird::draw(id),
        Kind::Bonfire => bonfire::draw(id),
        Kind::CliffFlag => cliffflag::draw(id),
        Kind::Cobweb => cobweb::draw(id),
        Kind::Debris => debris::draw(id),
        Kind::FlutterBird => flutterbird::draw(id),
        Kind::HangingLamp => hanginglamp::draw(id),
        Kind::Lamp => lamp::draw(id),
        Kind::LightBeam => lightbeam::draw(id),
        Kind::ResortLantern => resort_lantern::draw(id),
        Kind::SoundSource => soundsource::draw(id),
        Kind::SummitBackground => summitbackground::draw(id),
        Kind::Torch => torch::draw(id),
        Kind::TowerViewer => towerviewer::draw(id),
        Kind::Wire => wire::draw(id),
    }
}
