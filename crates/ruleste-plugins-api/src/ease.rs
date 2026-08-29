//! Easing functions, mirroring the original Celeste `Ease` class. These are
//! pure math helpers compiled into both the host and the Wasm plugins (the
//! `ruleste-plugins-api` crate links into both), so plugins can tween motion,
//! alpha and camera offsets without reimplementing Penner's equations.
//!
//! Every function takes `t` in `[0, 1]` and returns an eased value, typically
//! also in `[0, 1]` (except `Back`/`Elastic` which overshoot). Use them with
//! [`crate::ease::lerp`] to map the eased progress onto a value range.

/// Clamps `t` into the `[0, 1]` range.
#[must_use]
pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Linear interpolation: `a + (b - a) * t`.
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The identity easing: returns `t` unchanged.
#[must_use]
pub fn linear(t: f32) -> f32 {
    t
}

// --- Sine ---------------------------------------------------------------

#[must_use]
pub fn sine_in(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::FRAC_PI_2).cos()
}

#[must_use]
pub fn sine_out(t: f32) -> f32 {
    (t * std::f32::consts::FRAC_PI_2).sin()
}

#[must_use]
pub fn sine_in_out(t: f32) -> f32 {
    -0.5 * ((std::f32::consts::PI * t).cos() - 1.0)
}

// --- Quad ---------------------------------------------------------------

#[must_use]
pub fn quad_in(t: f32) -> f32 {
    t * t
}

#[must_use]
pub fn quad_out(t: f32) -> f32 {
    t * (2.0 - t)
}

