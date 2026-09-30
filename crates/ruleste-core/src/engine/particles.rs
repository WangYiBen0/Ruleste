//! The particle system, mirroring Monocle's `ParticleType` / `Particle` /
//! `ParticleSystem`.
//!
//! The original keeps two layers in a level: `level.Particles` behind the
//! entities and `level.ParticlesFG` in front of them, each a `ParticleSystem`
//! with a fixed pool. This module provides the same model as host
//! infrastructure, so every Wasm plugin shares one simulation:
//!
//! - a [`ParticleType`] is a *recipe* — the ranges, colors, fade and color
//!   modes — from which [`ParticleType::create`] samples one [`Particle`];
//! - a [`Particle`] carries the sampled state and integrates it each frame;
//! - a [`Layer`] is one of the two depth slots, with a fixed-capacity ring the
//!   original writes through (`Add` overwrites the oldest slot).
//!
//! Plugins emit through the `host_emit_particle` FFI; the host integrates both
//! layers and renders them as atlas blits.

use ruleste_plugins_api::types::Color;

use crate::engine::draw;

/// How a particle's color evolves over its life, `ParticleType.ColorModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// The color never changes.
    #[default]
    Static,
    /// `Color2` is picked once at spawn instead of `Color`.
    Choose,
    /// Alternates between the two colors every 0.1s.
    Blink,
    /// Lerps from `Color2` to the spawn color as the particle ages.
    Fade,
}

/// How a particle's alpha evolves, `ParticleType.FadeModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FadeMode {
    /// Fully opaque for the whole life.
    #[default]
    None,
    /// Alpha scales with the remaining life fraction.
    Linear,
    /// Stays opaque, then fades over the last quarter of the life.
    Late,
    /// Fades in over the first quarter, holds, fades out over the last.
    InAndOut,
}

/// How a particle's rotation is chosen, `ParticleType.RotationModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RotationMode {
    /// Fixed at 0 (or at the spawn direction for `SameAsDirection`).
    #[default]
    None,
    /// A random angle at spawn.
    Random,
    /// Follows the particle's velocity direction each frame.
    SameAsDirection,
}

/// The default particle texture, `Draw.Particle` in the original.
pub const DEFAULT_PARTICLE_FRAME: &str = "util/particle";

/// A particle recipe, mirroring `Monocle.ParticleType`.
///
/// Every `*Min` / `*Max` pair is sampled per spawn; the midpoint is the mean
/// and the spread is half-width, matching `Calc.Random.Range`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleType {
    /// Atlas frame the particle is blitted from.
    pub source: String,
    /// The spawn color (`Color` in the original).
    pub color: Color,
    /// The second color, used by `Choose`, `Blink` and `Fade`.
    pub color2: Color,
    pub color_mode: ColorMode,
    pub fade_mode: FadeMode,
    pub speed_min: f32,
    pub speed_max: f32,
    /// Per-second multiplier applied to the speed, `SpeedMultiplier`.
    pub speed_multiplier: f32,
    pub acceleration: (f32, f32),
    /// How fast the speed decays toward zero, per second.
    pub friction: f32,
    /// Base direction in radians; 0 is +X, matching `Calc.AngleToVector`.
    pub direction: f32,
    /// Full width of the random direction spread, centered on `direction`.
    pub direction_range: f32,
    pub life_min: f32,
    pub life_max: f32,
    pub size: f32,
    /// Full width of the random size spread, centered on `size`.
    pub size_range: f32,
    pub spin_min: f32,
    pub spin_max: f32,
    /// 50% chance to flip the spin's sign.
    pub spin_flipped_chance: bool,
    pub rotation_mode: RotationMode,
    /// Shrinks with `Ease.CubeOut` over the particle's life.
    pub scale_out: bool,
}

impl Default for ParticleType {
    /// The original's field-initialized defaults.
    fn default() -> Self {
        ParticleType {
            source: DEFAULT_PARTICLE_FRAME.to_string(),
            color: Color::WHITE,
            color2: Color::WHITE,
            color_mode: ColorMode::Static,
            fade_mode: FadeMode::None,
            speed_min: 0.0,
            speed_max: 0.0,
            speed_multiplier: 1.0,
            acceleration: (0.0, 0.0),
            friction: 0.0,
            direction: 0.0,
            direction_range: 0.0,
            life_min: 0.0,
            life_max: 0.0,
            size: 2.0,
            size_range: 0.0,
            spin_min: 0.0,
            spin_max: 0.0,
            spin_flipped_chance: false,
            rotation_mode: RotationMode::None,
            scale_out: false,
        }
    }
}

