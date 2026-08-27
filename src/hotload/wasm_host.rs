//! Wasm plugin host. Loads `.wasm` entity plugins, maps entity types to plugin
//! instances, and provides the FFI surface plugins call to interact with the
//! world (position, speed, sprite, input, collision, sound, ...).
//!
//! Hot reload: the host polls each plugin file's mtime; when a file changes it
//! re-instantiates the plugin and migrates live entity state through the
//! `ruleste_entity_serialize`/`ruleste_entity_deserialize` pair.
//!
//! Buffers are passed to plugins by calling the plugin's exported
//! `ruleste_alloc`/`ruleste_dealloc`, so the host never guesses at the plugin's
//! heap layout.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use ruleste_plugins_api::export;
use ruleste_plugins_api::types::Color;
use wasmtime::{Caller, Engine, Extern, Instance, Linker, Memory, Module, Store, TypedFunc};

use crate::engine::autotiler::{Autotiler, TileGrid};
use crate::engine::draw::{Image, Line, Rect, TileBox};
use crate::engine::ecs::World;
use crate::engine::input::Input;
use crate::engine::physics::SolidGrid;

pub const SCRATCH_ALLOC: &str = "ruleste_alloc";
pub const SCRATCH_DEALLOC: &str = "ruleste_dealloc";

/// How long a death freezes the room before it reloads (mirrors the original's
/// respawn delay).
pub const DEATH_FREEZE_TIME: f32 = 1.2;

