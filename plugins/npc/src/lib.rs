#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `npc` entity plugin.
//!
//! Mirrors `NPC.cs` (granny, snowman, ...): a non-interactive character that
//! stands on a floor marker. Dialogue exists only as far as a spoken quote
//! needs the host's text system (not implemented), so the plugin draws a real
//! atlas sprite for the dispatched `npc` id and adds a tiny idle bob.

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("npc");
ruleste_plugin_api::ruleste_entity_types!("npc");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Per-NPC sprite entry. `(prefix, frame_count)` — frame id is `<prefix>{idx:02}`.
struct NpcSprite {
    prefix: &'static str,
    count: usize,
}

const DEFAULT_SPRITE: NpcSprite = NpcSprite {
    prefix: "characters/oldlady/ha",
    count: 10,
};

fn sprite_for(npc: &str) -> NpcSprite {
    match npc {
        "granny" | "granny_a" => NpcSprite {
            prefix: "characters/oldlady/ha",
            count: 10,
        },
        "oshiro" | "oshiro_big" => NpcSprite {
            prefix: "characters/oshiro/oshiro",
            count: 70,
        },
        "mroizo" => NpcSprite {
            prefix: "characters/oshiro/boss",
            count: 30,
        },
        "theo" => NpcSprite {
            prefix: "characters/theo/alert",
            count: 9,
        },
        "theo_crystal" => NpcSprite {
            prefix: "characters/theoCrystal/idle",
            count: 1,
        },
        "badeline" => NpcSprite {
            prefix: "characters/player_badeline/idle",
            count: 8,
        },
        "snwman" | "snowman" => NpcSprite {
            prefix: "characters/snowman/snowman",
            count: 1,
        },
        _ => DEFAULT_SPRITE,
    }
}

#[derive(Clone, Copy)]
struct NpcState {
    timer: f32,
    sprite: NpcSprite,
}

impl Default for NpcState {
    fn default() -> Self {
        NpcState {
            timer: 0.0,
            sprite: DEFAULT_SPRITE,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<NpcState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut NpcState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, NpcState::default) as *mut NpcState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(1000);
    let npc_id = spawn.get_str("npc", "");
    let sprite = sprite_for(&npc_id);
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        s.insert(id, NpcState { timer: 0.0, sprite });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let bob = (st.timer * 2.0).sin().abs() * 1.0;
        let idx = (st.timer * 4.0) as usize % st.sprite.count.max(1);
        let frame = format!("{}{:02}", st.sprite.prefix, idx);
        draw_image(&frame, p.x, p.y + bob, 0.0, 1.0, 1.0);
    });
}