impl ParticleType {
    /// `new ParticleType { ... }` in the original: start from the defaults and
    /// override fields, which is how every `ParticleTypes.cs` entry is
    /// written.
    #[must_use]
    pub fn with(mut self, configure: impl FnOnce(&mut ParticleType)) -> ParticleType {
        configure(&mut self);
        self
    }

    /// `new ParticleType(copyFrom) { ... }`: clone a type and override fields.
    #[must_use]
    pub fn copy_from(base: &ParticleType) -> ParticleType {
        base.clone()
    }

    /// Samples one particle, mirroring `ParticleType.Create`.
    ///
    /// `rng` is the host's xorshift stream, playing the role of
    /// `Calc.Random`.
    #[must_use]
    pub fn create(
        &self,
        position: (f32, f32),
        direction: Option<f32>,
        color: Option<Color>,
        rng: &mut Rng,
    ) -> Particle {
        let start_color = match self.color_mode {
            // `ColorModes.Choose` picks between the two colors at spawn.
            ColorMode::Choose => {
                if rng.next_f32() < 0.5 {
                    color.unwrap_or(self.color)
                } else {
                    self.color2
                }
            }
            _ => color.unwrap_or(self.color),
        };

        let start_size = if self.size_range != 0.0 {
            self.size - self.size_range * 0.5 + rng.next_f32() * self.size_range
        } else {
            self.size
        };

        // `direction - DirectionRange / 2 + rand01 * DirectionRange`, then the
        // speed sampled in [SpeedMin, SpeedMax].
        let base_direction = direction.unwrap_or(self.direction);
        let angle =
            base_direction - self.direction_range * 0.5 + rng.next_f32() * self.direction_range;
        let speed = rng.range(self.speed_min, self.speed_max);
        let (speed_x, speed_y) = angle_to_vector(angle, speed);

        let life = rng.range(self.life_min, self.life_max);
        let rotation = match self.rotation_mode {
            RotationMode::Random => rng.next_f32() * std::f32::consts::TAU,
            RotationMode::SameAsDirection => angle,
            RotationMode::None => 0.0,
        };
        let mut spin = rng.range(self.spin_min, self.spin_max);
        if self.spin_flipped_chance && rng.next_f32() < 0.5 {
            spin = -spin;
        }

        Particle {
            active: true,
            source: self.source.clone(),
            position,
            speed: (speed_x, speed_y),
            size: start_size,
            start_size,
            life,
            start_life: life,
            color: start_color,
            start_color,
            rotation,
            spin,
            acceleration: self.acceleration,
            friction: self.friction,
            speed_multiplier: self.speed_multiplier,
            fade_mode: self.fade_mode,
            color_mode: self.color_mode,
            color2: self.color2,
            scale_out: self.scale_out,
            rotation_mode: self.rotation_mode,
        }
    }
}

/// A live particle, mirroring `Monocle.Particle`.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    pub active: bool,
    pub source: String,
    pub position: (f32, f32),
    pub speed: (f32, f32),
    pub size: f32,
    pub start_size: f32,
    pub life: f32,
    pub start_life: f32,
    pub color: Color,
    pub start_color: Color,
    pub rotation: f32,
    pub spin: f32,
    pub acceleration: (f32, f32),
    pub friction: f32,
    pub speed_multiplier: f32,
    pub fade_mode: FadeMode,
    pub color_mode: ColorMode,
    pub color2: Color,
    pub scale_out: bool,
    pub rotation_mode: RotationMode,
}