/// Set once from the `RULESTE_DEBUG` env var; plugins read it through
/// `host_debug_enabled` to decide whether to emit verbose logs.
static DEBUG_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct GameEvent {
    pub entity: u32,
    pub kind: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct AudioBus {
    pub requests: Vec<(String, f32)>,
}

impl AudioBus {
    pub fn play(&mut self, name: &str, pitch: f32) {
        self.requests.push((name.to_string(), pitch));
    }

    pub fn drain(&mut self) -> Vec<(String, f32)> {
        std::mem::take(&mut self.requests)
    }
}

/// Everything the FFI callbacks can reach: the shared game state that plugins
/// read and mutate while running.
pub struct GameState {
    pub world: World,
    pub input: Input,
    pub solids: SolidGrid,
    pub audio: AudioBus,
    pub events: Vec<GameEvent>,
    /// Geometry submitted by plugins during their draw hook this frame.
    pub draw_commands: Vec<Line>,
    /// Filled rectangles submitted by plugins during their draw hook.
    pub draw_rects: Vec<Rect>,
    /// Atlas-frame blits submitted by plugins during their draw hook.
    pub draw_images: Vec<Image>,
    /// Autotiled boxes (e.g. introCrusher slabs) submitted during the draw hook.
    pub draw_tile_boxes: Vec<TileBox>,
    /// Seconds remaining on the death freeze; positive while the player is
    /// dead and the room is frozen before respawning.
    pub death_timer: f32,
    /// Entities that have been consumed this session (e.g. a collected
    /// strawberry) and must not be re-created on respawn. Keyed by spawn blob.
    pub collected: HashSet<Vec<u8>>,
    /// World position the player respawns at after a death, if a checkpoint has
    /// been reached this session. `None` = fall back to the level start.
    pub respawn_pos: Option<(f32, f32)>,
    /// Foreground autotiler, used to generate `generate_box` tile grids for
    /// plugins that render autotiled slabs (introCrusher). Set once at startup.
    pub autotiler: Option<Autotiler>,
    /// Cached `generate_box` grids keyed by `(tile_id, tiles_x, tiles_y)` so a
    /// static slab only regenerates its adjacency pass once.
    pub tile_box_cache: HashMap<(char, usize, usize), TileGrid>,
    /// Player dash counts published by the player plugin each frame, keyed by
    /// entity id. Other plugins (refill gems, springs) query them to decide
    /// whether an interaction is needed.
    pub player_dashes: HashMap<u32, i32>,
    /// Player stamina values published by the player plugin each frame.
    pub player_stamina: HashMap<u32, f32>,
    /// Player state numbers published by the player plugin each frame, so
    /// springs etc. can skip interactions the player forbids mid-dash.
    pub player_states: HashMap<u32, u32>,
    /// Per-plugin event delivery cursor: how many entries of `events` each
    /// plugin has already consumed. `host_drain_events` hands each plugin its
    /// own view of the shared bus, so any number of plugins can listen without
    /// stealing events from one another.
    pub plugin_cursors: HashMap<String, usize>,
    /// The plugin currently executing (set around each `update` call so the
    /// event FFI can route delivery).
    pub current_plugin: Option<String>,
}

impl GameState {
    pub fn new(world: World, input: Input, solids: SolidGrid) -> GameState {
        GameState {
            world,
            input,
            solids,
            audio: AudioBus::default(),
            events: Vec::new(),
            draw_commands: Vec::new(),
            draw_rects: Vec::new(),
            draw_images: Vec::new(),
            draw_tile_boxes: Vec::new(),
            death_timer: 0.0,
            collected: HashSet::new(),
            respawn_pos: None,
            autotiler: None,
            tile_box_cache: HashMap::new(),
            player_dashes: HashMap::new(),
            player_stamina: HashMap::new(),
            player_states: HashMap::new(),
            plugin_cursors: HashMap::new(),
            current_plugin: None,
        }
    }
}

struct Plugin {
    name: String,
    types: Vec<String>,
    path: PathBuf,
    mtime: Option<SystemTime>,
    instance: Option<Instance>,
    funcs: PluginFuncs,
}

#[derive(Default)]
struct PluginFuncs {
    update: Option<TypedFunc<(u32, f32), ()>>,
    draw: Option<TypedFunc<(u32,), ()>>,
    destroy: Option<TypedFunc<(u32,), ()>>,
    serialize: Option<TypedFunc<(u32, u32), u32>>,
    deserialize: Option<TypedFunc<(u32, u32, u32), ()>>,
}

pub struct WasmHost {
    engine: Engine,
    store: Store<GameState>,
    plugins: Vec<Plugin>,
    pub plugin_dir: PathBuf,
    /// The level's spawn recipes, used to rebuild the room after a death.
    respawn_entities: Vec<(String, Vec<u8>)>,
    /// The single Madeline's spawn recipe. The player is a persistent singleton:
    /// it is created once and never re-spawned on room switches — only its
    /// respawn point (the current room's player marker) is updated.
    player_spawn: Option<(String, Vec<u8>)>,
    /// Ids of the non-player entities belonging to the currently active room, so
    /// they can be despawned when the player leaves (Celeste keeps only the
    /// current room's entities alive, not the whole map).
    room_entity_ids: Vec<u32>,
}

impl WasmHost {
    pub fn new(
        world: World,
        input: Input,
        solids: SolidGrid,
        plugin_dir: impl Into<PathBuf>,
    ) -> Result<WasmHost> {
        let engine = Engine::new(&wasmtime::Config::new())?;
        let store = Store::new(&engine, GameState::new(world, input, solids));
        DEBUG_ENABLED.store(
            std::env::var("RULESTE_DEBUG")
                .is_ok_and(|v| v != "0" && !v.eq_ignore_ascii_case("false")),
            std::sync::atomic::Ordering::Relaxed,
        );
        Ok(WasmHost {
            engine,
            store,
            plugins: Vec::new(),
            plugin_dir: plugin_dir.into(),
            respawn_entities: Vec::new(),
            player_spawn: None,
            room_entity_ids: Vec::new(),
        })
    }

    /// Creates the single Madeline exactly once (called at map entry) and
    /// remembers her spawn recipe. Do not call this on room switches — the
    /// player must persist across rooms.
    pub fn spawn_player_once(&mut self, spawn: (String, Vec<u8>)) {
        self.player_spawn = Some(spawn.clone());
        if let Err(e) = self.spawn_entity(&spawn.0, spawn.1) {
            eprintln!("ruleste: spawn player failed: {e}");
        }
        self.rebuild_respawn();
    }

    /// Activates a room's non-player entities: the previous room's entities are
    /// despawned and the new room's are spawned (lazy, current-room-only — not
    /// the whole map). The persistent player is left untouched; only its respawn
    /// point is updated to `player_respawn` (the new room's player marker).
    pub fn enter_room(&mut self, spawns: &[(String, Vec<u8>)], player_respawn: (String, Vec<u8>)) {
        let ids = std::mem::take(&mut self.room_entity_ids);
        for id in ids {
            self.despawn(id);
        }
        self.player_spawn = Some(player_respawn);
        for (ty, s) in spawns {
            if let Ok(Some(id)) = self.spawn_entity(ty, s.clone()) {
                self.room_entity_ids.push(id);
            }
        }
        self.rebuild_respawn();
    }

    /// Rebuilds `respawn_entities` from the (single) player recipe plus the
    /// active room's entities, so a death reconstructs the current room only.
    fn rebuild_respawn(&mut self) {
        let mut v = Vec::new();
        if let Some(p) = &self.player_spawn {
            v.push(p.clone());
        }
        v.extend(self.room_entity_ids.iter().filter_map(|id| {
            self.store.data().world.get(*id).map(|e| (e.entity_type.clone(), e.spawn.clone()))
        }));
        self.respawn_entities = v;
    }

    /// Provides the foreground autotiler plugins use for `generate_box` tile
    /// slabs (e.g. introCrusher). Call before spawning entities.
    pub fn set_autotiler(&mut self, autotiler: Autotiler) {
        self.store.data_mut().autotiler = Some(autotiler);
    }

    pub fn game_state(&mut self) -> &mut GameState {
        self.store.data_mut()
    }

    /// Loads every `*.wasm` in the plugin directory.
    pub fn load_plugins(&mut self) -> Result<()> {
        self.load_plugins_filtered(None)
    }

    /// Loads only the plugins that handle at least one of the given entity
    /// types. `Some(empty)` loads nothing; `None` loads everything. Used by the
    /// game to instantiate just the plugins a level's entities actually need.
    pub fn load_plugins_for(
        &mut self,
        needed_types: &std::collections::HashSet<String>,
    ) -> Result<()> {
        self.load_plugins_filtered(Some(needed_types))
    }

    fn load_plugins_filtered(
        &mut self,
        needed_types: Option<&std::collections::HashSet<String>>,
    ) -> Result<()> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.plugin_dir)
            .with_context(|| format!("read plugin dir {}", self.plugin_dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "wasm"))
            .collect();
        entries.sort();
        for path in entries {
            // Peek at which entity types the module declares before
            // instantiating it, so unrelated plugins never get loaded.
            if let Some(needed) = needed_types {
                let types = match self.peek_plugin_types(&path) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("ruleste: skip plugin {}: {e:#}", path.display());
                        continue;
                    }
                };
                if types.is_empty() || !types.iter().any(|t| needed.contains(t)) {
                    continue;
                }
            }
            if let Err(e) = self.load_plugin(&path) {
                eprintln!("ruleste: failed to load plugin {}: {e:#}", path.display());
            }
        }
        Ok(())
    }

    /// Compiles a module and reads its declared entity types without keeping
    /// an instance. Cheaper than loading the whole plugin when filtering.
    fn peek_plugin_types(&mut self, path: &Path) -> Result<Vec<String>> {
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| anyhow!("compile {}: {e}", path.display()))?;
        let linker = self.build_linker(&self.engine)?;
        let instance = linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| anyhow!("instantiate {}: {e}", path.display()))?;
        self.read_plugin_types(&instance)
    }

    pub fn load_plugin(&mut self, path: &Path) -> Result<()> {
        let plugin = self.build_plugin(path)?;
        eprintln!(
            "ruleste: loaded plugin {:?} (types: {:?})",
            plugin.name, plugin.types
        );
        self.plugins.push(plugin);
        Ok(())
    }

    fn build_plugin(&mut self, path: &Path) -> Result<Plugin> {
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| anyhow!("compile {}: {e}", path.display()))?;
        let linker = self.build_linker(&self.engine)?;
        let instance = linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| anyhow!("instantiate {}: {e}", path.display()))?;

        let name = self.read_plugin_name(&instance)?;
        let types = self.read_plugin_types(&instance)?;
        let funcs = PluginFuncs {
            update: instance
                .get_typed_func(&mut self.store, export::UPDATE)
                .ok(),
            draw: instance.get_typed_func(&mut self.store, export::DRAW).ok(),
            destroy: instance
                .get_typed_func(&mut self.store, export::DESTROY)
                .ok(),
            serialize: instance
                .get_typed_func(&mut self.store, export::SERIALIZE)
                .ok(),
            deserialize: instance
                .get_typed_func(&mut self.store, export::DESERIALIZE)
                .ok(),
        };
        if types.is_empty() {
            return Err(anyhow!("plugin {} handles no entity types", path.display()));
        }

        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        Ok(Plugin {
            name,
            types,
            path: path.to_path_buf(),
            mtime,
            instance: Some(instance),
            funcs,
        })
    }

    pub fn plugin_for_type(&self, entity_type: &str) -> Option<usize> {
        self.plugins
            .iter()
            .position(|p| p.instance.is_some() && p.types.iter().any(|t| t == entity_type))
    }

    /// Spawns an entity handled by a plugin. Returns `None` if no plugin owns
    /// this entity type.
    pub fn spawn_entity(&mut self, entity_type: &str, spawn: Vec<u8>) -> Result<Option<u32>> {
        let Some(idx) = self.plugin_for_type(entity_type) else {
            return Ok(None);
        };
        let id = {
            let state = self.store.data_mut();
            let e = state.world.spawn();
            let e = state.world.get_mut(e).expect("fresh entity");
            e.plugin = self.plugins[idx].name.clone();
            e.entity_type = entity_type.to_string();
            e.spawn = spawn.clone();
            e.sprite.sprite = entity_type.to_string();
            e.id
        };
        let instance = self.plugins[idx].instance.as_ref().expect("plugin loaded");
        if let Err(e) = call_init(&mut self.store, instance, id, &spawn) {
            self.store.data_mut().world.despawn(id);
            return Err(e);
        }
        Ok(Some(id))
    }

    /// Runs every plugin's per-entity update, then flushes the event queue.
    ///
    /// While the player is dead (`death_timer > 0`) the room is frozen: entity
    /// updates are skipped, mirroring how the original engine stops time during
    /// the death freeze. When the timer elapses the room is rebuilt.
    pub fn update(&mut self, dt: f32) {
        let mut respawn = false;
        {
            let state = self.store.data_mut();
            if state.death_timer > 0.0 {
                state.death_timer -= dt;
                respawn = state.death_timer <= 0.0;
            }
        }
        if respawn {
            self.respawn();
            return;
        }
        if self.store.data().death_timer > 0.0 {
            return;
        }
        let jobs: Vec<(u32, usize)> = self
            .store
            .data()
            .world
            .iter()
            .filter_map(|e| {
                self.plugins
                    .iter()
                    .position(|p| p.name == e.plugin)
                    .map(|i| (e.id, i))
            })
            .collect();
        for (id, idx) in jobs {
            let Some(update) = &self.plugins[idx].funcs.update else {
                continue;
            };
            self.store.data_mut().current_plugin = Some(self.plugins[idx].name.clone());
            if let Err(e) = update.call(&mut self.store, (id, dt)) {
                eprintln!("ruleste: plugin update error (entity {id}): {e}");
            }
        }
        self.store.data_mut().current_plugin = None;
        // Garbage-collect events every plugin has already consumed.
        let min_consumed = self
            .store
            .data()
            .plugin_cursors
            .values()
            .copied()
            .min()
            .unwrap_or(0);
        if min_consumed > 0 {
            let state = self.store.data_mut();
            state.events.drain(..min_consumed);
            for cursor in state.plugin_cursors.values_mut() {
                *cursor = cursor.saturating_sub(min_consumed);
            }
        }
        // A plugin may have requested a death this frame; freeze now instead of
        // respawning until the timer elapses (handled at the top of `update`).
    }

    /// Triggers a death: freezes the room and schedules a respawn.
    pub fn kill_player(&mut self) {
        self.store.data_mut().death_timer = DEATH_FREEZE_TIME;
        let ids: Vec<u32> = self
            .store
            .data()
            .world
            .iter()
            .filter(|e| e.entity_type == "player")
            .map(|e| e.id)
            .collect();
        for id in ids {
            if let Some(e) = self.store.data_mut().world.get_mut(id) {
                e.visible = false;
            }
        }
    }

    /// Rebuilds the room from the recorded spawn recipes, skipping entities
    /// that were consumed this session.
    pub fn respawn(&mut self) {
        self.store.data_mut().death_timer = 0.0;
        let ids: Vec<u32> = self.store.data().world.iter().map(|e| e.id).collect();
        for id in ids {
            self.despawn(id);
        }
        let collected = self.store.data().collected.clone();
        let recipes = self.respawn_entities.clone();
        for (entity_type, spawn) in &recipes {
            if collected.contains(spawn) {
                continue;
            }
            if let Err(e) = self.spawn_entity(entity_type, spawn.clone()) {
                eprintln!("ruleste: respawn of {entity_type} failed: {e}");
            }
        }
        // A reached checkpoint overrides the level-start spawn position.
        if let Some((x, y)) = self.store.data().respawn_pos {
            let ids: Vec<u32> = self
                .store
                .data()
                .world
                .iter()
                .filter(|e| e.entity_type == "player")
                .map(|e| e.id)
                .collect();
            for id in ids {
                if let Some(e) = self.store.data_mut().world.get_mut(id) {
                    e.position = ruleste_plugins_api::types::Vec2::new(x, y);
                }
            }
        }
        // Keep `room_entity_ids` in sync with the rebuilt world (everything that
        // isn't the persistent player belongs to the active room).
        self.room_entity_ids = self
            .store
            .data()
            .world
            .iter()
            .filter(|e| e.entity_type != "player")
            .map(|e| e.id)
            .collect();
    }

    pub fn draw(&mut self) {
        self.store.data_mut().draw_commands.clear();
        self.store.data_mut().draw_rects.clear();
        self.store.data_mut().draw_images.clear();
        self.store.data_mut().draw_tile_boxes.clear();
        let jobs: Vec<(u32, usize)> = self
            .store
            .data()
            .world
            .iter()
            .filter_map(|e| {
                self.plugins
                    .iter()
                    .position(|p| p.name == e.plugin)
                    .map(|i| (e.id, i))
            })
            .collect();
        for (id, idx) in jobs {
            let Some(draw) = &self.plugins[idx].funcs.draw else {
                continue;
            };
            if let Err(e) = draw.call(&mut self.store, (id,)) {
                eprintln!("ruleste: plugin draw error (entity {id}): {e}");
            }
        }
    }

    pub fn despawn(&mut self, id: u32) {
        let entity = self.store.data().world.get(id).cloned();
        if let Some(entity) = entity {
            if let Some(idx) = self.plugins.iter().position(|p| p.name == entity.plugin) {
                if let Some(destroy) = &self.plugins[idx].funcs.destroy {
                    let _ = destroy.call(&mut self.store, (id,));
                }
            }
            self.store.data_mut().world.despawn(id);
        }
    }

    /// Re-checks plugin files for mtime changes and reloads any that changed.
    pub fn reload_plugins(&mut self) -> Result<()> {
        let changed: Vec<usize> = self
            .plugins
            .iter()
            .enumerate()
            .filter_map(|(i, plugin)| {
                let current = std::fs::metadata(&plugin.path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                (plugin.mtime != current).then_some(i)
            })
            .collect();
        for idx in changed {
            self.reload_plugin(idx)?;
        }
        Ok(())
    }

    fn reload_plugin(&mut self, idx: usize) -> Result<()> {
        let path = self.plugins[idx].path.clone();
        let owner = self.plugins[idx].name.clone();
        let instance = self.plugins[idx].instance.as_ref().expect("plugin present");

        // Snapshot the state of every entity this plugin owns. The plugin's
        // serializer hands us a pointer/length; we copy the bytes out before
        // dropping the module.
        let owned: Vec<(u32, Vec<u8>)> = self
            .store
            .data()
            .world
            .iter()
            .filter(|e| e.plugin == owner)
            .map(|e| (e.id, e.spawn.clone()))
            .collect();
        let mut states = HashMap::new();
        if let Some(serialize) = &self.plugins[idx].funcs.serialize {
            for (id, _) in &owned {
                let len_ptr = match write_buffer(&mut self.store, instance, &[0u8; 4]) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("ruleste: serialize alloc for entity {id}: {e}");
                        continue;
                    }
                };
                let call = serialize.call(&mut self.store, (*id, len_ptr));
                let len = read_word(&mut self.store, instance, len_ptr).unwrap_or(0);
                let _ = free_buffer(&mut self.store, instance, len_ptr, 4);
                match call {
                    Ok(ptr) if ptr != 0 => {
                        if let Some(bytes) =
                            read_bytes_at(instance, &mut self.store, ptr, len as usize)
                        {
                            states.insert(*id, bytes);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("ruleste: serialize entity {id}: {e}"),
                }
            }
        }

        self.plugins[idx] = self.build_plugin(&path)?;
        let instance = self.plugins[idx].instance.as_ref().expect("reloaded");

        for (id, spawn) in &owned {
            if let Err(e) = call_init(&mut self.store, instance, *id, spawn) {
                eprintln!("ruleste: re-init entity {id} after reload: {e}");
                continue;
            }
            if let Some(restore) = &self.plugins[idx].funcs.deserialize {
                if let Some(buf) = states.get(id) {
                    let ptr = match write_buffer(&mut self.store, instance, buf) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("ruleste: deserialize alloc for entity {id}: {e}");
                            continue;
                        }
                    };
                    let result = restore.call(&mut self.store, (*id, ptr, buf.len() as u32));
                    let _ = free_buffer(&mut self.store, instance, ptr, buf.len() as u32);
                    if let Err(e) = result {
                        eprintln!("ruleste: deserialize entity {id}: {e}");
                    }
                }
            }
        }
        eprintln!("ruleste: hot-reloaded plugin {owner:?}");
        Ok(())
    }

    // ---------------------------------------------------------------------
    // FFI host functions
    // ---------------------------------------------------------------------

    fn build_linker(&self, engine: &Engine) -> Result<Linker<GameState>> {
        let mut linker = Linker::<GameState>::new(engine);

        linker.func_wrap(
            "env",
            "host_position_get",
            |mut caller: Caller<'_, GameState>, id: u32, out: u32| {
                let (x, y) =
                    world_get(&caller, id, |e| (e.position.x, e.position.y)).unwrap_or((0.0, 0.0));
                write_vec2(&mut caller, out, x, y);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_position_set",
            |mut caller: Caller<'_, GameState>, id: u32, x: f32, y: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.position = ruleste_plugins_api::types::Vec2::new(x, y);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_speed_get",
            |mut caller: Caller<'_, GameState>, id: u32, out: u32| {
                let (x, y) =
                    world_get(&caller, id, |e| (e.speed.x, e.speed.y)).unwrap_or((0.0, 0.0));
                write_vec2(&mut caller, out, x, y);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_speed_set",
            |mut caller: Caller<'_, GameState>, id: u32, x: f32, y: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.speed = ruleste_plugins_api::types::Vec2::new(x, y);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_hitbox_set",
            |mut caller: Caller<'_, GameState>, id: u32, w: f32, h: f32, ox: f32, oy: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.hitbox = ruleste_plugins_api::types::Vec2::new(w, h);
                    e.hitbox_offset = ruleste_plugins_api::types::Vec2::new(ox, oy);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_hitbox_get",
            |mut caller: Caller<'_, GameState>, id: u32, out: u32| {
                let buf = caller
                    .data()
                    .world
                    .get(id)
                    .map(|e| {
                        let mut b = [0f32; 4];
                        b[0] = e.hitbox.x;
                        b[1] = e.hitbox.y;
                        b[2] = e.hitbox_offset.x;
                        b[3] = e.hitbox_offset.y;
                        b
                    })
                    .unwrap_or([0f32; 4]);
                if let Some(mem) = plugin_memory(&mut caller) {
                    let bytes: Vec<u8> = buf.iter().flat_map(|v| v.to_le_bytes()).collect();
                    let _ = mem.write(caller, out as usize, &bytes);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_depth_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller.data().world.get(id).map_or(0, |e| e.depth)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_depth_set",
            |mut caller: Caller<'_, GameState>, id: u32, depth: i32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.depth = depth;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_visible_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller
                    .data()
                    .world
                    .get(id)
                    .map_or(1i32, |e| i32::from(e.visible))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_visible_set",
            |mut caller: Caller<'_, GameState>, id: u32, visible: i32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.visible = visible != 0;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_play",
            |mut caller: Caller<'_, GameState>, id: u32, name: u32, len: u32| {
                let s = read_string(&mut caller, name, len);
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.animation = s;
                    e.sprite.frame = 0.0;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_bank_set",
            |mut caller: Caller<'_, GameState>, id: u32, name: u32, len: u32| {
                let s = read_string(&mut caller, name, len);
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.sprite = s;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_animation",
            |mut caller: Caller<'_, GameState>, id: u32, out: u32, cap: u32| {
                let s = world_get(&caller, id, |e| e.sprite.animation.clone()).unwrap_or_default();
                write_string(&mut caller, out, cap, &s) as u32
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_frame_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller.data().world.get(id).map_or(0.0, |e| e.sprite.frame)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_frame_set",
            |mut caller: Caller<'_, GameState>, id: u32, frame: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.frame = frame;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_rate_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller.data().world.get(id).map_or(1.0, |e| e.sprite.rate)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_rate_set",
            |mut caller: Caller<'_, GameState>, id: u32, rate: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.rate = rate;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_color_set",
            |mut caller: Caller<'_, GameState>, id: u32, packed: u32| {
                let color = Color::new(
                    (packed >> 24) as u8,
                    (packed >> 16) as u8,
                    (packed >> 8) as u8,
                    packed as u8,
                );
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.color = color;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_flip_x_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller
                    .data()
                    .world
                    .get(id)
                    .map_or(0i32, |e| i32::from(e.sprite.flip_x))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_flip_x_set",
            |mut caller: Caller<'_, GameState>, id: u32, flip: i32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.flip_x = flip != 0;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_flip_y_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller
                    .data()
                    .world
                    .get(id)
                    .map_or(0i32, |e| i32::from(e.sprite.flip_y))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_sprite_flip_y_set",
            |mut caller: Caller<'_, GameState>, id: u32, flip: i32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.sprite.flip_y = flip != 0;
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_input_axis",
            |caller: Caller<'_, GameState>, action: i32| caller.data().input.axis(action),
        )?;
        linker.func_wrap(
            "env",
            "host_input_button",
            |caller: Caller<'_, GameState>, action: i32| {
                i32::from(caller.data().input.button(action))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_input_pressed",
            |caller: Caller<'_, GameState>, action: i32| {
                i32::from(caller.data().input.pressed(action))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_input_released",
            |caller: Caller<'_, GameState>, action: i32| {
                i32::from(caller.data().input.released(action))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_input_consume",
            |mut caller: Caller<'_, GameState>, action: i32| {
                caller.data_mut().input.consume(action);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_collide_check",
            |mut caller: Caller<'_, GameState>, id: u32, ox: f32, oy: f32| {
                let state = caller.data_mut();
                i32::from(state.solids.entity_collide(&state.world, id, ox, oy))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_collide_solid_platform_set",
            |mut caller: Caller<'_, GameState>, id: u32, on: i32| {
                let state = caller.data_mut();
                crate::engine::physics::SolidGrid::mark_solid_platform(
                    &mut state.world,
                    id,
                    on != 0,
                );
            },
        )?;
        linker.func_wrap(
            "env",
            "host_collide_solid_set",
            |mut caller: Caller<'_, GameState>, id: u32, on: i32| {
                let state = caller.data_mut();
                crate::engine::physics::SolidGrid::mark_solid_entity(&mut state.world, id, on != 0);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_actor_move",
            |mut caller: Caller<'_, GameState>, id: u32, h: f32, v: f32| {
                let state = caller.data_mut();
                state.solids.actor_move(&mut state.world, id, h, v)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_actor_is_grounded",
            |mut caller: Caller<'_, GameState>, id: u32| {
                let state = caller.data_mut();
                i32::from(state.solids.is_grounded(&state.world, id))
            },
        )?;
        linker.func_wrap(
            "env",
            "host_play_sound",
            |mut caller: Caller<'_, GameState>, name: u32, len: u32| {
                let s = read_string(&mut caller, name, len);
                caller.data_mut().audio.play(&s, 1.0);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_log",
            |mut caller: Caller<'_, GameState>, msg: u32, len: u32| {
                let s = read_string(&mut caller, msg, len);
                eprintln!("[plugin] {s}");
            },
        )?;
        linker.func_wrap(
            "env",
            "host_emit",
            |mut caller: Caller<'_, GameState>, id: u32, event: u32, data: u32, len: u32| {
                let buf = read_bytes(&mut caller, data, len);
                caller.data_mut().events.push(GameEvent {
                    entity: id,
                    kind: event,
                    data: buf,
                });
            },
        )?;
        linker.func_wrap(
            "env",
            "host_draw_line",
            |mut caller: Caller<'_, GameState>,
             x1: f32,
             y1: f32,
             x2: f32,
             y2: f32,
             r: u32,
             g: u32,
             b: u32,
             a: u32| {
                caller.data_mut().draw_commands.push(Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: Color {
                        r: r as u8,
                        g: g as u8,
                        b: b as u8,
                        a: a as u8,
                    },
                });
            },
        )?;
        linker.func_wrap(
            "env",
            "host_draw_rect",
            |mut caller: Caller<'_, GameState>,
             x: f32,
             y: f32,
             w: f32,
             h: f32,
             r: u32,
             g: u32,
             b: u32,
             a: u32| {
                caller.data_mut().draw_rects.push(Rect {
                    x,
                    y,
                    w,
                    h,
                    color: Color {
                        r: r as u8,
                        g: g as u8,
                        b: b as u8,
                        a: a as u8,
                    },
                });
            },
        )?;
        // --- Kill the player: freeze the room, then respawn ---
        linker.func_wrap("env", "host_die", |mut caller: Caller<'_, GameState>| {
            let state = caller.data_mut();
            if state.death_timer <= 0.0 {
                state.death_timer = DEATH_FREEZE_TIME;
                let ids: Vec<u32> = state
                    .world
                    .iter()
                    .filter(|e| e.entity_type == "player")
                    .map(|e| e.id)
                    .collect();
                for id in ids {
                    if let Some(e) = state.world.get_mut(id) {
                        e.visible = false;
                    }
                }
            }
        })?;
        // --- Consume an entity this session (won't respawn on death) ---
        linker.func_wrap(
            "env",
            "host_collect",
            |mut caller: Caller<'_, GameState>, id: u32| {
                let state = caller.data_mut();
                if let Some(e) = state.world.get(id).cloned() {
                    state.collected.insert(e.spawn.clone());
                    state.world.despawn(id);
                }
            },
        )?;
        // --- Remove an entity now (it still respawns on death) ---
        linker.func_wrap(
            "env",
            "host_remove",
            |mut caller: Caller<'_, GameState>, id: u32| {
                caller.data_mut().world.despawn(id);
            },
        )?;
        // --- Set the player's respawn position (used by checkpoints) ---
        linker.func_wrap(
            "env",
            "host_respawn_set",
            |mut caller: Caller<'_, GameState>, x: f32, y: f32| {
                caller.data_mut().respawn_pos = Some((x, y));
            },
        )?;
        // --- Read the current respawn position, if any ---
        linker.func_wrap(
            "env",
            "host_respawn_get",
            |mut caller: Caller<'_, GameState>, out: u32| {
                let pos = caller.data().respawn_pos;
                write_vec2(
                    &mut caller,
                    out,
                    pos.map_or(0.0, |p| p.0),
                    pos.map_or(0.0, |p| p.1),
                );
            },
        )?;
        // --- Blit an atlas frame at an arbitrary world position ---
        linker.func_wrap(
            "env",
            "host_draw_image",
            |mut caller: Caller<'_, GameState>,
             frame_ptr: u32,
             frame_len: u32,
             x: f32,
             y: f32,
             rotation: f32,
             scale_x: f32,
             scale_y: f32,
             flip_x: i32,
             flip_y: i32,
             r: u32,
             g: u32,
             b: u32,
             a: u32| {
                let frame_id = read_string(&mut caller, frame_ptr, frame_len);
                caller.data_mut().draw_images.push(Image {
                    frame_id,
                    x,
                    y,
                    rotation,
                    scale_x,
                    scale_y,
                    flip_x: flip_x != 0,
                    flip_y: flip_y != 0,
                    color: Color {
                        r: r as u8,
                        g: g as u8,
                        b: b as u8,
                        a: a as u8,
                    },
                });
            },
        )?;
        // --- Draw an autotiled box of tiles (Autotiler.GenerateBox) ---
        linker.func_wrap(
            "env",
            "host_draw_tile_box",
            |mut caller: Caller<'_, GameState>,
             tile_id: u32,
             x: f32,
             y: f32,
             tiles_x: u32,
             tiles_y: u32| {
                let tile_char = tile_id as u8 as char;
                let (tx, ty) = (tiles_x as usize, tiles_y as usize);
                if tx == 0 || ty == 0 {
                    return;
                }
                let state = caller.data_mut();
                let Some(autotiler) = &state.autotiler else {
                    return;
                };
                let grid = state
                    .tile_box_cache
                    .entry((tile_char, tx, ty))
                    .or_insert_with(|| {
                        autotiler
                            .generate_box(tile_char, tx, ty)
                            .unwrap_or_else(|| TileGrid {
                                width: tx,
                                height: ty,
                                tileset: vec![String::new(); tx * ty],
                                col: vec![0; tx * ty],
                                row: vec![0; tx * ty],
                                origin_x: 0.0,
                                origin_y: 0.0,
                            })
                    });
                if grid.tileset[0].is_empty() {
                    return;
                }
                state.draw_tile_boxes.push(TileBox {
                    frame_id: format!("tilesets/{}", grid.tileset[0]),
                    x,
                    y,
                    width: tx,
                    height: ty,
                    col: grid.col.clone(),
                    row: grid.row.clone(),
                });
            },
        )?;
        // --- Entity query: find all entities of a given type ---
        linker.func_wrap(
            "env",
            "host_entities_by_type",
            |mut caller: Caller<'_, GameState>,
             type_ptr: u32,
             type_len: u32,
             out_ids_ptr: u32,
             max_count: u32| {
                let type_name = read_string(&mut caller, type_ptr, type_len);
                let ids: Vec<u32> = caller
                    .data()
                    .world
                    .iter()
                    .filter(|e| e.entity_type == type_name)
                    .map(|e| e.id)
                    .take(max_count as usize)
                    .collect();
                let count = ids.len() as u32;
                if let Some(mem) = plugin_memory(&mut caller) {
                    let bytes: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
                    let _ = mem.write(caller, out_ids_ptr as usize, &bytes);
                }
                count
            },
        )?;
        // --- Drain event queue ---
        //
        // Delivers only the events this plugin has not consumed yet: each plugin
        // keeps its own cursor, so a shared bus can serve any number of
        // listeners (the player consumes `EV_LAUNCH`/`EV_REFILL` while swap and
        // dash blocks watch `PLAYER_DASH`).
        linker.func_wrap(
            "env",
            "host_drain_events",
            |mut caller: Caller<'_, GameState>, out_buf: u32, buf_cap: u32| {
                let state = caller.data();
                let Some(plugin) = state.current_plugin.clone() else {
                    return 0;
                };
                let start = state.plugin_cursors.get(&plugin).copied().unwrap_or(0);
                let events_len = state.events.len();
                let mut buf = Vec::new();
                for ev in state.events.iter().skip(start) {
                    buf.extend_from_slice(&ev.entity.to_le_bytes());
                    buf.extend_from_slice(&ev.kind.to_le_bytes());
                    buf.extend_from_slice(&(ev.data.len() as u32).to_le_bytes());
                    buf.extend_from_slice(&ev.data);
                }
                let total = buf.len().min(buf_cap as usize);
                if let Some(mem) = plugin_memory(&mut caller) {
                    let _ = mem.write(&mut caller, out_buf as usize, &buf[..total]);
                }
                if let Some(cursor) = caller.data_mut().plugin_cursors.get_mut(&plugin) {
                    *cursor = events_len;
                }
                total as u32
            },
        )?;
        // --- Check if an entity is alive ---
        linker.func_wrap(
            "env",
            "host_entity_alive",
            |caller: Caller<'_, GameState>, id: u32| {
                i32::from(caller.data().world.get(id).is_some())
            },
        )?;
        // --- Verbose plugin debug logging (RULESTE_DEBUG=1) ---
        linker.func_wrap("env", "host_debug_enabled", |_: Caller<'_, GameState>| {
            i32::from(DEBUG_ENABLED.load(std::sync::atomic::Ordering::Relaxed))
        })?;
        // --- Player resource access (published by the player plugin) ---
        linker.func_wrap(
            "env",
            "host_player_dashes_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller.data().player_dashes.get(&id).copied().unwrap_or(0)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_player_dashes_set",
            |mut caller: Caller<'_, GameState>, id: u32, dashes: i32| {
                caller.data_mut().player_dashes.insert(id, dashes);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_player_stamina_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller
                    .data()
                    .player_stamina
                    .get(&id)
                    .copied()
                    .unwrap_or(0.0)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_player_stamina_set",
            |mut caller: Caller<'_, GameState>, id: u32, stamina: f32| {
                caller.data_mut().player_stamina.insert(id, stamina);
            },
        )?;
        linker.func_wrap(
            "env",
            "host_player_state_get",
            |caller: Caller<'_, GameState>, id: u32| {
                caller.data().player_states.get(&id).copied().unwrap_or(0)
            },
        )?;
        linker.func_wrap(
            "env",
            "host_player_state_set",
            |mut caller: Caller<'_, GameState>, id: u32, state: u32| {
                caller.data_mut().player_states.insert(id, state);
            },
        )?;
        Ok(linker)
    }

    // ---------------------------------------------------------------------
    // buffer helpers
    // ---------------------------------------------------------------------

    fn read_plugin_name(&mut self, instance: &Instance) -> Result<String> {
        let Some(func) = instance
            .get_typed_func::<(), u32>(&mut self.store, export::META)
            .ok()
        else {
            return Ok(String::new());
        };
        let ptr = func.call(&mut self.store, ())?;
        Ok(read_cstring(instance, &mut self.store, ptr).unwrap_or_default())
    }

    fn read_plugin_types(&mut self, instance: &Instance) -> Result<Vec<String>> {
        let Some(func) = instance
            .get_typed_func::<(u32,), u32>(&mut self.store, export::ENTITY_TYPES)
            .ok()
        else {
            return Ok(Vec::new());
        };
        let len_ptr = write_buffer(&mut self.store, instance, &[0u8; 4])?;
        let call = func.call(&mut self.store, (len_ptr,));
        let len = read_word(&mut self.store, instance, len_ptr)?;
        free_buffer(&mut self.store, instance, len_ptr, 4)?;
        let ptr = call?;
        let bytes = read_bytes_at(instance, &mut self.store, ptr, len as usize).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        Ok(text
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(str::trim)
            .map(str::to_string)
            .collect())
    }
}

