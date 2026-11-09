//! Special relativity: the kinematics of two frames in uniform relative motion.
//!
//! The first departure in this crate from Newtonian mechanics. Everything here follows from two
//! postulates — the laws of physics are the same in every inertial frame, and `c` is the same in
//! every inertial frame — and nothing here needs a force, a mass, or a scene. It is geometry.
//!
//! # What is where
//!
//! | Module | Job |
//! | --- | --- |
//! | [`constants`] | `SPEED_OF_LIGHT_M_S`, exact by definition of the metre. |
//! | [`lorentz`] | `β` and `γ`, which every other module here is built from. |
//! | [`time_dilation`] | Moving clocks run slow: `t = γτ`. |
//! | [`length_contraction`] | Moving rulers are short: `L = L₀/γ`. |
//! | [`velocity`] | Transforming a velocity between frames, and composing two velocities. |
//! | [`momentum`] | `p = γmv`, its inverse, and the excess over `mv`. |
//! | [`dynamics`] | Force and acceleration, where `F = ma` becomes `γ³ma` along the motion and `γma` across it. |
//! | [`energy`] | `E = γmc²`, `KE = (γ−1)mc²`, and work — where `W = ΔKE` survives **unchanged**. |
//!
//! The last two mirror the parent layer's own split — [`conservation_laws`] holds momentum and
//! [`newtons_laws`] holds force, so [`momentum`] and [`dynamics`] sit in the same relationship here.
//!
//! [`conservation_laws`]: crate::physics_utils::conservation_laws
//! [`newtons_laws`]: crate::physics_utils::newtons_laws
//!
//! # The one sign convention, stated once
//!
//! Throughout, **`S` is "our" frame and `S'` moves at `frame_velocity_m_s` along `+x` as measured in
//! `S`**. A primed quantity is measured in `S'`. Every function names its arguments after that
//! convention rather than after the letters, because `u`, `u'`, and `v` are exactly the three symbols
//! that get swapped by accident.
//!
//! # Why `Option` rather than a benign fallback
//!
//! A speed at or above `c` for a *frame* is not a bad input to be shrugged off — it is a question with
//! no answer, the same category as
//! [`speed_from_displacement`](crate::physics_utils::kinematics::speed_from_displacement) past a
//! projectile's apex, so it returns `None` like that one does.
//!
//! This layer's usual `debug_assert!`-plus-benign-value contract is actively wrong here. The
//! candidate fallbacks for `γ` are `1.0`, which says "no relativistic effect" — the most misleading
//! answer available — and `f64::INFINITY`, which then silently poisons every arithmetic result
//! downstream into `NaN`. `None` is the only answer that a caller cannot mistake for a measurement.
//!
//! # What is deliberately not here yet
//!
//! No spacetime interval, no four-vectors, no relativistic collision solver — the parent layer's
//! `elastic_collision_1d` is Newtonian and has no counterpart here — no Doppler shift, no Thomas
//! precession, and nothing about **general** relativity — no gravitational time dilation, no
//! curvature, no perihelion precession. This module cannot correct Mercury's orbit and does not claim
//! to; that is GR, and the crate's Newtonian integrator is still Newtonian.
//!
//! Boosts are along a single axis only. A general boost in an arbitrary direction does not decompose
//! componentwise the way the rest of [`crate::physics_utils`] does — two perpendicular boosts compose
//! into a boost *plus a rotation* (Thomas–Wigner), so pretending otherwise would be a real physics
//! error rather than a simplification. When a second axis is needed the honest move is a `4×4`
//! transformation, not three calls to these functions.

pub mod constants;
pub mod dynamics;
pub mod energy;
pub mod length_contraction;
pub mod lorentz;
pub mod momentum;
pub mod time_dilation;
pub mod velocity;