impl Particle {
    /// `Particle.Update(dt)`: advance one step, fading and deactivating it once
    /// its life runs out.
    pub fn update(&mut self, dt: f32) {
        // The original samples `life / startLife` *before* decrementing, so
        // the color/size curve lags one frame behind the countdown.
        let life_fraction = if self.start_life > 0.0 {
            self.life / self.start_life
        } else {
            0.0
        };
        self.life -= dt;
        if self.life <= 0.0 {
            self.active = false;
            return;
        }

        // Rotation: either locked to the velocity direction, or spun.
        if self.rotation_mode == RotationMode::SameAsDirection {
            if self.speed != (0.0, 0.0) {
                self.rotation = angle_of(self.speed);
            }
        } else {
            self.rotation += self.spin * dt;
        }

        let fade = match self.fade_mode {
            FadeMode::Linear => life_fraction,
            FadeMode::Late => (life_fraction / 0.25).min(1.0),
            FadeMode::InAndOut => {
                if life_fraction > 0.75 {
                    1.0 - (life_fraction - 0.75) / 0.25
                } else if life_fraction < 0.25 {
                    life_fraction / 0.25
                } else {
                    1.0
                }
            }
            FadeMode::None => 1.0,
        };

        if fade <= 0.0 {
            self.color = Color::TRANSPARENT;
        } else {
            self.color = match self.color_mode {
                ColorMode::Static => self.start_color,
                ColorMode::Fade => lerp_color(self.color2, self.start_color, life_fraction),
                // `Calc.BetweenInterval(Life, 0.1f)` toggles on 0.1s slices.
                ColorMode::Blink => {
                    if (self.life / 0.1).fract() < f32::EPSILON {
                        self.start_color
                    } else {
                        self.color2
                    }
                }
                ColorMode::Choose => self.start_color,
            };
            if fade < 1.0 {
                self.color = scale_color(self.color, fade);
            }
        }

        self.position.0 += self.speed.0 * dt;
        self.position.1 += self.speed.1 * dt;
        self.speed.0 += self.acceleration.0 * dt;
        self.speed.1 += self.acceleration.1 * dt;
        // `Speed = Calc.Approach(Speed, Vector2.Zero, Friction * dt)`.
        self.speed.0 = approach(self.speed.0, 0.0, self.friction * dt);
        self.speed.1 = approach(self.speed.1, 0.0, self.friction * dt);
        if self.speed_multiplier != 1.0 {
            let k = self.speed_multiplier.powf(dt);
            self.speed.0 *= k;
            self.speed.1 *= k;
        }
        if self.scale_out {
            self.size = self.start_size * ease_cube_out(life_fraction);
        }
    }

    /// `Particle.Render`: the position is floored to whole pixels so
    /// particles do not shimmer as they move.
    #[must_use]
    pub fn render_position(&self) -> (f32, f32) {
        (self.position.0.floor(), self.position.1.floor())
    }
}

/// One of the two depth layers a level draws particles in: `level.Particles`
/// (behind the entities) and `level.ParticlesFG` (in front).
#[derive(Debug, Clone, Default)]
pub struct Layer {
    /// A fixed-size ring, as in the original's `Particle[] particles`. Writing
    /// past the end overwrites the oldest particle instead of growing.
    particles: Vec<Particle>,
    next_slot: usize,
}

impl Layer {
    /// `new ParticleSystem(depth, maxParticles)`.
    #[must_use]
    pub fn with_capacity(max_particles: usize) -> Layer {
        Layer {
            particles: vec![Particle::inactive(); max_particles],
            next_slot: 0,
        }
    }

    /// `ParticleSystem.Add`: writes into the next ring slot, wrapping.
    pub fn add(&mut self, particle: Particle) {
        if self.particles.is_empty() {
            return;
        }
        self.particles[self.next_slot] = particle;
        self.next_slot = (self.next_slot + 1) % self.particles.len();
    }

    /// `ParticleSystem.Emit(type, position)`.
    pub fn emit(&mut self, particle_type: &ParticleType, position: (f32, f32), rng: &mut Rng) {
        self.add(particle_type.create(position, None, None, rng));
    }

    /// `ParticleSystem.Emit(type, position, direction)`.
    pub fn emit_dir(
        &mut self,
        particle_type: &ParticleType,
        position: (f32, f32),
        direction: f32,
        rng: &mut Rng,
    ) {
        self.add(particle_type.create(position, Some(direction), None, rng));
    }

    /// `ParticleSystem.Emit(type, amount, position, positionRange)`: each
    /// particle is offset by a random point in a box around `position`.
    pub fn emit_amount(
        &mut self,
        particle_type: &ParticleType,
        amount: usize,
        position: (f32, f32),
        position_range: (f32, f32),
        rng: &mut Rng,
    ) {
        for _ in 0..amount {
            let x = rng.range(position.0 - position_range.0, position.0 + position_range.0);
            let y = rng.range(position.1 - position_range.1, position.1 + position_range.1);
            self.emit(particle_type, (x, y), rng);
        }
    }