fn world_get<T>(
    caller: &Caller<'_, GameState>,
    id: u32,
    f: impl Fn(&crate::engine::ecs::Entity) -> T,
) -> Option<T> {
    caller.data().world.get(id).map(f)
}

fn plugin_memory(caller: &mut Caller<'_, GameState>) -> Option<Memory> {
    caller.get_export("memory").and_then(Extern::into_memory)
}

fn read_bytes(caller: &mut Caller<'_, GameState>, ptr: u32, len: u32) -> Vec<u8> {
    let Some(mem) = plugin_memory(caller) else {
        return Vec::new();
    };
    let mut buf = vec![0u8; len as usize];
    if mem.read(caller, ptr as usize, &mut buf).is_err() {
        return Vec::new();
    }
    buf
}

fn read_string(caller: &mut Caller<'_, GameState>, ptr: u32, len: u32) -> String {
    let bytes = read_bytes(caller, ptr, len);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn write_vec2(caller: &mut Caller<'_, GameState>, ptr: u32, x: f32, y: f32) {
    if let Some(mem) = plugin_memory(caller) {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&x.to_le_bytes());
        buf[4..8].copy_from_slice(&y.to_le_bytes());
        let _ = mem.write(caller, ptr as usize, &buf);
    }
}

fn write_string(caller: &mut Caller<'_, GameState>, ptr: u32, cap: u32, s: &str) -> usize {
    let Some(mem) = plugin_memory(caller) else {
        return 0;
    };
    let bytes = s.as_bytes();
    let n = bytes.len().min(cap as usize);
    let _ = mem.write(caller, ptr as usize, &bytes[..n]);
    n
}

