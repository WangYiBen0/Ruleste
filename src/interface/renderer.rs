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
        let sdl = sdl3::init()?;
        let video = sdl.video()?;
        println!("SDL3 video driver: {}", video.current_video_driver());
        let builder = WindowBuilder::new(
            &video,
            "Ruleste",
            WINDOW_WIDTH * PIXEL_SCALE,
            WINDOW_HEIGHT * PIXEL_SCALE,
        );
        let window = builder.build()?;
        let mut canvas = sdl3::render::create_renderer(window.clone(), None)?;
        let pump = sdl.event_pump()?;
        canvas.set_scale(PIXEL_SCALE as f32, PIXEL_SCALE as f32)?;
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
                Some(PixelFormat::RGBA8888),
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

                // Look up the tileset frame in the atlas.
                if let Some(&(page_idx, frame_idx)) = atlas.frame_index.get(tileset_path) {
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
                    let _ = self.canvas.copy_ex(texture, src, dst, 0.0, None, false, false);
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
            let Some(sprite) = bank.sprite(&entity.sprite.sprite) else {
                continue;
            };
            let Some(anim) = sprite.animation(&entity.sprite.animation) else {
                continue;
            };
            let Some(frame_id) = animator.current_frame(sprite, anim, entity.sprite.frame) else {
                continue;
            };
            let Some((page_idx, frame_idx)) = atlas.frame_index.get(&frame_id) else {
                continue;
            };
            let page = &atlas.pages[*page_idx];
            let frame = &page.frames[*frame_idx];
            let Some(texture) = self.atlas_textures.get(page_idx) else {
                continue;
            };

            let ox = sprite.origin.0 as f32 + frame.offset.x as f32;
            let oy = sprite.origin.1 as f32 + frame.offset.y as f32;

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
        self.canvas.present();
    }
}
