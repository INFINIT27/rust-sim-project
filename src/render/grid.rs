//! A tracing grid drawn on the floor.
//!
//! Watching a body move against a featureless plane tells you almost nothing: without a
//! reference you cannot see whether an orbit is drifting, whether a bounce came back to
//! where it started, or how far apart two bodies are. The grid is that reference.
//!
//! # It is scenery, not physics
//!
//! Every line here is a **render-only entity**. Nothing is added to the [`Scene`], which
//! matters for three reasons:
//!
//! - `Scene.props` stays the list of things being simulated, so the overlay's prop count
//!   and per-prop readout keep meaning what they say — 42 grid lines would drown them.
//! - Gravitation and contact are `O(n²)` over props. Scenery in that loop would cost real
//!   time to compute nothing: the lines have no mass, and a `Body::Line` of zero width has
//!   no hull anyway.
//! - The model stays the model. `Scene` is what a physics engine drives; a visual aid in
//!   it would be a category error, and the first thing to confuse someone reading the
//!   scene's contents.
//!
//! The crosshair takes the same stance in [`crate::ui::crosshair`] — an observer aid,
//! outside the model.
//!
//! # It covers the whole floor, at two spacings
//!
//! [`spawn_floor_grid`] reads the floor prop's own `half_size` out of the `Scene`, so the grid
//! always covers exactly the surface that is there — including the shipped floor, which is
//! `2000` units to the side and reads as unbounded.
//!
//! Covering that at a single `5`-unit spacing would be 1,602 line entities whose distant half
//! resolves to a grey moiré, finer than the pixels available to draw it. So [`GridLayout`]
//! splits the job into three tiers, and the split is the design:
//!
//! | Tier | Spacing | Extent | Job |
//! | --- | --- | --- | --- |
//! | **Axes** | — | the whole floor | Where the origin is, from anywhere |
//! | **Coarse** | [`GRID_COARSE_SPACING_UNITS`] (100) | the whole floor | The plane is never bare |
//! | **Fine** | [`GRID_SPACING_UNITS`] (5) | [`GRID_CORE_EXTENT_UNITS`] (60) | Measuring the motion that actually happens |
//!
//! That is ~130 entities for a floor a fine grid would need 1,602 for, and it puts the
//! resolution where the bodies are: the `±50` bounds get a 5-unit rule, and everything out to
//! the edge gets a 100-unit one. The coarse spacing is a whole multiple of the fine one, so
//! the tiers line up rather than interleaving into a beat pattern.
//!
//! A *smaller* floor than the core extent still gets a fine grid that matches it exactly — see
//! [`grid_core_extent_units`] — because a grid overhanging its own surface looks broken.
//!
//! # Reading it
//!
//! | Line | Spacing | Meaning |
//! | --- | --- | --- |
//! | Axis | through the origin | The `X` and `Z` axes, each in its own colour |
//! | Major | every `GRID_MAJOR_EVERY × GRID_SPACING_UNITS` units | Coarse scale for judging distance |
//! | Minor | every [`GRID_SPACING_UNITS`] units | Fine scale for judging motion |
//!
//! Lines are lifted [`GRID_LIFT_UNITS`] above the surface. Drawing them *at* the surface
//! would put two coplanar surfaces in the depth buffer, and the resulting z-fighting
//! flickers as the camera moves — the classic decal problem, and the reason the lift is
//! not an aesthetic choice.

use bevy::app::{App, Plugin, Startup};
use bevy::asset::{Assets, Handle};
use bevy::ecs::component::Component;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::math::primitives::Cuboid;
use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::render::mesh::Mesh;
use bevy::transform::components::Transform;

use crate::core::scene::{Body, Color, Scene};
use crate::render::constants::{
    GRID_AXIS_THICKNESS_UNITS, GRID_AXIS_X_COLOR, GRID_AXIS_Z_COLOR, GRID_COARSE_SPACING_UNITS,
    GRID_CORE_EXTENT_UNITS, GRID_LIFT_UNITS, GRID_MAJOR_COLOR, GRID_MAJOR_EVERY,
    GRID_MAJOR_THICKNESS_UNITS, GRID_MINOR_COLOR, GRID_MINOR_THICKNESS_UNITS, GRID_SPACING_UNITS,
};

/// Marks an entity as part of the floor grid.
///
/// Deliberately *not* [`MirroredBody`](crate::render::plugin::MirroredBody): a grid line
/// mirrors nothing in the `Scene`, and the mirror invariant
/// ("`count_entities_with::<MirroredBody>() == scene.props.len()`") must keep holding. A
/// separate marker is also what a future "hide the grid" toggle would query.
#[derive(Component, Copy, Clone, Debug, Eq, PartialEq)]
pub struct FloorGridLine;