fn read_cstring<T>(instance: &Instance, store: &mut Store<T>, ptr: u32) -> Option<String> {
    let memory = instance.get_memory(&mut *store, "memory")?;
    let data = memory.data(&*store);
    let bytes = data.get(ptr as usize..)?;
    let end = bytes.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

fn read_bytes_at<T>(
    instance: &Instance,
    store: &mut Store<T>,
    ptr: u32,
    len: usize,
) -> Option<Vec<u8>> {
    let memory = instance.get_memory(&mut *store, "memory")?;
    let data = memory.data(&*store);
    let bytes = data.get(ptr as usize..)?.get(..len)?;
    Some(bytes.to_vec())
}

/// Calls the plugin's `entity_init` export, copying `spawn` into plugin memory.
fn call_init(
    store: &mut Store<GameState>,
    instance: &Instance,
    id: u32,
    spawn: &[u8],
) -> Result<()> {
    let Some(init) = instance
        .get_typed_func::<(u32, u32, u32), ()>(&mut *store, export::INIT)
        .ok()
    else {
        return Ok(());
    };
    let ptr = write_buffer(store, instance, spawn)?;
    let result = init.call(&mut *store, (id, ptr, spawn.len() as u32));
    free_buffer(store, instance, ptr, spawn.len() as u32)?;
    result?;
    Ok(())
}

/// Allocates room in plugin memory via the plugin's own allocator and
/// copies `data` in. Returns the plugin-side pointer.
fn write_buffer(store: &mut Store<GameState>, instance: &Instance, data: &[u8]) -> Result<u32> {
    let alloc = instance
        .get_typed_func::<u32, u32>(&mut *store, SCRATCH_ALLOC)
        .map_err(|e| anyhow!("plugin missing {SCRATCH_ALLOC} export: {e}"))?;
    let ptr = alloc.call(&mut *store, data.len() as u32)?;
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| anyhow!("plugin has no memory export"))?;
    memory.write(&mut *store, ptr as usize, data)?;
    Ok(ptr)
}

/// Reads a single `u32` from plugin linear memory at `ptr`.
fn read_word(store: &mut Store<GameState>, instance: &Instance, ptr: u32) -> Result<u32> {
    let bytes = read_bytes_at(instance, &mut *store, ptr, 4)
        .ok_or_else(|| anyhow!("failed to read word at {ptr:#x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn free_buffer(
    store: &mut Store<GameState>,
    instance: &Instance,
    ptr: u32,
    len: u32,
) -> Result<()> {
    let dealloc = instance
        .get_typed_func::<(u32, u32), ()>(&mut *store, SCRATCH_DEALLOC)
        .map_err(|e| anyhow!("plugin missing {SCRATCH_DEALLOC} export: {e}"))?;
    dealloc.call(&mut *store, (ptr, len))?;
    Ok(())
}