#[must_use]
pub fn quad_in_out(t: f32) -> f32 {
    if t < 0.5 {
        quad_in(t * 2.0) * 0.5
    } else {
        1.0 - quad_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Cube ---------------------------------------------------------------

#[must_use]
pub fn cube_in(t: f32) -> f32 {
    t * t * t
}

#[must_use]
pub fn cube_out(t: f32) -> f32 {
    1.0 + (t - 1.0) * (t - 1.0) * (t - 1.0)
}

#[must_use]
pub fn cube_in_out(t: f32) -> f32 {
    if t < 0.5 {
        cube_in(t * 2.0) * 0.5
    } else {
        1.0 - cube_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Quart --------------------------------------------------------------

#[must_use]
pub fn quart_in(t: f32) -> f32 {
    let tt = t * t;
    tt * tt
}

#[must_use]
pub fn quart_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(4)
}

#[must_use]
pub fn quart_in_out(t: f32) -> f32 {
    if t < 0.5 {
        quart_in(t * 2.0) * 0.5
    } else {
        1.0 - quart_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Quint --------------------------------------------------------------

#[must_use]
pub fn quint_in(t: f32) -> f32 {
    let tt = t * t;
    t * tt * tt
}

#[must_use]
pub fn quint_out(t: f32) -> f32 {
    1.0 + (t - 1.0).powi(5)
}

#[must_use]
pub fn quint_in_out(t: f32) -> f32 {
    if t < 0.5 {
        quint_in(t * 2.0) * 0.5
    } else {
        1.0 - quint_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Expo ---------------------------------------------------------------

#[must_use]
pub fn expo_in(t: f32) -> f32 {
    (2.0_f32).powf(10.0 * t - 10.0)
}

#[must_use]
pub fn expo_out(t: f32) -> f32 {
    1.0 - (2.0_f32).powf(-10.0 * t)
}

#[must_use]
pub fn expo_in_out(t: f32) -> f32 {
    if t < 0.5 {
        expo_in(t * 2.0) * 0.5
    } else {
        1.0 - expo_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Circ ---------------------------------------------------------------

#[must_use]
pub fn circ_in(t: f32) -> f32 {
    1.0 - (1.0 - t * t).sqrt()
}

#[must_use]
pub fn circ_out(t: f32) -> f32 {
    (1.0 - (t - 1.0) * (t - 1.0)).sqrt()
}

#[must_use]
pub fn circ_in_out(t: f32) -> f32 {
    if t < 0.5 {
        circ_in(t * 2.0) * 0.5
    } else {
        1.0 - circ_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Back ---------------------------------------------------------------
// `S` is the overshoot constant from the original `Ease` (`1.70158`).

const BACK_S: f32 = 1.70158;

#[must_use]
pub fn back_in(t: f32) -> f32 {
    t * t * ((BACK_S + 1.0) * t - BACK_S)
}

#[must_use]
pub fn back_out(t: f32) -> f32 {
    1.0 + (t - 1.0) * (t - 1.0) * ((BACK_S + 1.0) * (t - 1.0) + BACK_S)
}

#[must_use]
pub fn back_in_out(t: f32) -> f32 {
    if t < 0.5 {
        back_in(t * 2.0) * 0.5
    } else {
        1.0 - back_out((1.0 - t) * 2.0) * 0.5
    }
}

// --- Elastic ------------------------------------------------------------
// Faithful to the original constants (`0.4349` is the phase multiplier used
// by Celeste's `Ease.Elastic*`).

#[must_use]
pub fn elastic_in(t: f32) -> f32 {
    1.0 + (2.0_f32).powf(-10.0 * t) * (t * 10.0 - 10.75).sin() * 0.4349
}

#[must_use]
pub fn elastic_out(t: f32) -> f32 {
    (2.0_f32).powf(-10.0 * t) * (t * 10.0 - 0.75).sin() * 0.4349 + 1.0
}

#[must_use]
pub fn elastic_in_out(t: f32) -> f32 {
    if t < 0.5 {
        elastic_in(t * 2.0) * 0.5
    } else {
        1.0 - elastic_in((1.0 - t) * 2.0) * 0.5
    }
}

// --- Bounce -------------------------------------------------------------

#[must_use]
pub fn bounce_out(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

#[must_use]
pub fn bounce_in(t: f32) -> f32 {
    1.0 - bounce_out(1.0 - t)
}

#[must_use]
pub fn bounce_in_out(t: f32) -> f32 {
    if t < 0.5 {
        bounce_in(t * 2.0) * 0.5
    } else {
        bounce_out(t * 2.0 - 1.0) * 0.5 + 0.5
    }
}

// --- Combinators --------------------------------------------------------

/// Inverts an eased progress: `1 - t`.
#[must_use]
pub fn invert(t: f32) -> f32 {
    1.0 - t
}

/// A frame-rate independent follow: eases `a` toward `b`, covering the
/// remaining distance at a rate such that 99.9% is closed every second. Mirrors
/// `Ease.Follow(a, b, dt)`.
#[must_use]
pub fn follow(a: f32, b: f32, dt: f32) -> f32 {
    a + (b - a) * (1.0 - 0.001_f32.powf(dt))
}

/// Triangle wave mapping `[0, 1]` to `[0, 1, 0]`: rises for the first half,
/// falls for the second. Mirrors `Ease.UpDown`.
#[must_use]
pub fn up_down(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t } else { 2.0 - 2.0 * t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoints() {
        assert_eq!(linear(0.0), 0.0);
        assert_eq!(linear(1.0), 1.0);
        assert_eq!(sine_in(0.0), 0.0);
        assert!((sine_in(1.0) - 1.0).abs() < 1e-5);
        assert_eq!(sine_out(0.0), 0.0);
        assert!((sine_out(1.0) - 1.0).abs() < 1e-5);
        assert_eq!(quad_in(0.0), 0.0);
        assert_eq!(quad_in(1.0), 1.0);
        assert_eq!(quad_out(0.0), 0.0);
        assert_eq!(quad_out(1.0), 1.0);
        assert_eq!(cube_in(0.0), 0.0);
        assert_eq!(cube_in(1.0), 1.0);
        assert_eq!(cube_out(0.0), 0.0);
        assert_eq!(cube_out(1.0), 1.0);
        assert_eq!(bounce_in(0.0), 0.0);
        assert!((bounce_in(1.0) - 1.0).abs() < 1e-5);
        assert_eq!(bounce_out(0.0), 0.0);
        assert!((bounce_out(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_invert_and_up_down() {
        assert_eq!(invert(0.25), 0.75);
        assert_eq!(up_down(0.0), 0.0);
        assert_eq!(up_down(0.5), 1.0);
        assert_eq!(up_down(1.0), 0.0);
    }

    #[test]
    fn test_lerp_and_clamp() {
        assert_eq!(clamp01(-0.5), 0.0);
        assert_eq!(clamp01(1.5), 1.0);
        assert_eq!(clamp01(0.4), 0.4);
        assert_eq!(lerp(10.0, 20.0, 0.5), 15.0);
    }
}