/// Which kind of grid line this is, which sets its colour and thickness.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GridLineKind {
    /// Every [`GRID_SPACING_UNITS`]: the fine scale, for seeing that something moved.
    Minor,
    /// Every `GRID_MAJOR_EVERY` minor lines: the coarse scale, for judging how far.
    Major,
    /// Through the origin — the `X` axis (running along X) or the `Z` axis.
    Axis {
        /// `true` for the line *along* the X axis, `false` for the one along Z.
        along_x: bool,
    },
}

impl GridLineKind {
    /// The line's colour, in linear RGBA.
    #[inline]
    pub fn color(&self) -> Color {
        match self {
            Self::Minor => GRID_MINOR_COLOR,
            Self::Major => GRID_MAJOR_COLOR,
            Self::Axis { along_x: true } => GRID_AXIS_X_COLOR,
            Self::Axis { along_x: false } => GRID_AXIS_Z_COLOR,
        }
    }

    /// The line's cross-section, in world units.
    #[inline]
    pub fn thickness_units(&self) -> f32 {
        match self {
            Self::Minor => GRID_MINOR_THICKNESS_UNITS,
            Self::Major => GRID_MAJOR_THICKNESS_UNITS,
            Self::Axis { .. } => GRID_AXIS_THICKNESS_UNITS,
        }
    }
}

/// One line of the grid: two endpoints and what kind of line it is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GridSegment {
    /// One end, in world units.
    pub from: Vec3,
    /// The other end, in world units.
    pub to: Vec3,
    /// Minor, major, or an axis.
    pub kind: GridLineKind,
}

impl GridSegment {
    /// The segment's length in world units.
    #[inline]
    pub fn length_units(&self) -> f32 {
        (self.to - self.from).length()
    }

    /// The midpoint, which is where the rendered rod's transform is placed.
    #[inline]
    pub fn midpoint(&self) -> Vec3 {
        0.5 * (self.from + self.to)
    }
}

/// The three tiers a grid is built from, and where each one reaches.
///
/// A plain struct rather than five positional arguments: the two extents and the two spacings
/// are easy to transpose, and `GridLayout { .. }` at the call site says which is which. Also
/// what makes the geometry testable at any shape without a floor to read it from.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GridLayout {
    /// How far the **fine** lines reach from the origin, in world units.
    pub core_extent_units: f32,
    /// Spacing of the fine lines, in world units.
    pub fine_spacing_units: f32,
    /// How far the **axes and coarse** lines reach — normally the floor's half-size, so the
    /// plane is gridded to its edge.
    pub full_extent_units: f32,
    /// Spacing of the coarse lines, in world units. Should be a whole multiple of
    /// [`fine_spacing_units`](Self::fine_spacing_units) so the tiers line up.
    pub coarse_spacing_units: f32,
    /// The floor's surface height, in world units. Lines are drawn [`GRID_LIFT_UNITS`] above it.
    pub surface_y: f32,
}

impl GridLayout {
    /// The shipped layout for a floor of `floor_half_size_units` with its surface at
    /// `surface_y`.
    ///
    /// Axes and coarse lines span the **whole** floor; fine lines reach
    /// [`grid_core_extent_units`] — which is the floor's half-size when that is smaller than
    /// [`GRID_CORE_EXTENT_UNITS`], so a modest floor gets a fully fine grid and only a vast one
    /// is tiered.
    pub fn for_floor(floor_half_size_units: f32, surface_y: f32) -> Self {
        Self {
            core_extent_units: grid_core_extent_units(floor_half_size_units),
            fine_spacing_units: GRID_SPACING_UNITS,
            full_extent_units: floor_half_size_units,
            coarse_spacing_units: GRID_COARSE_SPACING_UNITS,
            surface_y,
        }
    }

    /// Validation_Rules for `GridLayout`: both extents and both spacings finite and strictly
    /// positive, the core extent no larger than the full one, and `surface_y` finite.
    ///
    /// A core extent beyond the full extent would put fine lines outside the floor, which is
    /// the one arrangement that looks like a bug rather than a choice.
    pub fn is_valid(&self) -> bool {
        [
            self.core_extent_units,
            self.fine_spacing_units,
            self.full_extent_units,
            self.coarse_spacing_units,
        ]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
            && self.core_extent_units <= self.full_extent_units
            && self.surface_y.is_finite()
    }
}

