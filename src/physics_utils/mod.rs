//! Physics utilities: constants and closed-form formulas.
//!
//! Like [`crate::core`], this layer is GUI-free — it imports nothing from
//! Bevy, egui, or wgpu — so every item here is unit- and property-testable
//! with no window, no GPU, and no ECS. It is the intended home for the math
//! that will eventually be driven from
//! [`crate::core::scene::Scene::step`], which is still a no-op seam.
//!
//! # Why `f64` here when the rest of the crate is `f32`
//!
//! The render side uses `f32` because that is what glam and the GPU want.
//! Gravitation does not fit in `f32`: the constant is `~6.7e-11` while
//! astronomical masses reach `~1e30 kg`, and the product of two such masses
//! (`~1e55`) overflows `f32`, whose maximum is `~3.4e38`. Every value in this
//! module is therefore `f64`, and callers convert at the render boundary.

//! # What is where
//!
//! - [`physics_constants`] — named physical constants with their units and
//!   sources.
//! - [`physics_formulas`] — gravitation.
//! - [`newtons_laws`] — Newton's three laws of motion.
//! - [`conservation_laws`] — momentum and energy: the quantities, the
//!   conservation checks, and the collision solvers that follow from them.
//! - [`kinematics`] — position, velocity, acceleration, and time under constant
//!   acceleration, plus [`kinematics::MotionState`] to carry the three together.
//! - [`relativity`] — special relativity: `β` and `γ`, time dilation, length contraction, and the
//!   velocity transformation between two inertial frames. The one sub-directory here, because it is
//!   the one topic with its own constant (`c`), its own domain rule (`|v| < c`), and its own error
//!   convention (`Option`, not a benign fallback).
//!
//! Everything is scalar `f64` along a single caller-chosen axis, with sign
//! carrying direction. A caller working in 3-D applies these componentwise, and
//! supplies the geometry itself.

pub mod conservation_laws;
pub mod kinematics;
pub mod newtons_laws;
pub mod physics_constants;
pub mod physics_formulas;
pub mod relativity;
