//! A traversable wormhole: two regions of flat space joined by a throat, and the paths of light
//! through it.
//!
//! # The geometry
//!
//! ```text
//! ds² = −c²dt² + dℓ² + r(ℓ)² (dθ² + sin²θ dφ²)
//! ```
//!
//! `ℓ` is the proper distance along the wormhole: positive on one side, negative on the other, zero
//! in the middle. `r(ℓ)` is the radius of the sphere at `ℓ` (its circumference over `2π`). It never
//! falls below the throat's radius `ρ`, so there is no centre, no horizon and no singularity. Two
//! shapes are here ([`WormholeShape`]):
//!
//! - **Dneg**, three parameters: a cylinder of radius `ρ` from `−a` to `a`, and mouths that flare
//!   out over a length set by `ℳ`:
//!   `r = ρ + ℳ [x atan x − ½ ln(1 + x²)]`, `x = 2(|ℓ| − a)/(πℳ)`, and `r = ρ` for `|ℓ| < a`.
//!   James, von Tunzelmann, Franklin & Thorne (2015), "Visualizing Interstellar's Wormhole",
//!   American Journal of Physics, doi:10.1119/1.4916949, arXiv:1502.03809, eqs. (5a–c). How sharply
//!   it flares is given as the lensing width `𝒲 = 1.42953… ℳ` (their eq. 7).
//! - **Ellis**, `r² = ρ² + ℓ²` (the same paper's eqs. 1–2, credited there to Ellis 1973). Kept
//!   because light's whole bend through it has a closed form, which the tests hold the integrator to.
//!
//! # Light, and anything coasting
//!
//! Space is curved but clocks all run alike (`g_tt = −1`), so there is no gravity: a body at rest
//! stays at rest. A path stays in the plane through the centre it starts in, and in that plane, with
//! the time (or, for a body, the distance) `s` along it (their eqs. A.7, in the plane):
//!
//! ```text
//! dℓ/ds = p      dα/ds = B/r²      dp/ds = B² r′/r³      p² + B²/r² = 1
//! ```
//!
//! `B`, the impact parameter, is conserved. Since `p² = 1 − B²/r²` and `r ≥ ρ`, a path crosses
//! exactly when `B < ρ`; otherwise it turns back where `r = B`. As `g_tt = −1`, light's path and the
//! straightest path of a moving body are the same curve.
//!
//! # What is not here
//!
//! What holds it open. The geometry needs matter of negative energy density, which the paper says
//! likely cannot be had in this amount; this module is kinematics in a given space.

/// The shape of a wormhole: how `r` depends on `ℓ`. Any unit of length, the same for all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WormholeShape {
    /// `r² = ρ² + ℓ²`.
    Ellis {
        /// The throat's radius `ρ`.
        throat: f64,
    },
    /// A cylinder with flaring mouths.
    Dneg {
        /// The throat's radius `ρ`.
        throat: f64,
        /// Half the cylinder's length, `a`.
        half_length: f64,
        /// The length `ℳ` the mouths flare over.
        flare: f64,
    },
}

/// `𝒲/ℳ`: the lensing width of a Dneg wormhole, for a flare of `1`.
///
/// The radius gained between the throat, where the embedded surface is vertical, and where it has
/// turned to `45°` (`dr/dℓ = 1/√2`, so `atan x = π/(2√2)`): `x atan x − ½ ln(1 + x²)` there.
/// The paper's footnote 19 gives the same expression.
pub fn lensing_width_per_flare() -> f64 {
    let angle = std::f64::consts::PI / (2.0 * std::f64::consts::SQRT_2);
    angle * angle.tan() + angle.cos().ln()
}

impl WormholeShape {
    /// A Dneg wormhole by its lensing width `𝒲` instead of its flare.
    ///
    /// # Returns
    ///
    /// `None` unless the throat and the width are finite and positive and the half-length is finite
    /// and not negative.
    pub fn dneg(throat: f64, half_length: f64, lensing_width: f64) -> Option<Self> {
        let usable = |x: f64| x.is_finite() && x > 0.0;
        (usable(throat) && usable(lensing_width) && half_length.is_finite() && half_length >= 0.0)
            .then(|| Self::Dneg {
                throat,
                half_length,
                flare: lensing_width / lensing_width_per_flare(),
            })
    }

    /// The throat's radius `ρ`.
    pub fn throat(&self) -> f64 {
        match *self {
            Self::Ellis { throat } | Self::Dneg { throat, .. } => throat,
        }
    }

    /// Where the mouths are: `|ℓ| = a`, `0` for an Ellis wormhole.
    pub fn half_length(&self) -> f64 {
        match *self {
            Self::Ellis { .. } => 0.0,
            Self::Dneg { half_length, .. } => half_length,
        }
    }

    /// The radius `r(ℓ)`.
    pub fn radius(&self, l: f64) -> f64 {
        match *self {
            Self::Ellis { throat } => throat.hypot(l),
            Self::Dneg {
                throat,
                half_length,
                flare,
            } => {
                let outside = l.abs() - half_length;
                if outside <= 0.0 {
                    return throat;
                }
                let x = 2.0 * outside / (std::f64::consts::PI * flare);
                throat + flare * (x * x.atan() - 0.5 * (x * x).ln_1p())
            }
        }
    }

    /// `dr/dℓ`, from `−1` (far away on the negative side) to `1`.
    pub fn slope(&self, l: f64) -> f64 {
        match *self {
            Self::Ellis { throat } => l / throat.hypot(l),
            Self::Dneg {
                half_length, flare, ..
            } => {
                let outside = l.abs() - half_length;
                if outside <= 0.0 {
                    return 0.0;
                }
                let x = 2.0 * outside / (std::f64::consts::PI * flare);
                l.signum() * std::f64::consts::FRAC_2_PI * x.atan()
            }
        }
    }