    /// `ParticleSystem.Emit(type, amount, position, positionRange, direction)`.
    pub fn emit_amount_dir(
        &mut self,
        particle_type: &ParticleType,
        amount: usize,
        position: (f32, f32),
        position_range: (f32, f32),
        direction: f32,
        rng: &mut Rng,
    ) {
        for _ in 0..amount {
            let x = rng.range(position.0 - position_range.0, position.0 + position_range.0);
            let y = rng.range(position.1 - position_range.1, position.1 + position_range.1);
            self.emit_dir(particle_type, (x, y), direction, rng);
        }
    }

    /// `ParticleSystem.Clear`.
    pub fn clear(&mut self) {
        for p in &mut self.particles {
            p.active = false;
        }
    }

    /// Advances every active particle.
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles.iter_mut().filter(|p| p.active) {
            p.update(dt);
        }
    }

    /// The live particles, in ring order.
    pub fn iter(&self) -> impl Iterator<Item = &Particle> {
        self.particles.iter().filter(|p| p.active)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.particles.iter().filter(|p| p.active).count()
    }
}

impl Particle {
    /// The all-zero particle a `Particle[]` is initialized with.
    #[must_use]
    pub fn inactive() -> Particle {
        Particle {
            active: false,
            source: String::new(),
            position: (0.0, 0.0),
            speed: (0.0, 0.0),
            size: 0.0,
            start_size: 0.0,
            life: 0.0,
            start_life: 0.0,
            color: Color::TRANSPARENT,
            start_color: Color::TRANSPARENT,
            rotation: 0.0,
            spin: 0.0,
            acceleration: (0.0, 0.0),
            friction: 0.0,
            speed_multiplier: 1.0,
            fade_mode: FadeMode::None,
            color_mode: ColorMode::Static,
            color2: Color::TRANSPARENT,
            scale_out: false,
            rotation_mode: RotationMode::None,
        }
    }
}

/// The host's two particle layers plus the shared RNG.
///
/// `emit_particle` targets the background layer; the foreground layer exists
/// for effects that must draw over the player (`P_DashA` and friends go here,
/// as `level.ParticlesFG.Emit` does in the original).
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    /// `level.Particles`: behind the entities.
    pub background: Layer,
    /// `level.ParticlesFG`: in front of the entities.
    pub foreground: Layer,
    rng: Rng,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        // The original sizes its pools generously; 512 per layer keeps bursts
        // (a strawberry collect, a dream-block shatter) from recycling visibly.
        ParticleSystem {
            background: Layer::with_capacity(512),
            foreground: Layer::with_capacity(512),
            rng: Rng::new(0x2545_F491_4F6C_DD1D),
        }
    }
}

impl ParticleSystem {
    #[must_use]
    pub fn new() -> ParticleSystem {
        ParticleSystem::default()
    }

    /// The shared xorshift stream, playing the role of `Calc.Random`.
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    /// Removes every particle from both layers.
    pub fn clear(&mut self) {
        self.background.clear();
        self.foreground.clear();
    }

    /// Integrates both layers.
    pub fn update(&mut self, dt: f32) {
        self.background.update(dt);
        self.foreground.update(dt);
    }

    /// The total live particle count across both layers.
    #[must_use]
    pub fn count(&self) -> usize {
        self.background.count() + self.foreground.count()
    }
}

/// Appends one alpha-blended rectangle per live particle into `rects`.
///
/// Particles are textured in the original (`Draw.Particle`, the dash bursts,
/// feathers, ...); this fallback draws the plain square for the types whose
/// source has no dedicated blit path, and keeps the API compatible with the
/// rectangle renderer.
pub fn append_to_rects(layer: &Layer, rects: &mut Vec<draw::Rect>) {
    for p in layer.iter() {
        if p.color.a == 0 {
            continue;
        }
        let (x, y) = p.render_position();
        let s = p.size.max(0.0);
        rects.push(draw::Rect {
            x: x - s,
            y: y - s,
            w: s * 2.0,
            h: s * 2.0,
            color: p.color,
        });
    }
}

