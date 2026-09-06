//! SDL3 window and rendering.

use std::collections::HashMap;
use std::env;

use ruleste_plugins_api::types::Vec2;
use sdl3::pixels::{Color as SdlColor, PixelFormat};
use sdl3::render::{BlendMode, FRect, Texture, TextureAccess, WindowCanvas};
use sdl3::video::{Window, WindowBuilder};
use sdl3::{EventPump, Sdl};

use crate::data::atlas::{Atlas, FrameRect};
use crate::data::spritebank::SpriteBank;
use crate::engine::autotiler::TileGrid;
use crate::engine::backdrops::Backdrop;
use crate::engine::draw::{Circle, HollowRect, Image, Line, Rect, Text, TileBox};
use crate::engine::ecs::World;
use crate::engine::sprites::SpriteAnimator;
use ruleste_plugins_api::types::Justify;

pub const WINDOW_WIDTH: u32 = 320;
pub const WINDOW_HEIGHT: u32 = 180;
pub const PIXEL_SCALE: u32 = 4;

pub struct Renderer {
    pub sdl: Sdl,
    pub canvas: WindowCanvas,
    _window: Window,
    pub pump: EventPump,
    atlas_textures: HashMap<usize, Texture>,
    /// Per-PixelFont-size texture page cache. Keyed by the size's `size`
    /// field (e.g. 64.0 for renogare64). Stored in a separate map so it
    /// doesn't collide with atlas page indices.
    pub font_pages: HashMap<String, Texture>,
    /// Dedicated per-frame textures for backdrop layers. Backdrops are
    /// alpha/color modulated, so they must not share the atlas page textures
    /// used by tiles and sprites. Stores the frame's offset/untrimmed box so
    /// the tiling loop matches `Parallax.Render`.
    backdrop_textures: HashMap<String, BackdropFrame>,
    camera: Vec2,
    /// The active PixelFont used for plugin-submitted text commands.
    /// Populated via `upload_pixel_font`; when present, `SpriteFont` is
    /// ignored (the legacy `set_font` path).
    pub pixel_font: Option<crate::data::font::PixelFont>,
    /// The active SpriteFont used for plugin-submitted text commands.
    /// Used as a fallback when `pixel_font` is `None`.
    pub font: Option<crate::data::font::SpriteFont>,
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
            font: None,
            font_pages: HashMap::new(),
            pixel_font: None,
        })
    }

    pub fn set_camera(&mut self, camera: Vec2) {
        self.camera = camera;
    }

    /// Resize the logical render target to `(w, h)` with `STRETCH` (no
    /// letterbox padding). The window is also resized to match so that
    /// `read_pixels` returns exactly the logical pixels (no black padding).
    pub fn set_logical_size_raw(&mut self, w: u32, h: u32) -> anyhow::Result<()> {
        self.canvas.set_logical_size(
            w,
            h,
            sdl3::sys::render::SDL_RendererLogicalPresentation::STRETCH,
        )?;
        // Resize window to match logical size so `read_pixels` returns exactly
        // the rendered logical pixels (no extra black padding / scaling mismatch).
        self._window.set_size(w, h)?;
        Ok(())
    }

    /// Returns the current window-to-logical pixel scale. The renderer is set
    /// to a logical 320x180 view (letterbox), so this is the actual window
    /// width divided by `WINDOW_WIDTH`. Mouse coordinates reported in events
    /// are in window pixels and must be divided by this to land in logical
    /// (world) units.
    pub fn pixel_scale(&self) -> f32 {
        let (w, _) = self
            .canvas
            .output_size()
            .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT));
        w as f32 / WINDOW_WIDTH as f32
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
        let (pi, fi) = atlas.resolve_frame(frame_id)
            .ok_or_else(|| anyhow::anyhow!("backdrop frame {frame_id:?} not in atlas"))?;
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
        // Only draw tiles inside the visible 320x180 window (world space),
        // translated into the grid's local tile range via its world origin.
        let view_w = self
            .canvas
            .output_size()
            .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
            .0 as f32;
        let view_h = self
            .canvas
            .output_size()
            .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
            .1 as f32;
        let min_tx = (((self.camera.x - tile_grid.origin_x) / 8.0).floor() as i32) - 1;
        let max_tx = (((self.camera.x + view_w - tile_grid.origin_x) / 8.0).ceil() as i32) + 1;
        let min_ty = (((self.camera.y - tile_grid.origin_y) / 8.0).floor() as i32) - 1;
        let max_ty = (((self.camera.y + view_h - tile_grid.origin_y) / 8.0).ceil() as i32) + 1;
        let min_tx = min_tx.max(0).min(tile_grid.width as i32);
        let max_tx = max_tx.max(0).min(tile_grid.width as i32);
        let min_ty = min_ty.max(0).min(tile_grid.height as i32);
        let max_ty = max_ty.max(0).min(tile_grid.height as i32);
        let mut drawn = 0usize;
        let mut missing = 0usize;
        for ty in min_ty..max_ty {
            for tx in min_tx..max_tx {
                let Some((tileset_path, col, row)) = tile_grid.tile_at(tx as usize, ty as usize)
                else {
                    continue;
                };
                let x = (tx as f32 * 8.0 + tile_grid.origin_x - self.camera.x).round();
                let y = (ty as f32 * 8.0 + tile_grid.origin_y - self.camera.y).round();

                // The XML `path` is relative to "tilesets/", matching the
                // atlas frame id (e.g. "tilesets/dirt").
                let frame_id = format!("tilesets/{tileset_path}");
                // Look up the tileset frame in the atlas.
                if let Some((page_idx, frame_idx)) = atlas.resolve_frame(&frame_id) {
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
                    drawn += 1;
                } else {
                    // Fallback: debug color for missing tilesets.
                    self.canvas.set_draw_color(fallback_color);
                    let _ = self.canvas.fill_rect(FRect::new(x, y, 8.0, 8.0));
                    drawn += 1;
                    missing += 1;
                }
            }
        }
        if env::var_os("RULESTE_TRACE").is_some() {
            eprintln!(
                "RULESTE_TRACE draw_solids: origin=({},{}) camera=({:.1},{:.1}) visible=[{min_tx}..{max_tx},{min_ty}..{max_ty}] grid={}x{} drawn={drawn} missing_atlas={missing}",
                tile_grid.origin_x,
                tile_grid.origin_y,
                self.camera.x,
                self.camera.y,
                tile_grid.width,
                tile_grid.height
            );
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
            let (frame_id, sprite_ref) = if let Some(sprite) = bank.sprite(&entity.sprite.sprite) {
                let Some(anim) = sprite.animation(&entity.sprite.animation) else {
                    continue;
                };
                let Some(frame_id) = animator.current_frame(sprite, anim, entity.sprite.frame)
                else {
                    continue;
                };
                (frame_id, Some(sprite))
            } else {
                let frame_id = entity.sprite.animation.clone();
                if !atlas.has_frame(&frame_id) {
                    continue;
                }
                (frame_id, None)
            };
            let Some((page_idx, frame_idx)) = atlas.resolve_frame(&frame_id) else {
                continue;
            };
            let page = &atlas.pages[page_idx];
            let frame = &page.frames[frame_idx];
            let Some(texture) = self.atlas_textures.get(&page_idx) else {
                continue;
            };

            // Resolve the anchor point within the frame. A sprite may declare an
            // explicit `<Origin>`, be `<Center/>`d (Justify 0.5), or use an
            // explicit `<Justify x y>`; otherwise the origin defaults to (0,0).
            let (mut ox, mut oy) = match sprite_ref {
                Some(sprite) if sprite.center => {
                    (frame.clip.w as f32 * 0.5, frame.clip.h as f32 * 0.5)
                }
                Some(sprite) => match sprite.justify {
                    Some((jx, jy)) => (jx * frame.clip.w as f32, jy * frame.clip.h as f32),
                    None => (sprite.origin.0 as f32, sprite.origin.1 as f32),
                },
                None => (0.0, 0.0),
            };
            ox += frame.offset.x as f32;
            oy += frame.offset.y as f32;

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

    /// Draws wireframe outlines of every alive entity's hitbox in world space,
    /// transformed by the camera. Red = normal, yellow = solid-platform, green =
    /// solid-entity. Only drawn when `show_hitboxes` is `true` (env var
    /// `RULESTE_SHOW_HITBOXES=1`).
    pub fn draw_hitboxes(&mut self, world: &World, show_hitboxes: bool) {
        if !show_hitboxes {
            return;
        }
        for entity in world.iter() {
            if !world.is_alive(entity.id) {
                continue;
            }
            let hx = entity.position.x + entity.hitbox_offset.x;
            let hy = entity.position.y + entity.hitbox_offset.y;
            let hw = entity.hitbox.x;
            let hh = entity.hitbox.y;
            let (r, g, b) = if world.solid_entities.contains(&entity.id) {
                (0, 255, 0)
            } else if world.solid_platforms.contains(&entity.id) {
                (255, 255, 0)
            } else {
                (255, 0, 0)
            };
            let (x, y) = (hx - self.camera.x, hy - self.camera.y);
            self.canvas.set_draw_color(SdlColor::RGBA(r, g, b, 200));
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x, y),
                sdl3::render::FPoint::new(x + hw, y),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x + hw, y),
                sdl3::render::FPoint::new(x + hw, y + hh),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x + hw, y + hh),
                sdl3::render::FPoint::new(x, y + hh),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x, y + hh),
                sdl3::render::FPoint::new(x, y),
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

    /// Draws plugin-submitted hollow rectangles (four line segments at the
    /// edges), transformed by the camera.
    pub fn draw_hollow_rects(&mut self, rects: &[HollowRect]) {
        for rect in rects {
            let (x, y) = (rect.x - self.camera.x, rect.y - self.camera.y);
            let (w, h) = (rect.w, rect.h);
            self.set_draw_color(rect.color);
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x, y),
                sdl3::render::FPoint::new(x + w, y),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x + w, y),
                sdl3::render::FPoint::new(x + w, y + h),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x + w, y + h),
                sdl3::render::FPoint::new(x, y + h),
            );
            let _ = self.canvas.draw_line(
                sdl3::render::FPoint::new(x, y + h),
                sdl3::render::FPoint::new(x, y),
            );
        }
    }

    /// Draws plugin-submitted circles (pixel-perfect outline, Bresenham's
    /// midpoint algorithm), transformed by the camera.
    pub fn draw_circles(&mut self, circles: &[Circle]) {
        for c in circles {
            let r = c.r;
            if r <= 0.0 {
                continue;
            }
            self.set_draw_color(c.color);
            let cx = c.cx - self.camera.x;
            let cy = c.cy - self.camera.y;
            let mut x = r as i32;
            let mut y = 0i32;
            let mut err = 1 - x;
            while x >= y {
                self.draw_circle_point(cx + x as f32, cy + y as f32);
                self.draw_circle_point(cx - x as f32, cy + y as f32);
                self.draw_circle_point(cx + x as f32, cy - y as f32);
                self.draw_circle_point(cx - x as f32, cy - y as f32);
                self.draw_circle_point(cx + y as f32, cy + x as f32);
                self.draw_circle_point(cx - y as f32, cy + x as f32);
                self.draw_circle_point(cx + y as f32, cy - x as f32);
                self.draw_circle_point(cx - y as f32, cy - x as f32);
                y += 1;
                if err < 0 {
                    err += 2 * y + 1;
                } else {
                    x -= 1;
                    err += 2 * (y - x) + 1;
                }
            }
        }
    }

    fn draw_circle_point(&mut self, x: f32, y: f32) {
        let x0 = (x - 0.5).floor();
        let x1 = x0 + 1.0;
        let y0 = (y - 0.5).floor();
        let y1 = y0 + 1.0;
        let _ = self.canvas.draw_line(
            sdl3::render::FPoint::new(x0, y0),
            sdl3::render::FPoint::new(x1, y1),
        );
    }

    fn set_draw_color(&mut self, color: ruleste_plugins_api::types::Color) {
        self.canvas
            .set_draw_color(SdlColor::RGBA(color.r, color.g, color.b, color.a));
    }

    /// Sets the SpriteFont used to render plugin-submitted [`Text`] commands.
    /// Plugins draw text relative to the camera, so the renderer needs the
    /// active font; it's set once at startup from the resources root.
    pub fn set_font(&mut self, font: crate::data::font::SpriteFont) {
        self.font = Some(font);
    }

    /// Uploads every texture page referenced by a `PixelFont` into the
    /// renderer's per-page cache. Page basenames are resolved relative to
    /// `font_dir` (typically the directory containing the `.fnt` descriptor).
    /// Pages whose PNG is missing are skipped silently — characters on those
    /// pages will simply not render.
    pub fn upload_pixel_font(
        &mut self,
        font: crate::data::font::PixelFont,
        font_dir: &std::path::Path,
    ) -> anyhow::Result<()> {
        for size in &font.sizes {
            for page_name in &size.page_textures {
                if self.font_pages.contains_key(page_name) {
                    continue;
                }
                let Some(page) = crate::data::font::load_page_png(font_dir, page_name)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                else {
                    continue;
                };
                let creator = self.canvas.texture_creator();
                let mut tex = creator.create_texture(
                    Some(PixelFormat::ABGR8888),
                    TextureAccess::Static,
                    page.width,
                    page.height,
                )?;
                tex.set_blend_mode(sdl3::render::BlendMode::Blend);
                tex.update(None, &page.rgba, (page.width as usize) * 4)?;
                self.font_pages.insert(page_name.clone(), tex);
            }
        }
        self.pixel_font = Some(font);
        Ok(())
    }

    /// Draws plugin-submitted text using the active `PixelFont` (BMFont
    /// pipeline). Falls back to no-op if no font was uploaded. Multi-line
    /// text uses the per-size `line_height`; justify offsets per-line width.
    pub fn draw_pixel_texts(&mut self, font: &crate::data::font::PixelFont, texts: &[Text]) {
        if font.sizes.is_empty() {
            return;
        }
        for t in texts {
            if t.text.is_empty() {
                continue;
            }
            let size = match font.get(t.text.len() as f32) {
                Some(s) => s,
                None => continue,
            };
            let line_spacing = size.line_height as f32;
            let x0 = t.x - self.camera.x;
            let y0 = t.y - self.camera.y;
            let mut cursor_y = y0;
            let mut line_start = 0usize;
            let chars: Vec<char> = t.text.chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                if c == '\n' {
                    let line: String = chars[line_start..i].iter().collect();
                    let (line_w, _) = size.measure(&line);
                    let sx = match t.justify {
                        Justify::Left => x0,
                        Justify::Center => x0 - line_w as f32 * 0.5,
                        Justify::Right => x0 - line_w as f32,
                    };
                    self.draw_pixel_line(font, size, &line, sx, cursor_y, t.color, t.outline);
                    cursor_y += line_spacing;
                    line_start = i + 1;
                }
            }
            let line: String = chars[line_start..].iter().collect();
            let (line_w, _) = size.measure(&line);
            let sx = match t.justify {
                Justify::Left => x0,
                Justify::Center => x0 - line_w as f32 * 0.5,
                Justify::Right => x0 - line_w as f32,
            };
            self.draw_pixel_line(font, size, &line, sx, cursor_y, t.color, t.outline);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pixel_line(
        &mut self,
        _font: &crate::data::font::PixelFont,
        size: &crate::data::font::PixelFontSize,
        text: &str,
        x: f32,
        y: f32,
        color: ruleste_plugins_api::types::Color,
        outline: Option<ruleste_plugins_api::types::Color>,
    ) {
        let page_key = match size.page_textures.first() {
            Some(k) => k,
            None => return,
        };
        let Some(texture) = self.font_pages.get(page_key) else {
            return;
        };
        let mut cursor_x = x;
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c == '\n' {
                break;
            }
            let Some(glyph) = size.characters.get(&c) else {
                continue;
            };
            let dst_x = cursor_x + glyph.x_offset as f32;
            let dst_y = y + glyph.y_offset as f32;
            let w = glyph.region.width as f32;
            let h = glyph.region.height as f32;
            if let Some(oc) = outline {
                self.canvas
                    .set_draw_color(SdlColor::RGBA(oc.r, oc.g, oc.b, oc.a));
                let _ =
                    self.canvas
                        .fill_rect(FRect::new(dst_x - 1.0, dst_y - 1.0, w + 2.0, h + 2.0));
            }
            let src = FRect::new(glyph.region.x as f32, glyph.region.y as f32, w, h);
            let dst = FRect::new(dst_x, dst_y, w, h);
            self.canvas
                .set_draw_color(SdlColor::RGBA(color.r, color.g, color.b, color.a));
            let _ = self.canvas.copy(texture, src, dst);
            cursor_x += glyph.x_advance as f32;
            if i + 1 < chars.len() {
                if let Some(&k) = glyph.kerning.get(&chars[i + 1]) {
                    cursor_x += k as f32;
                }
            }
        }
    }

    /// Draws plugin-submitted text (`Draw.Text` / `TextJustified` /
    /// `TextCentered` / `OutlineText`). Position is in world coordinates,
    /// transformed by the camera; multi-line text is rendered line-by-line.
    /// Prefers the active `PixelFont`; falls back to the legacy `SpriteFont`
    /// when no BMFont is loaded.
    pub fn draw_texts(&mut self, texts: &[Text]) {
        if let Some(pf) = self.pixel_font.clone() {
            self.draw_pixel_texts(&pf, texts);
            return;
        }
        let Some(font) = self.font.clone() else {
            return;
        };
        let line_spacing = font.line_spacing as f32;
        for t in texts {
            if t.text.is_empty() {
                continue;
            }
            let x0 = t.x - self.camera.x;
            let y0 = t.y - self.camera.y;
            let mut cursor_y = y0;
            let mut line_start = 0usize;
            for (i, c) in t.text.char_indices() {
                if c == '\n' {
                    let line = &t.text[line_start..i];
                    let line_w = font.measure_string(line) as f32;
                    let sx = match t.justify {
                        Justify::Left => x0,
                        Justify::Center => x0 - line_w * 0.5,
                        Justify::Right => x0 - line_w,
                    };
                    self.draw_text_line(&font, line, sx, cursor_y, t.color, t.outline);
                    cursor_y += line_spacing;
                    line_start = i + c.len_utf8();
                }
            }
            let line = &t.text[line_start..];
            let line_w = font.measure_string(line) as f32;
            let sx = match t.justify {
                Justify::Left => x0,
                Justify::Center => x0 - line_w * 0.5,
                Justify::Right => x0 - line_w,
            };
            self.draw_text_line(&font, line, sx, cursor_y, t.color, t.outline);
        }
    }

    fn draw_text_line(
        &mut self,
        font: &crate::data::font::SpriteFont,
        text: &str,
        x: f32,
        y: f32,
        color: ruleste_plugins_api::types::Color,
        outline: Option<ruleste_plugins_api::types::Color>,
    ) {
        let mut cursor_x = x;
        let mut prev_char: Option<char> = None;
        for c in text.chars() {
            if let Some(glyph) = font.glyph(c) {
                let ox = if font.use_kerning && prev_char.is_some() {
                    font.spacing as f32
                } else {
                    0.0
                };
                cursor_x += ox;
                let bounds = glyph.bounds;
                let dst = FRect::new(
                    cursor_x + bounds.x as f32,
                    y + bounds.y as f32,
                    bounds.width as f32,
                    bounds.height as f32,
                );
                if let Some(oc) = outline {
                    // Draw the outline as a filled rect offset by ±1px.
                    let orect = FRect::new(dst.x - 1.0, dst.y - 1.0, dst.w + 2.0, dst.h + 2.0);
                    self.canvas
                        .set_draw_color(SdlColor::RGBA(oc.r, oc.g, oc.b, oc.a));
                    let _ = self.canvas.fill_rect(orect);
                }
                self.canvas
                    .set_draw_color(SdlColor::RGBA(color.r, color.g, color.b, color.a));
                let _ = self.canvas.fill_rect(dst);
                cursor_x += glyph.advance as f32;
            }
            prev_char = Some(c);
        }
    }

    /// Draws plugin-submitted autotiled boxes (`TileBox`), e.g. introCrusher
    pub fn draw_tile_boxes(&mut self, boxes: &[TileBox], atlas: &Atlas) {
        for tile_box in boxes {
            let Some((page_idx, frame_idx)) = atlas.resolve_frame(&tile_box.frame_id) else {
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
            let Some((page_idx, frame_idx)) = atlas.resolve_frame(&image.frame_id) else {
                continue;
            };
            let page = &atlas.pages[page_idx];
            let frame = &page.frames[frame_idx];
            let Some(texture) = self.atlas_textures.get(&page_idx) else {
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

/// Text rendering utilities.
impl Renderer {
    /// Draw text at the specified position with the given font and color.
    /// Text is drawn left-aligned; for centered or right-aligned text, use
    /// `measure_text` to compute the offset.
    ///
    /// For now, this uses a simple glyph-by-glyph rendering approach.
    /// A production implementation would batch glyphs into a single draw call.
    pub fn draw_text(
        &mut self,
        font: &crate::data::font::SpriteFont,
        text: &str,
        x: f32,
        y: f32,
        color: (u8, u8, u8, u8),
    ) {
        let mut cursor_x = x;
        let mut prev_char: Option<char> = None;

        for c in text.chars() {
            if let Some(glyph) = font.glyph(c) {
                // Apply kerning if enabled
                if font.use_kerning && prev_char.is_some() {
                    cursor_x += font.spacing as f32;
                }

                // Draw the glyph (placeholder: draw a rectangle for now)
                // A real implementation would render from a texture atlas
                let bounds = glyph.bounds;
                let dst = FRect::new(
                    cursor_x + bounds.x as f32,
                    y + bounds.y as f32,
                    bounds.width as f32,
                    bounds.height as f32,
                );

                self.canvas
                    .set_draw_color(SdlColor::RGBA(color.0, color.1, color.2, color.3));
                if let Err(e) = self.canvas.fill_rect(dst) {
                    eprintln!("Failed to draw glyph '{c}': {e}");
                }

                cursor_x += glyph.advance as f32;
            }
            prev_char = Some(c);
        }
    }

    /// Draw centered text at the specified position.
    pub fn draw_text_centered(
        &mut self,
        font: &crate::data::font::SpriteFont,
        text: &str,
        center_x: f32,
        y: f32,
        color: (u8, u8, u8, u8),
    ) {
        let width = font.measure_string(text) as f32;
        let x = center_x - width / 2.0;
        self.draw_text(font, text, x, y, color);
    }

    /// Draw right-aligned text.
    pub fn draw_text_right(
        &mut self,
        font: &crate::data::font::SpriteFont,
        text: &str,
        right_x: f32,
        y: f32,
        color: (u8, u8, u8, u8),
    ) {
        let width = font.measure_string(text) as f32;
        let x = right_x - width;
        self.draw_text(font, text, x, y, color);
    }

    /// Measure text width.
    pub fn measure_text(font: &crate::data::font::SpriteFont, text: &str) -> f32 {
        font.measure_string(text) as f32
    }

    /// Draw text with word wrapping.
    ///
    /// Returns the total height of the rendered text.
    pub fn draw_text_wrapped(
        &mut self,
        font: &crate::data::font::SpriteFont,
        text: &str,
        x: f32,
        mut y: f32,
        max_width: f32,
        color: (u8, u8, u8, u8),
    ) -> f32 {
        let line_height = font.line_spacing as f32;
        let mut current_width = 0.0;
        let mut word_start = 0;
        let mut current_line = String::new();

        for (i, c) in text.chars().chain(Some(' ')).enumerate() {
            if c == ' ' || c == '\n' {
                // Word boundary
                let word = &text[word_start..i];
                let word_width = Renderer::measure_text(font, word);

                if current_width == 0.0 || current_width + word_width <= max_width {
                    // Word fits on current line
                    if !current_line.is_empty() {
                        current_line.push(' ');
                        current_width += font.spacing as f32;
                    }
                    current_line.push_str(word);
                    current_width += word_width;
                } else {
                    // Need to wrap
                    self.draw_text(font, &current_line, x, y, color);
                    y += line_height;
                    current_line.clear();
                    current_line.push_str(word);
                    current_width = word_width;
                }

                word_start = i + 1;

                if c == '\n' {
                    // Explicit newline
                    if !current_line.is_empty() {
                        self.draw_text(font, &current_line, x, y, color);
                        y += line_height;
                        current_line.clear();
                        current_width = 0.0;
                    }
                }
            }
        }

        // Draw the last line
        if !current_line.is_empty() {
            self.draw_text(font, &current_line, x, y, color);
            y += line_height;
        }

        y - (y - line_height) // Return total height
    }
}
