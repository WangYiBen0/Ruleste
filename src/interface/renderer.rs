//! SDL3 window and rendering.

use std::collections::HashMap;

use ruleste_plugin_api::types::Vec2;
use sdl3::pixels::{Color as SdlColor, PixelFormat};
use sdl3::render::{BlendMode, FRect, Texture, TextureAccess, WindowCanvas};
use sdl3::video::{Window, WindowBuilder};
use sdl3::{EventPump, Sdl};

use crate::data::atlas::{Atlas, FrameRect};
use crate::data::spritebank::SpriteBank;
use crate::engine::autotiler::TileGrid;
use crate::engine::backdrops::Backdrop;
use crate::engine::draw::{Image, Line, Rect, TileBox};
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
    /// Dedicated per-frame textures for backdrop layers. Backdrops are
    /// alpha/color modulated, so they must not share the atlas page textures
    /// used by tiles and sprites. Stores the frame's offset/untrimmed box so
    /// the tiling loop matches `Parallax.Render`.
    backdrop_textures: HashMap<String, BackdropFrame>,
    camera: Vec2,
}

/// A backdrop texture plus the atlas frame's offset rect (untrimmed box).
struct BackdropFrame {
    texture: Texture,
    offset: FrameRect,
}

impl Renderer {
    /// `for_frame_dump` forces the software renderer: `SDL_RenderReadPixels`
    /// (used by `RULESTE_DUMP_FRAME`) only returns valid data on the software
    /// path. Normal gameplay uses the GPU (SDL's default pick: opengl/vulkan/
    /// direct3d) so maximizing the window scales on the GPU instead of the CPU.
    pub fn new(for_frame_dump: bool) -> anyhow::Result<Renderer> {
        // Software renderer only when a frame dump is requested, so that
        // `SDL_RenderReadPixels` works. Otherwise let SDL pick a hardware
        // driver (GPU scaling is much cheaper than the CPU when maximized).
        // The hint is Normal priority, so an explicit `SDL_RENDER_DRIVER`
        // environment variable (Override priority) still wins.
        if for_frame_dump {
            sdl3::hint::set_with_priority(
                "SDL_RENDER_DRIVER",
                "software",
                &sdl3::hint::Hint::Normal,
            );
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
        // SDL3 defaults the presentation (and texture) scale mode to LINEAR,
        // which makes the 320x180 framebuffer look soft when upscaled. Force
        // nearest-neighbor so pixels stay crisp at any window size.
        unsafe {
            sdl3::sys::render::SDL_SetDefaultTextureScaleMode(
                canvas.raw(),
                sdl3::sys::surface::SDL_ScaleMode::NEAREST,
            );
        }
        Ok(Renderer {
            sdl,
            canvas,
            _window: window,
            pump,
            atlas_textures: HashMap::new(),
            backdrop_textures: HashMap::new(),
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

    /// Crops a backdrop frame out of the atlas into its own texture, so it can
    /// be tinted and alpha-blended without disturbing tiles/sprites.
    pub fn upload_backdrop(&mut self, atlas: &Atlas, frame_id: &str) -> anyhow::Result<()> {
        if self.backdrop_textures.contains_key(frame_id) {
            return Ok(());
        }
        let Some(rgba) = atlas.frame_rgba_into(frame_id) else {
            anyhow::bail!("backdrop frame {frame_id:?} not in atlas");
        };
        let (pi, fi) = atlas.frame_index[frame_id];
        let frame = &atlas.pages[pi].frames[fi];
        let w = frame.clip.w as u32;
        let h = frame.clip.h as u32;
        let creator = self.canvas.texture_creator();
        let mut texture =
            creator.create_texture(Some(PixelFormat::ABGR8888), TextureAccess::Static, w, h)?;
        texture.set_blend_mode(BlendMode::Blend);
        texture.update(None, &rgba, w as usize * 4)?;
        self.backdrop_textures.insert(
            frame_id.to_string(),
            BackdropFrame {
                texture,
                offset: frame.offset,
            },
        );
        Ok(())
    }

    /// Draws a list of parallax backdrops in screen space, mirroring
    /// `Parallax.Render`. The camera's world position selects the tile of the
    /// texture to show; the draw loop covers the whole 320x180 view.
    pub fn draw_backdrops(&mut self, backdrops: &[Backdrop]) {
        for b in backdrops {
            let Some(bf) = self.backdrop_textures.get_mut(&b.texture) else {
                eprintln!("ruleste: backdrop texture {:?} not uploaded", b.texture);
                continue;
            };
            let texture = &mut bf.texture;
            // The loop steps by the frame's untrimmed box (offset.w/h); the
            // visible clip (our cropped texture) is drawn at `position - offset`.
            let tw = bf.offset.w as f32;
            let th = bf.offset.h as f32;
            let ox = bf.offset.x as f32;
            let oy = bf.offset.y as f32;
            let cw = texture.width() as f32;
            let ch = texture.height() as f32;

            // `vector = (camera.position + camera_offset).floor()` (Parallax's
            // own CameraOffset is zero); anchor the backdrop by scroll factor.
            let cam = Vec2::new(self.camera.x.floor(), self.camera.y.floor());
            let mut sx = (b.position.x - cam.x * b.scroll.x).floor();
            let mut sy = (b.position.y - cam.y * b.scroll.y).floor();

            // LoopX/LoopY: wrap the start tile into the untrimmed box dims.
            if b.loop_x {
                while sx < 0.0 {
                    sx += tw;
                }
                while sx > 0.0 {
                    sx -= tw;
                }
            }
            if b.loop_y {
                while sy < 0.0 {
                    sy += th;
                }
                while sy > 0.0 {
                    sy -= th;
                }
            }

            // `Color *= alpha` scales every channel; skip fully-transparent.
            if b.alpha <= 0.0 {
                continue;
            }
            let mul = b.alpha;
            let cr = (b.color.0 as f32 * mul).round() as u8;
            let cg = (b.color.1 as f32 * mul).round() as u8;
            let cb = (b.color.2 as f32 * mul).round() as u8;
            let ca = (255.0 * mul).round() as u8;
            if ca <= 1 {
                continue;
            }

            if b.additive {
                // SDL's Add blend is `dst += srcRGB * srcAlpha`, so the alpha
                // must live in the color mod (as in XNA's One/One additive);
                // applying it to alpha_mod as well would square it (4x faint).
                texture.set_blend_mode(BlendMode::Add);
                texture.set_color_mod(cr, cg, cb);
                texture.set_alpha_mod(255);
            } else {
                texture.set_color_mod(cr, cg, cb);
                texture.set_alpha_mod(ca);
            }
            let mut x = sx;
            loop {
                let mut y = sy;
                loop {
                    let dst = FRect::new(x - ox, y - oy, cw, ch);
                    let _ = self
                        .canvas
                        .copy_ex(texture, None, dst, 0.0, None, b.flip_x, b.flip_y);
                    if !b.loop_y {
                        break;
                    }
                    y += th;
                    if y >= WINDOW_HEIGHT as f32 {
                        break;
                    }
                }
                if !b.loop_x {
                    break;
                }
                x += tw;
                if x >= WINDOW_WIDTH as f32 {
                    break;
                }
            }
            if b.additive {
                texture.set_blend_mode(BlendMode::Blend);
            }
            texture.set_color_mod(255, 255, 255);
            texture.set_alpha_mod(255);
        }
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
                let x = (tx as f32 * 8.0 - self.camera.x).round();
                let y = (ty as f32 * 8.0 - self.camera.y).round();

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

    /// Draws plugin-submitted line geometry (world coordinates), transformed by
    /// the camera. Used for procedural scenery like hanging wires.
    pub fn draw_lines(&mut self, lines: &[Line]) {
        for line in lines {
            let (x1, y1) = (line.x1 - self.camera.x, line.y1 - self.camera.y);
            let (x2, y2) = (line.x2 - self.camera.x, line.y2 - self.camera.y);
            self.canvas.set_draw_color(SdlColor::RGBA(
                line.color.r,
                line.color.g,
                line.color.b,
                line.color.a,
            ));
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x1, y1),
                sdl3::render::FPoint::new(x2, y2),
            );
        }
    }

    /// Draws plugin-submitted filled rectangles (world coordinates),
    /// transformed by the camera.
    pub fn draw_rects(&mut self, rects: &[Rect]) {
        for rect in rects {
            self.canvas.set_draw_color(SdlColor::RGBA(
                rect.color.r,
                rect.color.g,
                rect.color.b,
                rect.color.a,
            ));
            let _ = self.canvas.fill_rect(FRect::new(
                rect.x - self.camera.x,
                rect.y - self.camera.y,
                rect.w,
                rect.h,
            ));
        }
    }

    /// Draws plugin-submitted autotiled boxes (`TileBox`), e.g. introCrusher
    /// slabs. Blits each 8x8 cell from the tileset frame, honoring the box's
    /// world position.
    pub fn draw_tile_boxes(&mut self, boxes: &[TileBox], atlas: &Atlas) {
        for tile_box in boxes {
            let Some(&(page_idx, frame_idx)) = atlas.frame_index.get(&tile_box.frame_id) else {
                continue;
            };
            let page = &atlas.pages[page_idx];
            let frame = &page.frames[frame_idx];
            let Some(texture) = self.atlas_textures.get(&page_idx) else {
                continue;
            };
            for ty in 0..tile_box.height {
                for tx in 0..tile_box.width {
                    let idx = ty * tile_box.width + tx;
                    let col = tile_box.col[idx];
                    let row = tile_box.row[idx];
                    let x = (tile_box.x + tx as f32 * 8.0 - self.camera.x).round();
                    let y = (tile_box.y + ty as f32 * 8.0 - self.camera.y).round();
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
                }
            }
        }
    }

    /// Draws plugin-submitted atlas-frame blits. `(x, y)` is the center of the
    /// frame's untrimmed box in world coordinates; the frame's offset is
    /// honored like entity sprites.
    pub fn draw_images(&mut self, images: &[Image], atlas: &Atlas) {
        for image in images {
            let Some((page_idx, frame_idx)) = atlas.frame_index.get(&image.frame_id) else {
                continue;
            };
            let page = &atlas.pages[*page_idx];
            let frame = &page.frames[*frame_idx];
            let Some(texture) = self.atlas_textures.get(page_idx) else {
                continue;
            };
            let w = frame.offset.w as f32;
            let h = frame.offset.h as f32;
            let cx = image.x - self.camera.x;
            let cy = image.y - self.camera.y;
            // Top-left of the full frame, then the trimmed clip inside it.
            let dst = FRect::new(
                cx - w * 0.5 * image.scale_x + frame.offset.x as f32,
                cy - h * 0.5 * image.scale_y + frame.offset.y as f32,
                frame.clip.w as f32 * image.scale_x,
                frame.clip.h as f32 * image.scale_y,
            );
            let src = FRect::new(
                frame.clip.x as f32,
                frame.clip.y as f32,
                frame.clip.w as f32,
                frame.clip.h as f32,
            );
            let color = image.color;
            self.canvas
                .set_draw_color(SdlColor::RGBA(color.r, color.g, color.b, color.a));
            let res = self.canvas.copy_ex(
                texture,
                src,
                dst,
                f64::from(image.rotation.to_radians()),
                None,
                image.flip_x,
                image.flip_y,
            );
            if let Err(e) = res {
                eprintln!("ruleste: copy_ex failed for {:?}: {e}", image.frame_id);
            }
        }
    }

    /// Clears the frame to opaque black. Must set the color explicitly:
    /// `SDL_RenderClear` uses the current draw color, which otherwise leaks
    /// from the last sprite/line drawn in the previous frame.
    pub fn clear(&mut self) {
        self.canvas.set_draw_color(SdlColor::RGB(0, 0, 0));
        self.canvas.clear();
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