/// A small xorshift64 generator standing in for `Calc.Random`.
///
/// The plugin side cannot share the host's stream, so each layer samples from
/// this instead; the distribution is what matters visually, not the exact
/// sequence.
#[derive(Debug, Clone, Copy)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seeds the generator. A zero seed would be a fixed point, so it is
    /// replaced with a nonzero constant.
    #[must_use]
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// The next raw 64-bit value (xorshift64).
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// A float in `[0, 1)`, `Calc.Random.NextFloat()`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// A float in `[min, max)`, `Calc.Random.Range(min, max)` for floats.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }
}

// ---------------------------------------------------------------------------
// Math helpers, matching the original's `Calc` / `Ease` / `Color` behavior
// ---------------------------------------------------------------------------

/// `Calc.AngleToVector(angle, length)`: 0 radians points along +X, and the
/// angle grows clockwise on screen (so +Y is downward).
fn angle_to_vector(angle: f32, length: f32) -> (f32, f32) {
    (angle.cos() * length, angle.sin() * length)
}

/// `Vector2.Angle()`.
fn angle_of(v: (f32, f32)) -> f32 {
    v.1.atan2(v.0)
}

/// `Calc.Approach(value, target, maxMove)`.
fn approach(value: f32, target: f32, max_move: f32) -> f32 {
    if value > target {
        (value - max_move).max(target)
    } else {
        (value + max_move).min(target)
    }
}

/// `Color.Lerp(from, to, amount)`.
fn lerp_color(from: Color, to: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color {
        r: mix(from.r, to.r),
        g: mix(from.g, to.g),
        b: mix(from.b, to.b),
        a: mix(from.a, to.a),
    }
}

/// `Color * float`, clamping to the byte range.
fn scale_color(c: Color, k: f32) -> Color {
    let scale = |v: u8| ((v as f32 * k).round() as i32).clamp(0, 255) as u8;
    Color {
        r: scale(c.r),
        g: scale(c.g),
        b: scale(c.b),
        a: scale(c.a),
    }
}