    /// The `|ℓ|` at which the radius is `radius`: the inverse of [`radius`](Self::radius) outside
    /// the mouths. A radius at or below the throat's gives the mouth itself.
    pub fn distance_at_radius(&self, radius: f64) -> f64 {
        let mouth = self.half_length();
        if radius <= self.throat() {
            return mouth;
        }
        // Double until it is bracketed, then bisect.
        let mut low = mouth;
        let mut high = mouth + radius;
        while self.radius(high) < radius {
            low = high;
            high *= 2.0;
        }
        for _ in 0..200 {
            let middle = 0.5 * (low + high);
            if middle <= low || middle >= high {
                break;
            }
            if self.radius(middle) < radius {
                low = middle;
            } else {
                high = middle;
            }
        }
        0.5 * (low + high)
    }

    /// The height `z(ℓ)` of the equatorial plane drawn as a surface in flat space, from the middle
    /// (the paper's eq. 6): `∫₀^ℓ √(1 − r′²) dℓ′`, by Simpson's rule over `intervals` (made even).
    pub fn embedding_height(&self, l: f64, intervals: usize) -> f64 {
        let n = (intervals.max(2) + 1) & !1;
        let h = l / n as f64;
        let f = |x: f64| (1.0 - self.slope(x).powi(2)).max(0.0).sqrt();
        let inner: f64 = (1..n)
            .map(|i| f(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 })
            .sum();
        (f(0.0) + inner + f(l)) * h / 3.0
    }

    /// The length over which the geometry changes near `ℓ`, which sets how long a step may be.
    pub fn scale(&self, l: f64) -> f64 {
        match *self {
            Self::Ellis { throat } => throat.hypot(l),
            Self::Dneg {
                half_length, flare, ..
            } => flare.hypot((l.abs() - half_length).max(0.0)),
        }
    }
}

/// A point on a path through a wormhole, in the path's own plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WormholePath {
    /// Position `ℓ` along the wormhole.
    pub l: f64,
    /// Angle `α` round the centre since the path's start.
    pub angle: f64,
    /// `dℓ/ds`, from `−1` to `1`.
    pub p: f64,
}

impl WormholePath {
    /// A path starting at `l`, heading at `heading` radians from `+ℓ` (so `0` goes straight up `ℓ`
    /// and `π/2` goes round), and its impact parameter `B`.
    pub fn start(shape: &WormholeShape, l: f64, heading: f64) -> (Self, f64) {
        (
            Self {
                l,
                angle: 0.0,
                p: heading.cos(),
            },
            shape.radius(l) * heading.sin(),
        )
    }

    /// How far `p² + B²/r²` is from `1`.
    pub fn error(&self, shape: &WormholeShape, impact: f64) -> f64 {
        let across = impact / shape.radius(self.l);
        (self.p * self.p + across * across - 1.0).abs()
    }

    /// The heading from `+ℓ`, in `(−π, π]`.
    pub fn heading(&self, shape: &WormholeShape, impact: f64) -> f64 {
        (impact / shape.radius(self.l)).atan2(self.p)
    }

    /// The direction of travel in the plane as if the space were flat and `ℓ` pointed away from the
    /// centre: the angle `α` plus the [`heading`](Self::heading).
    pub fn direction(&self, shape: &WormholeShape, impact: f64) -> f64 {
        self.angle + self.heading(shape, impact)
    }

    /// The path a distance `h` further on: one fourth-order Runge–Kutta step.
    pub fn step(&self, shape: &WormholeShape, impact: f64, h: f64) -> Self {
        let rates = |l: f64, p: f64| {
            let r = shape.radius(l);
            let across = impact / (r * r);
            (p, across, impact * across * shape.slope(l) / r)
        };
        let k1 = rates(self.l, self.p);
        let k2 = rates(self.l + 0.5 * h * k1.0, self.p + 0.5 * h * k1.2);
        let k3 = rates(self.l + 0.5 * h * k2.0, self.p + 0.5 * h * k2.2);
        let k4 = rates(self.l + h * k3.0, self.p + h * k3.2);
        let sum = |a: f64, b: f64, c: f64, d: f64| h / 6.0 * (a + 2.0 * b + 2.0 * c + d);
        Self {
            l: self.l + sum(k1.0, k2.0, k3.0, k4.0),
            angle: self.angle + sum(k1.1, k2.1, k3.1, k4.1),
            p: self.p + sum(k1.2, k2.2, k3.2, k4.2),
        }
    }

    /// The path a distance `distance` further on, in steps of at most `fraction` of the local
    /// [`scale`](WormholeShape::scale) of the geometry.
    pub fn advance(
        mut self,
        shape: &WormholeShape,
        impact: f64,
        distance: f64,
        fraction: f64,
    ) -> Self {
        let mut left = distance;
        while left > 0.0 {
            let h = left.min(fraction * shape.scale(self.l));
            self = self.step(shape, impact, h);
            left -= h;
        }
        self
    }

    /// The path followed until it is heading away at `|ℓ| ≥ reach`, or `None` if that takes more
    /// than `max_steps` steps (`B` is as good as `ρ`, and it circles the throat).
    pub fn escape(
        mut self,
        shape: &WormholeShape,
        impact: f64,
        reach: f64,
        fraction: f64,
        max_steps: usize,
    ) -> Option<Self> {
        for _ in 0..max_steps {
            if self.l.abs() >= reach && self.l * self.p > 0.0 {
                return Some(self);
            }
            self = self.step(shape, impact, fraction * shape.scale(self.l));
        }
        None
    }
}

#[cfg(test)]
#[path = "../../../tst/physics_utils/relativity/wormhole_tests.rs"]
mod tests;
