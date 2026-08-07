//! SDL3 window and rendering.

use std::collections::HashMap;

use ruleste_plugin_api::types::Vec2;
use sdl3::pixels::{Color as SdlColor, PixelFormat};
use sdl3::render::{FRect, Texture, TextureAccess, WindowCanvas};
use sdl3::video::{Window, WindowBuilder};
use sdl3::{EventPump, Sdl};

use crate::data::atlas::Atlas;
use crate::data::spritebank::SpriteBank;
use crate::engine::autotiler::TileGrid;
use crate::engine::ecs::World;
use crate::engine::sprites::SpriteAnimator;

pub const WINDOW_WIDTH: u32 = 320;
pub const WINDOW_HEIGHT: u32 = 180;
pub const PIXEL_SCALE: u32 = 4;

pub struct Renderer {
    pub sdl: Sdl,
    pub canvas: WindowCanvas,
    _window: Window,
    pub pump: EventPump,
    atlas_textures: HashMap<usize, Texture>,
    camera: Vec2,
}

impl Renderer {
    pub fn new() -> anyhow::Result<Renderer> {
        // Software renderer by default: it is verified to display correctly on
        // X11 and Wayland (including Niri) with this SDL3 build, and avoids
        // GPU-driver quirks. GPU drivers (e.g. vulkan) also work, but note
        // that `SDL_RenderReadPixels` only returns valid data on the software
        // path. Users can override with SDL_RENDER_DRIVER.
        if std::env::var("SDL_RENDER_DRIVER").is_err() {
            std::env::set_var("SDL_RENDER_DRIVER", "software");
        }
        let sdl = sdl3::init()?;
        let video = sdl.video()?;
        println!("SDL3 video driver: {}", video.current_video_driver());
        let mut builder = WindowBuilder::new(
            &video,
            "Ruleste",
            WINDOW_WIDTH * PIXEL_SCALE,
            WINDOW_HEIGHT * PIXEL_SCALE,
        );
        let window = builder.resizable().build()?;
        let mut canvas = sdl3::render::create_renderer(window.clone(), None)?;
        let pump = sdl.event_pump()?;
        // Render at the fixed internal resolution and let SDL scale it to the
        // window. Resizable windows tile in Niri (fixed-size ones auto-float).
        canvas.set_logical_size(
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            sdl3::sys::render::SDL_RendererLogicalPresentation::LETTERBOX,
        )?;
        Ok(Renderer {
            sdl,
            canvas,
            _window: window,
            pump,
            atlas_textures: HashMap::new(),
            camera: Vec2::ZERO,
        })
    }

    pub fn set_camera(&mut self, camera: Vec2) {
        self.camera = camera;
    }

    /// Uploads every page of an atlas into a GPU texture.
    pub fn upload_atlas(&mut self, atlas: &Atlas) -> anyhow::Result<()> {
        let creator = self.canvas.texture_creator();
        for (i, page) in atlas.pages.iter().enumerate() {
            let mut texture = creator.create_texture(
                Some(PixelFormat::ABGR8888),
                TextureAccess::Static,
                page.width,
                page.height,
            )?;
            texture.set_blend_mode(sdl3::render::BlendMode::Blend);
            texture.update(None, &page.rgba, (page.width as usize) * 4)?;
            self.atlas_textures.insert(i, texture);
        }
        Ok(())
    }