/// How far the **fine** grid should reach for a floor of `floor_half_size_units`.
///
/// The smaller of the floor's own half-size and [`GRID_CORE_EXTENT_UNITS`]. Two behaviours in
/// one line, and both matter:
///
/// - a floor **smaller** than the cap gets fine lines that match it exactly, so the grid never
///   overhangs the surface it is painted on;
/// - a floor **larger** than the cap gets fine lines over the region that matters, and coarse
///   lines the rest of the way — rather than 1,602 entities of grey moiré.
///
/// Public because `grid_tests` needs the same number `spawn_floor_grid` uses; a test that
/// recomputed the cap itself would pass while the two disagreed.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{grid_core_extent_units, GRID_CORE_EXTENT_UNITS};
///
/// // A small floor is matched exactly.
/// assert_eq!(grid_core_extent_units(20.0), 20.0);
///
/// // A vast one gets fine lines only where they can be read.
/// assert_eq!(grid_core_extent_units(2000.0), GRID_CORE_EXTENT_UNITS);
/// ```
#[inline]
pub fn grid_core_extent_units(floor_half_size_units: f32) -> f32 {
    floor_half_size_units.min(GRID_CORE_EXTENT_UNITS)
}

/// Every line of the grid described by `layout`.
///
/// Pure and `Vec`-returning so the geometry is testable without a window, a GPU, or egui — the
/// same split [`crosshair_segments`](crate::ui::crosshair::crosshair_segments) uses.
///
/// # What it emits
///
/// Three tiers, in this order — which is also back-to-front for drawing, so a fine line lands
/// on top of a coarse one where they meet:
///
/// 1. **Two axis lines** through the origin, spanning `±full_extent_units`, one per direction.
/// 2. **Coarse lines** every `coarse_spacing_units` out to `±full_extent_units`, skipping the
///    origin (the axes are already there) — so the plane is gridded to its edge.
/// 3. **Fine lines** every `fine_spacing_units` out to `±core_extent_units`, skipping the
///    origin *and* any offset a coarse line already occupies, so no two lines are ever drawn on
///    top of each other. Every `GRID_MAJOR_EVERY`th one is a major line.
///
/// # Returns
///
/// Empty when `layout` fails [`GridLayout::is_valid`], or when a spacing is wider than the
/// extent it applies to: those describe no grid, and returning nothing beats returning a single
/// line through the middle that looks like a bug.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{floor_grid_segments, GridLayout, GridLineKind};
///
/// // Fine lines every 5 units out to 20, coarse every 40 out to 200.
/// let layout = GridLayout {
///     core_extent_units: 20.0,
///     fine_spacing_units: 5.0,
///     full_extent_units: 200.0,
///     coarse_spacing_units: 40.0,
///     surface_y: 0.0,
/// };
/// let segments = floor_grid_segments(layout);
///
/// // Exactly two axes, and they span the full extent rather than the core.
/// let axes: Vec<_> = segments
///     .iter()
///     .filter(|segment| matches!(segment.kind, GridLineKind::Axis { .. }))
///     .collect();
/// assert_eq!(axes.len(), 2);
/// assert!((axes[0].length_units() - 400.0).abs() < 1.0e-3);
/// ```
pub fn floor_grid_segments(layout: GridLayout) -> Vec<GridSegment> {
    debug_assert!(
        layout.is_valid(),
        "floor_grid_segments: layout fails Validation_Rules: {:?}",
        layout
    );

    if !layout.is_valid() {
        return Vec::new();
    }

    let y = layout.surface_y + GRID_LIFT_UNITS;
    let mut segments = Vec::new();

    // A line in each direction at `offset`, spanning `±half_length`.
    let mut push_pair = |offset: f32, half_length: f32, kind: GridLineKind| {
        let along_z = match kind {
            // The two axis lines are distinguished so each gets its own colour.
            GridLineKind::Axis { .. } => GridLineKind::Axis { along_x: false },
            other => other,
        };

        segments.push(GridSegment {
            from: Vec3::new(-half_length, y, offset),
            to: Vec3::new(half_length, y, offset),
            kind,
        });
        segments.push(GridSegment {
            from: Vec3::new(offset, y, -half_length),
            to: Vec3::new(offset, y, half_length),
            kind: along_z,
        });
    };

    // 1. The axes, spanning the whole floor: the origin has to be findable from the far end
    //    of the plane, which is the one thing a local fine grid cannot tell you.
    push_pair(
        0.0,
        layout.full_extent_units,
        GridLineKind::Axis { along_x: true },
    );

    // 2. Coarse lines to the edge. This is what stops the plane looking bare once the floor
    //    became large enough to read as unbounded.
    if layout.coarse_spacing_units <= layout.full_extent_units {
        let steps = (layout.full_extent_units / layout.coarse_spacing_units).floor() as i32;
        for step in -steps..=steps {
            if step == 0 {
                continue; // the axes already cover the origin
            }
            push_pair(
                step as f32 * layout.coarse_spacing_units,
                layout.full_extent_units,
                GridLineKind::Major,
            );
        }
    }

    // 3. Fine lines over the core, skipping any offset a coarse line already occupies so
    //    nothing is drawn twice — two coincident lines read as one thicker line, which makes
    //    the spacing look irregular.
    if layout.fine_spacing_units <= layout.core_extent_units {
        let steps = (layout.core_extent_units / layout.fine_spacing_units).floor() as i32;
        let coarse_per_fine = layout.coarse_spacing_units / layout.fine_spacing_units;

        for step in -steps..=steps {
            if step == 0 {
                continue;
            }

            // Integer arithmetic where the spacings divide evenly, which they should; the
            // remainder test is the general case for when they do not.
            let on_coarse = ((step as f32) % coarse_per_fine).abs() < 1.0e-4;
            if on_coarse {
                continue;
            }

            let kind = if step % GRID_MAJOR_EVERY as i32 == 0 {
                GridLineKind::Major
            } else {
                GridLineKind::Minor
            };

            push_pair(
                step as f32 * layout.fine_spacing_units,
                layout.core_extent_units,
                kind,
            );
        }
    }

    segments
}