/// `Ease.CubeOut(t)`.
fn ease_cube_out(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(v: u32) -> Color {
        Color {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
            a: 0xFF,
        }
    }

    /// `Player.P_DashA` from `ParticleTypes.cs`.
    fn dash_a() -> ParticleType {
        ParticleType::default().with(|t| {
            t.color = hex(0x44B7FF);
            t.color2 = hex(0x75C9FF);
            t.color_mode = ColorMode::Blink;
            t.fade_mode = FadeMode::Late;
            t.life_min = 1.0;
            t.life_max = 1.8;
            t.size = 1.0;
            t.speed_min = 10.0;
            t.speed_max = 20.0;
            t.acceleration = (0.0, 8.0);
            t.direction_range = std::f32::consts::PI / 3.0;
        })
    }

    #[test]
    fn defaults_match_particle_type_constructor() {
        let t = ParticleType::default();
        assert_eq!(t.source, "util/particle");
        assert_eq!(t.color, Color::WHITE);
        assert_eq!(t.color2, Color::WHITE);
        assert_eq!(t.color_mode, ColorMode::Static);
        assert_eq!(t.fade_mode, FadeMode::None);
        assert_eq!(t.speed_multiplier, 1.0);
        assert_eq!(t.size, 2.0);
        assert_eq!(t.acceleration, (0.0, 0.0));
        assert!(!t.scale_out);
    }

    #[test]
    fn create_samples_life_speed_and_size_within_range() {
        let t = dash_a();
        let mut rng = Rng::new(12345);
        for _ in 0..64 {
            let p = t.create((10.0, 20.0), None, None, &mut rng);
            assert!(
                p.life >= t.life_min && p.life <= t.life_max,
                "life {}",
                p.life
            );
            let speed = (p.speed.0 * p.speed.0 + p.speed.1 * p.speed.1).sqrt();
            assert!(
                speed >= t.speed_min - 1e-3 && speed <= t.speed_max + 1e-3,
                "speed {speed}"
            );
            assert_eq!(p.size, 1.0, "no SizeRange means a fixed size");
            assert_eq!(p.start_size, 1.0);
        }
    }

    #[test]
    fn size_range_spreads_symmetrically_around_size() {
        let t = ParticleType::default().with(|t| t.size_range = 2.0);
        let mut rng = Rng::new(99);
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for _ in 0..256 {
            let p = t.create((0.0, 0.0), None, None, &mut rng);
            min = min.min(p.size);
            max = max.max(p.size);
        }
        // `Size - SizeRange/2 + rand01 * SizeRange` spans [1, 3).
        assert!(
            min >= 1.0 - 1e-3 && max <= 3.0 + 1e-3,
            "sizes spanned {min}..{max}"
        );
        assert!(max - min > 1.0, "the range is actually used");
    }

    #[test]
    fn direction_range_centers_on_the_given_direction() {
        let t = dash_a();
        let mut rng = Rng::new(7);
        // Aim at "up" (-PI/2); with DirectionRange = PI/3 every spawn should
        // land within +/- PI/6 of it.
        let want = -std::f32::consts::FRAC_PI_2;
        for _ in 0..128 {
            let p = t.create((0.0, 0.0), Some(want), None, &mut rng);
            let a = angle_of(p.speed);
            let mut diff = (a - want).abs();
            if diff > std::f32::consts::PI {
                diff = std::f32::consts::TAU - diff;
            }
            assert!(
                diff <= t.direction_range / 2.0 + 1e-3,
                "angle {a} vs {want}"
            );
        }
    }

    #[test]
    fn late_fade_holds_opaque_then_fades_the_last_quarter() {
        let t = dash_a();
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.life = p.start_life * 0.9;
        p.update(0.01);
        assert_eq!(p.color.a, 0xFF, "still opaque at 90% life");
        p.life = p.start_life * 0.2;
        p.update(0.01);
        assert!(p.color.a < 0xFF, "fading at 20% life: {:?}", p.color);
    }

    #[test]
    fn linear_fade_scales_with_remaining_life() {
        let t = ParticleType::default().with(|t| {
            t.fade_mode = FadeMode::Linear;
            t.life_min = 1.0;
            t.life_max = 1.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        // The original samples `life / startLife` *before* the decrement, so
        // the alpha after a 0.25s step reflects the fraction at the start of
        // that step (1.0), not the fraction it ends on (0.75).
        p.update(0.25);
        assert_eq!(p.color.a, 0xFF, "still opaque on the first step");
        p.update(0.25);
        // This step samples 0.75, so the alpha drops to three quarters.
        assert!(
            (p.color.a as i32 - 191).abs() <= 1,
            "alpha {} after half the life",
            p.color.a
        );
    }

    #[test]
    fn in_and_out_fade_ramps_at_both_ends() {
        let t = ParticleType::default().with(|t| {
            t.fade_mode = FadeMode::InAndOut;
            t.life_min = 1.0;
            t.life_max = 1.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        // Just born: still ramping up.
        p.update(0.1);
        assert!(p.color.a < 0xFF, "fading in at 10%: {}", p.color.a);
        // Mid-life: fully opaque.
        p.life = 0.5;
        p.update(0.01);
        assert_eq!(p.color.a, 0xFF, "opaque mid-life");
        // Tail: ramping down.
        p.life = 0.1;
        p.update(0.01);
        assert!(p.color.a < 0xFF, "fading out at 10%: {}", p.color.a);
    }

    #[test]
    fn fade_color_mode_lerps_from_color2_to_the_spawn_color() {
        let t = ParticleType::default().with(|t| {
            t.color_mode = ColorMode::Fade;
            t.color = hex(0xFFFFFF);
            t.color2 = hex(0x000000);
            t.life_min = 1.0;
            t.life_max = 1.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        // Newborn: the lerp is at t=1, so it is the spawn color.
        p.update(0.01);
        assert_eq!(p.color.r, 0xFF, "newborn is the spawn color");
        // Halfway: `Color.Lerp(Color2, StartColor, 0.5)`.
        p.life = 0.5;
        p.update(0.01);
        assert!(
            (p.color.r as i32 - 128).abs() <= 1,
            "mid-life lerp gave {}",
            p.color.r
        );
        // Nearly expired: the lerp is well on its way back to `Color2`.
        p.life = 0.02;
        p.update(0.01);
        assert!(p.color.r < 0x40, "aged toward color2: {}", p.color.r);
    }

    #[test]
    fn acceleration_and_friction_integrate_like_the_original() {
        let t = ParticleType::default().with(|t| {
            t.acceleration = (0.0, 100.0);
            t.friction = 100.0;
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.speed = (0.0, 50.0);
        p.update(0.1);
        // Accelerate to 60, then friction pulls it back to 50.
        assert!((p.speed.1 - 50.0).abs() < 1e-3, "speed {}", p.speed.1);
    }

    #[test]
    fn speed_multiplier_damps_over_time() {
        let t = ParticleType::default().with(|t| {
            t.speed_multiplier = 0.5;
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.speed = (100.0, 0.0);
        p.update(1.0);
        // `Speed *= pow(0.5, 1.0)` halves it.
        assert!((p.speed.0 - 50.0).abs() < 1e-3, "speed {}", p.speed.0);
    }

    #[test]
    fn scale_out_shrinks_with_cube_out() {
        let t = ParticleType::default().with(|t| {
            t.scale_out = true;
            t.size = 8.0;
            t.life_min = 1.0;
            t.life_max = 1.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.life = 0.5;
        p.update(0.01);
        // `StartSize * Ease.CubeOut(0.5)` = 8 * (1 - 0.125) = 7.
        assert!((p.size - 7.0).abs() < 0.05, "size {}", p.size);
    }

    #[test]
    fn same_as_direction_rotation_tracks_the_velocity() {
        let t = ParticleType::default().with(|t| {
            t.rotation_mode = RotationMode::SameAsDirection;
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.speed = (0.0, 100.0);
        p.update(0.016);
        assert!(
            (p.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "rotation {}",
            p.rotation
        );
    }

    #[test]
    fn spin_flipped_chance_can_reverse_the_spin() {
        let t = ParticleType::default().with(|t| {
            t.spin_min = 10.0;
            t.spin_max = 10.0;
            t.spin_flipped_chance = true;
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        let mut rng = Rng::new(4242);
        let mut negative = 0;
        for _ in 0..64 {
            if t.create((0.0, 0.0), None, None, &mut rng).spin < 0.0 {
                negative += 1;
            }
        }
        assert!(
            (20..44).contains(&negative),
            "{negative} of 64 spins were flipped"
        );
    }

    #[test]
    fn a_particle_deactivates_when_its_life_runs_out() {
        let t = ParticleType::default().with(|t| {
            t.life_min = 0.5;
            t.life_max = 0.5;
        });
        let mut p = t.create((0.0, 0.0), None, None, &mut Rng::new(1));
        p.update(0.6);
        assert!(!p.active, "an expired particle deactivates");
    }

    #[test]
    fn layer_recycles_the_oldest_slot_when_full() {
        let mut layer = Layer::with_capacity(2);
        let mut rng = Rng::new(5);
        let t = ParticleType::default().with(|t| {
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        layer.emit(&t, (1.0, 1.0), &mut rng);
        layer.emit(&t, (2.0, 2.0), &mut rng);
        assert_eq!(layer.count(), 2);
        // The third write wraps onto slot 0.
        layer.emit(&t, (3.0, 3.0), &mut rng);
        assert_eq!(layer.count(), 2, "the pool does not grow past its capacity");
        let xs: Vec<f32> = layer.iter().map(|p| p.position.0).collect();
        assert!(xs.contains(&2.0) && xs.contains(&3.0), "slots hold {xs:?}");
        assert!(!xs.contains(&1.0), "the oldest particle was recycled");
    }

    #[test]
    fn emit_amount_scatters_within_the_position_range() {
        let mut layer = Layer::with_capacity(64);
        let mut rng = Rng::new(31);
        let t = ParticleType::default().with(|t| {
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        layer.emit_amount(&t, 32, (100.0, 200.0), (4.0, 4.0), &mut rng);
        assert_eq!(layer.count(), 32);
        for p in layer.iter() {
            assert!((p.position.0 - 100.0).abs() <= 4.0, "x {}", p.position.0);
            assert!((p.position.1 - 200.0).abs() <= 4.0, "y {}", p.position.1);
        }
    }

    #[test]
    fn clear_deactivates_everything() {
        let mut sys = ParticleSystem::default();
        let mut rng = Rng::new(1);
        let t = ParticleType::default().with(|t| {
            t.life_min = 10.0;
            t.life_max = 10.0;
        });
        sys.background.emit(&t, (0.0, 0.0), &mut rng);
        sys.foreground.emit(&t, (0.0, 0.0), &mut rng);
        assert_eq!(sys.count(), 2);
        sys.clear();
        assert_eq!(sys.count(), 0);
    }

    #[test]
    fn render_position_is_floored_to_whole_pixels() {
        let mut p = Particle::inactive();
        p.position = (10.9, -3.2);
        assert_eq!(p.render_position(), (10.0, -4.0));
    }
}
