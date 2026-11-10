//! Relativity: special relativity in full, and the closed-form results of general relativity.
//!
//! The first departure in this crate from Newtonian mechanics. Special relativity follows from two
//! postulates — the laws of physics are the same in every inertial frame, and `c` is the same in
//! every inertial frame. [`gravity`] is the one module about general relativity.
//!
//! Nothing in the simulation calls this sub-layer. It is a library of formulas with tests.
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
//! | [`spacetime`] | Events: the Lorentz transformation, the interval, and the relativity of simultaneity. |
//! | [`rapidity`] | `φ = atanh β`, which adds where velocity does not, and motion under constant proper acceleration. |
//! | [`doppler`] | Frequency shift of light from a moving source, along the line of sight and at any angle. |
//! | [`energy_momentum`] | `E² = (pc)² + (mc²)²`, invariant mass, light (`m = 0`), and mass–energy conversion. |
//! | [`collisions`] | The centre-of-momentum frame, and elastic and perfectly inelastic collisions on one axis. |
//! | [`velocity_3d`] | Velocities in any direction, the Wigner rotation, and aberration. |
//! | [`four_vector`] | `FourVector`, the Minkowski product, and the boost in any direction. |
//! | [`gravity`] | General relativity around one mass: clock rates, redshift, perihelion precession, light bending. |
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
//! # One axis, and when that is not enough
//!
//! Most modules here are along a single axis. A boost in an arbitrary direction does not decompose
//! componentwise the way the rest of [`crate::physics_utils`] does — two perpendicular boosts compose
//! into a boost *plus a rotation* (Thomas–Wigner), so calling the one-axis functions per component
//! is a physics error. For a second axis use [`velocity_3d`] and [`four_vector`], which are written
//! for any direction.
//!
//! # What is not here
//!
//! - **A solver for general relativity.** [`gravity`] is closed-form results around one spherical,
//!   non-rotating mass: no geodesics, no rotating masses, no gravitational waves. The crate's
//!   integrator is still Newtonian, so a simulated Mercury does not precess; the module only says by
//!   how much it should.
//! - **Collisions beyond one axis and two bodies**, and collisions involving light.
//! - **Orientation under boosts**: no rotation matrices, no Thomas precession rate.
//! - **Electromagnetism**, and anything that draws what a fast observer would see.
//!
//! The original eight modules are re-exported from the crate root; the later ones are reached by
//! path, for example `physics_utils::relativity::doppler::redshift`.

pub mod collisions;
pub mod constants;
pub mod doppler;
pub mod dynamics;
pub mod energy;
pub mod energy_momentum;
pub mod four_vector;
pub mod gravity;
pub mod length_contraction;
pub mod lorentz;
pub mod momentum;
pub mod rapidity;
pub mod spacetime;
pub mod time_dilation;
pub mod velocity;
pub mod velocity_3d;
