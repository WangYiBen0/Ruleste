//! A lightweight particle system owned by the host. Plugins spawn particles
//! through the `host_emit_particle` FFI (`ruleste_plugins_api::host::emit_particle`),
//! the host integrates them each frame and blits them as filled rectangles
//! during the draw pass. This replaces the original Celeste `ParticleSystem` /
//! `ParticleType` classes, which lived in the Monocle engine; here they are
//! host infrastructure so every Wasm plugin can share one simulation without
//! reimplementing it.

use ruleste_plugins_api::types::Color;

use crate::engine::draw;

/// A single point particle drawn as a fading square. All fields are in world
/// coordinates / seconds; the host advances `life` by `dt` each frame and
/// removes the particle once `life <= 0`.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub ax: f32,
    pub ay: f32,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
    /// Half-extent of the square in pixels; the particle is drawn `2*size` wide
    /// and tall, centered on `(x, y)`.
    pub size: f32,
}

impl Particle {
    /// Builds a particle. `alpha` is the starting opacity (0–255); it fades
    /// linearly to 0 as `life` counts down to 0.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        ax: f32,
        ay: f32,
        life: f32,
        color: Color,
        size: f32,
    ) -> Particle {
        Particle {
            x,
            y,
            vx,
            vy,
            ax,
            ay,
            life,
            max_life: life,
            color,
            size,
        }
    }

    /// Advances the particle by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.vx += self.ax * dt;
        self.vy += self.ay * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.life -= dt;
    }

    /// Current opacity (0–255) based on remaining life.
    #[must_use]
    pub fn alpha(&self) -> u8 {
        if self.max_life <= 0.0 {
            return self.color.a;
        }
        let f = (self.life / self.max_life).clamp(0.0, 1.0);
        (f * self.color.a as f32).round() as u8
    }
}

/// The host's particle pool. Cheap to clear; particles are integrated in
/// [`ParticleSystem::update`] and baked into the frame's rectangle list by
/// [`ParticleSystem::append_to_rects`].
#[derive(Clone, Debug, Default)]
pub struct ParticleSystem {
    particles: Vec<Particle>,
}

impl ParticleSystem {
    #[must_use]
    pub fn new() -> ParticleSystem {
        ParticleSystem::default()
    }

    /// Spawns a particle (no-op if `life <= 0`).
    pub fn emit(&mut self, p: Particle) {
        if p.life > 0.0 {
            self.particles.push(p);
        }
    }

    /// Removes all particles (e.g. on room respawn).
    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// Integrates every live particle and drops the expired ones.
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.update(dt);
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    /// Appends one filled, alpha-faded rectangle per live particle into `rects`,
    /// so the existing rectangle renderer draws them. Call after the plugin draw
    /// hooks have populated `rects`.
    pub fn append_to_rects(&self, rects: &mut Vec<draw::Rect>) {
        for p in &self.particles {
            let a = p.alpha();
            if a == 0 {
                continue;
            }
            let s = p.size.max(0.0);
            rects.push(draw::Rect {
                x: p.x - s,
                y: p.y - s,
                w: s * 2.0,
                h: s * 2.0,
                color: Color {
                    r: p.color.r,
                    g: p.color.g,
                    b: p.color.b,
                    a,
                },
            });
        }
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.particles.len()
    }
}