    /// Draws the solid grid using autotiled textures from the atlas.
    /// Falls back to debug-colored rectangles if the tileset frame is not found.
    pub fn draw_solids(&mut self, tile_grid: &TileGrid, atlas: &Atlas) {
        let fallback_color = SdlColor::RGB(64, 64, 80);
        for ty in 0..tile_grid.height {
            for tx in 0..tile_grid.width {
                let Some((tileset_path, col, row)) = tile_grid.tile_at(tx, ty) else {
                    continue;
                };
                let x = tx as f32 * 8.0 - self.camera.x;
                let y = ty as f32 * 8.0 - self.camera.y;

                // The XML `path` is relative to "tilesets/", matching the
                // atlas frame id (e.g. "tilesets/dirt").
                let frame_id = format!("tilesets/{tileset_path}");
                // Look up the tileset frame in the atlas.
                if let Some(&(page_idx, frame_idx)) = atlas.frame_index.get(&frame_id) {
                    let page = &atlas.pages[page_idx];
                    let frame = &page.frames[frame_idx];
                    let Some(texture) = self.atlas_textures.get(&page_idx) else {
                        continue;
                    };
                    // The tileset frame contains the full tileset texture.
                    // Each cell is 8x8, arranged in a grid.
                    let src = FRect::new(
                        frame.clip.x as f32 + col as f32 * 8.0,
                        frame.clip.y as f32 + row as f32 * 8.0,
                        8.0,
                        8.0,
                    );
                    let dst = FRect::new(x, y, 8.0, 8.0);
                    let _ = self
                        .canvas
                        .copy_ex(texture, src, dst, 0.0, None, false, false);
                } else {
                    // Fallback: debug color for missing tilesets.
                    self.canvas.set_draw_color(fallback_color);
                    let _ = self.canvas.fill_rect(FRect::new(x, y, 8.0, 8.0));
                }
            }
        }
    }

    /// Draws every entity with an active sprite animation.
    pub fn draw_entities(
        &mut self,
        world: &World,
        atlas: &Atlas,
        bank: &SpriteBank,
        animator: &mut SpriteAnimator<'_>,
    ) {
        let mut ordered: Vec<_> = world.iter().collect();
        ordered.sort_by_key(|e| e.depth);

        for entity in ordered {
            if !entity.visible || entity.sprite.animation.is_empty() {
                continue;
            }
            // Resolve the atlas frame: prefer SpriteBank animations; fall back
            // to treating the animation name as a direct atlas frame id (used
            // by scenery/decals like "scenery/lamp" that are not in Sprites.xml).
            let (frame_id, sprite_origin) = if let Some(sprite) = bank.sprite(&entity.sprite.sprite)
            {
                let Some(anim) = sprite.animation(&entity.sprite.animation) else {
                    continue;
                };
                let Some(frame_id) = animator.current_frame(sprite, anim, entity.sprite.frame)
                else {
                    continue;
                };
                (frame_id, (sprite.origin.0 as f32, sprite.origin.1 as f32))
            } else {
                let frame_id = entity.sprite.animation.clone();
                if !atlas.frame_index.contains_key(&frame_id) {
                    continue;
                }
                (frame_id, (0.0, 0.0))
            };
            let Some((page_idx, frame_idx)) = atlas.frame_index.get(&frame_id) else {
                continue;
            };
            let page = &atlas.pages[*page_idx];
            let frame = &page.frames[*frame_idx];
            let Some(texture) = self.atlas_textures.get(page_idx) else {
                continue;
            };

            let ox = sprite_origin.0 + frame.offset.x as f32;
            let oy = sprite_origin.1 + frame.offset.y as f32;

            // Anchor: entity position is the bottom-center of its hitbox.
            let anchor_x = entity.position.x + entity.hitbox_offset.x + entity.hitbox.x * 0.5;
            let anchor_y = entity.position.y + entity.hitbox_offset.y + entity.hitbox.y;

            let dst_x = anchor_x - ox - self.camera.x;
            let dst_y = anchor_y - oy - self.camera.y;

            let src = FRect::new(
                frame.clip.x as f32,
                frame.clip.y as f32,
                frame.clip.w as f32,
                frame.clip.h as f32,
            );
            let dst = FRect::new(dst_x, dst_y, frame.clip.w as f32, frame.clip.h as f32);
            let color = entity.sprite.color;
            self.canvas
                .set_draw_color(SdlColor::RGBA(color.r, color.g, color.b, color.a));
            // copy_ex tints via the texture's color modulation; for now draw
            // the raw frame.
            let _ = self.canvas.copy_ex(
                texture,
                src,
                dst,
                0.0,
                None,
                entity.sprite.flip_x,
                entity.sprite.flip_y,
            );
        }
    }

    pub fn present(&mut self) {
        let ok = self.canvas.present();
        if !ok {
            let msg: String = sdl3::get_error().to_string();
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 5 {
                eprintln!("ruleste: SDL_RenderPresent failed ({n}): {msg}");
            }
        }
    }
}
