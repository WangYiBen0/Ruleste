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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use ruleste_plugin_api::export;
use ruleste_plugin_api::types::Color;
use wasmtime::{Caller, Engine, Extern, Instance, Linker, Memory, Module, Store, TypedFunc};

use crate::engine::ecs::World;
use crate::engine::input::Input;
use crate::engine::physics::SolidGrid;

pub const SCRATCH_ALLOC: &str = "ruleste_alloc";
pub const SCRATCH_DEALLOC: &str = "ruleste_dealloc";

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
}

impl GameState {
    pub fn new(world: World, input: Input, solids: SolidGrid) -> GameState {
        GameState {
            world,
            input,
            solids,
            audio: AudioBus::default(),
            events: Vec::new(),
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
        Ok(WasmHost {
            engine,
            store,
            plugins: Vec::new(),
            plugin_dir: plugin_dir.into(),
        })
    }

    pub fn game_state(&mut self) -> &mut GameState {
        self.store.data_mut()
    }

    /// Loads every `*.wasm` in the plugin directory.
    pub fn load_plugins(&mut self) -> Result<()> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.plugin_dir)
            .with_context(|| format!("read plugin dir {}", self.plugin_dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "wasm"))
            .collect();
        entries.sort();
        for path in entries {
            if let Err(e) = self.load_plugin(&path) {
                eprintln!("ruleste: failed to load plugin {}: {e:#}", path.display());
            }
        }
        Ok(())
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
        call_init(&mut self.store, instance, id, &spawn)?;
        Ok(Some(id))
    }

    /// Runs every plugin's per-entity update, then flushes the event queue.
    pub fn update(&mut self, dt: f32) {
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
            if let Err(e) = update.call(&mut self.store, (id, dt)) {
                eprintln!("ruleste: plugin update error (entity {id}): {e}");
            }
        }
    }

    pub fn draw(&mut self) {
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
                let mut len: u32 = 0;
                match serialize.call(&mut self.store, (*id, std::ptr::addr_of_mut!(len) as u32)) {
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
                    let ptr = write_buffer(&mut self.store, instance, buf)?;
                    let result = restore.call(&mut self.store, (*id, ptr, buf.len() as u32));
                    free_buffer(&mut self.store, instance, ptr, buf.len() as u32)?;
                    result?;
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
                    e.position = ruleste_plugin_api::types::Vec2::new(x, y);
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
                    e.speed = ruleste_plugin_api::types::Vec2::new(x, y);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "host_hitbox_set",
            |mut caller: Caller<'_, GameState>, id: u32, w: f32, h: f32, ox: f32, oy: f32| {
                if let Some(e) = caller.data_mut().world.get_mut(id) {
                    e.hitbox = ruleste_plugin_api::types::Vec2::new(w, h);
                    e.hitbox_offset = ruleste_plugin_api::types::Vec2::new(ox, oy);
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
            "host_collide_check",
            |mut caller: Caller<'_, GameState>, id: u32, ox: f32, oy: f32| {
                let state = caller.data_mut();
                i32::from(state.solids.entity_collide(&state.world, id, ox, oy))
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
        let mut len: u32 = 0;
        let ptr = func.call(&mut self.store, (std::ptr::addr_of_mut!(len) as u32,))?;
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