/// The half-size of the scene's ground plane, if it has one.
///
/// Takes the **first** `Body::Plane` in insertion order. Every experiment starts from
/// `Scene::demo`, which adds the floor first, so that is the floor. Returns `None` for a
/// scene with no plane at all, in which case there is nothing to draw a grid on and
/// [`spawn_floor_grid`] does nothing rather than inventing a size.
pub fn floor_half_size(scene: &Scene) -> Option<(f32, f32)> {
    scene.props.iter().find_map(|prop| match &prop.body {
        Body::Plane {
            pose, half_size, ..
        } => Some((*half_size, pose.position.y as f32)),
        _ => None,
    })
}

/// Startup system that spawns the grid as thin unlit rods on the floor.
///
/// One shared cuboid mesh and one material per [`GridLineKind`], so a 42-line grid costs
/// one mesh and four materials rather than 42 of each. Each line is that unit cube scaled
/// to `(length, thickness, thickness)` — the same rod trick
/// [`spawn_scene_entities`](crate::render::plugin::spawn_scene_entities) uses for a
/// `Body::Line`.
///
/// The materials are **unlit**: a grid is a measuring aid, and one whose visibility
/// depended on the angle of the scene's directional light would be useless exactly where
/// the light does not reach.
///
/// Does nothing when the scene has no ground plane, or when the floor is smaller than the
/// grid spacing.
pub fn spawn_floor_grid(
    mut commands: Commands,
    scene: Res<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some((half_size, surface_y)) = floor_half_size(&scene) else {
        return;
    };

    let segments = floor_grid_segments(GridLayout::for_floor(half_size, surface_y));
    if segments.is_empty() {
        return;
    }

    // A unit cube, scaled per line. Shared, so the vertex data exists once.
    let rod: Handle<Mesh> = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

    // One material per kind, built on demand: four at most, and a scene with no major
    // lines does not allocate one.
    let material_for = |kind: GridLineKind, materials: &mut Assets<StandardMaterial>| {
        let color = kind.color();
        materials.add(StandardMaterial {
            base_color: bevy::color::Color::linear_rgba(color.r, color.g, color.b, color.a),
            unlit: true,
            ..Default::default()
        })
    };

    let minor = material_for(GridLineKind::Minor, &mut materials);
    let major = material_for(GridLineKind::Major, &mut materials);
    let axis_x = material_for(GridLineKind::Axis { along_x: true }, &mut materials);
    let axis_z = material_for(GridLineKind::Axis { along_x: false }, &mut materials);

    for segment in segments {
        let material = match segment.kind {
            GridLineKind::Minor => minor.clone(),
            GridLineKind::Major => major.clone(),
            GridLineKind::Axis { along_x: true } => axis_x.clone(),
            GridLineKind::Axis { along_x: false } => axis_z.clone(),
        };

        // Which way the rod runs decides which axis takes its length. Both grid
        // directions are axis-aligned, so no rotation is needed — a `looking_at` here
        // would be a degenerate transform for a zero-length segment.
        let along = segment.to - segment.from;
        let thickness = segment.kind.thickness_units();
        let scale = if along.x.abs() > along.z.abs() {
            Vec3::new(segment.length_units(), thickness, thickness)
        } else {
            Vec3::new(thickness, thickness, segment.length_units())
        };

        commands.spawn((
            PbrBundle {
                mesh: rod.clone(),
                material,
                transform: Transform {
                    translation: segment.midpoint(),
                    scale,
                    ..Default::default()
                },
                ..Default::default()
            },
            FloorGridLine,
        ));
    }
}

/// Plugin that draws the tracing grid on the floor.
///
/// Add it after [`ScenePlugin`](crate::render::plugin::ScenePlugin), which is what inserts
/// the `Scene` this reads. It registers one `Startup` system and nothing per frame: the
/// grid never moves, so there is no reason to touch it again.
pub struct FloorGridPlugin;

impl Plugin for FloorGridPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_floor_grid);
    }
}

#[cfg(test)]
#[path = "../../tst/render/grid_tests.rs"]
mod tests;
