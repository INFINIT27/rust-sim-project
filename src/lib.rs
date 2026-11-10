//! `physics-sandbox` library crate.
//!
//! The crate is split into layers, one per module directory:
//!
//! - [`core`] — the pure-Rust world model. May reference `bevy::math` and
//!   nothing else from Bevy (Requirements 4.2, 15.3).
//! - [`collision`] — body physics: the triangle hull that bounds each prop, the
//!   contact test over those hulls, and the elastic response. Bevy-free like
//!   [`core`].
//! - [`render`] — the boundary that mirrors the model onto Bevy's ECS
//!   ([`render::bridge`]) and mounts it into an `App` ([`render::plugin`]).
//! - [`input`] — the free-fly camera controller ([`input::camera`]).
//! - [`ui`] — the `bevy_egui` debug overlay ([`ui::overlay`]), the only
//!   module that imports egui.
//! - [`physics_utils`] — physical constants and closed-form formulas. Like
//!   [`core`], it is Bevy-free; unlike [`core`], it works in `f64`.
//! - [`time`] — the two clocks and the [`TimeStep`] every physics entry point
//!   takes. No Bevy import at all, not even `bevy::math`.
//! - [`experiments`] — the catalog of simulations, each one's settings, and the
//!   command-line selection that picks one. Bevy-free like [`core`].
//! - [`spacecraft`] — what a craft is (the [`Fleet`] `G` cycles), how it plans a flight
//!   between two bodies, how it flies it, and what it cost. Above [`core`], and like it
//!   Bevy-free but for `bevy::math`.
//!
//! Every public item is re-exported at the crate root below, so consumers
//! never name a layer: the module tree can be reshaped without breaking them.
//!
//! # Constants
//!
//! No constant is declared in an implementation file. Each layer keeps its own in
//! `constants.rs` ([`core::constants`], [`collision::constants`],
//! [`render::constants`], [`input::constants`], [`ui::constants`], [`config::constants`],
//! and [`physics_utils::physics_constants`]), and values read by more than one layer —
//! or by `main.rs`, which has no layer of its own — live in [`constants`]. Every
//! one documents why it holds that value, who reads it, and what changes if it
//! moves; the rule is `.claude/steering/constants.md`.

pub mod bindings;
pub mod collision;
pub mod config;
pub mod constants;
pub mod core;
pub mod experiments;
pub mod input;
pub mod performance;
pub mod physics_utils;
pub mod render;
pub mod spacecraft;
pub mod time;
pub mod ui;

// Time: the validated step every physics entry point takes, the clock that
// produces it, and the wall clock the camera and overlay run on instead.
pub use crate::time::constants::{
    DEFAULT_TIME_SCALE, MAX_REAL_STEP_SECONDS, MAX_TIME_SCALE, MIN_TIME_SCALE, SECONDS_PER_DAY,
    SECONDS_PER_HOUR, SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MINUTE,
};
pub use crate::time::observer::ObserverClock;
pub use crate::time::simulation::{clamp_real_step, SimulationClock, TimeStep};

// Experiments: the catalog, the per-experiment settings, the selection that picks
// one, and the seeded generator that randomises initial conditions.
pub use crate::experiments::catalog::{
    catalog_lines, ring_circular_speed_units_s, three_body_ring, two_body, BodyNames, Experiment,
    ExperimentId, ExperimentSpec,
};
pub use crate::experiments::random::Rng;

// The interstellar objects the `B` key can place, and the catalog they come from.
pub use crate::experiments::objects::{stellar_object, StellarObject};

// `experiment_5`: the static size-comparison row.
pub use crate::experiments::scale_check::{scale_check, scale_check_layout, scale_rows, ScaleRow};

// `experiment_4`: Jupiter and the Galilean moons.
pub use crate::experiments::constants::{
    DEFAULT_RANDOM_SEED, EARTH_MOONS, EARTH_RADIUS_M, JOVIAN_BOUNDS_MARGIN_UNITS,
    JOVIAN_CAMERA_POSITION_UNITS, JOVIAN_DISPLAY_EXAGGERATION, JOVIAN_FRAMED_RADIUS_UNITS,
    JOVIAN_MOONS, JOVIAN_SOFTENING_UNITS, JOVIAN_TIME_SCALE, JOVIAN_TRAIL_LENGTH_UNITS,
    MOON_SYSTEMS, SCALE_CHECK_CAMERA_OFFSET_UNITS, SCALE_CHECK_FRAMED_RADIUS_UNITS,
    SCALE_CHECK_GAP_UNITS, SCALE_CHECK_TIME_SCALE, SOLAR_SYSTEM_CAMERA_POSITION_UNITS,
    SOLAR_SYSTEM_ECLIPTIC_HEIGHT_UNITS, SOLAR_SYSTEM_FRAMED_RADIUS_UNITS, SOLAR_SYSTEM_PLANETS,
    SOLAR_SYSTEM_SOFTENING_UNITS, SOLAR_SYSTEM_SUN_COLOR, SOLAR_SYSTEM_SUN_MASS_KG,
    SOLAR_SYSTEM_SUN_RADIUS_M, SOLAR_SYSTEM_TIME_SCALE, SOLAR_SYSTEM_TRAIL_LENGTH_UNITS,
    STELLAR_OBJECTS, THREE_BODY_CAMERA_POSITION_UNITS, THREE_BODY_FRAMED_RADIUS_UNITS,
    THREE_BODY_HEIGHT_UNITS, THREE_BODY_MASS_KG, THREE_BODY_MAX_SPEED_FACTOR,
    THREE_BODY_MIN_SPEED_FACTOR, THREE_BODY_RING_RADIUS_UNITS, THREE_BODY_TIME_SCALE,
    THREE_BODY_VISUAL_RADIUS_UNITS, TWO_BODY_CAMERA_POSITION_UNITS, TWO_BODY_FRAMED_RADIUS_UNITS,
};
pub use crate::experiments::jovian::{
    jovian_drawn_radius_units, jovian_orbit_period_seconds, jovian_orbit_speed_units_s,
    jovian_primary, jovian_system,
};
pub use crate::experiments::selection::{help_text, parse_args, Command, ParseError, Selection};
pub use crate::experiments::solar_system::{
    circular_orbit_period_seconds, circular_orbit_speed_units_s, radius_units, solar_system,
    MoonSystem, Planet,
};

// Re-export the public scene API at the crate root so consumers can write
// `physics_sandbox::{Body, Scene, ...}` per the design's `Example Usage`.
pub use crate::core::scene::{
    moon_primary, Aabb, Body, BodyId, Color, DVec3, Pose, Prop, Quat, Scene, Vec3,
};

// Cross-layer constants: the unit scale, the shipped scene's configuration, and
// the window `main.rs` asks for.
pub use crate::constants::{
    EARTH_MASS_KG, KEY_BINDINGS, METRES_PER_UNIT, TWO_BODY_HEIGHT_UNITS, TWO_BODY_SEPARATION_UNITS,
    TWO_BODY_TIME_SCALE, TWO_BODY_VISUAL_RADIUS_UNITS, WINDOW_HEIGHT_PX, WINDOW_MODE, WINDOW_TITLE,
    WINDOW_WIDTH_PX,
};

// The `core` layer's own constants.
pub use crate::core::constants::{
    DEMO_BOUNDS_MAX_UNITS, DEMO_BOUNDS_MIN_UNITS, DEMO_FLOOR_COLOR, DEMO_FLOOR_HALF_SIZE_UNITS,
    GRAVITY_SOFTENING_UNITS, MOON_MAX_MASS_RATIO, ROTATION_UNIT_TOLERANCE,
    TRANSFER_AIM_TOLERANCE_M, TRANSFER_AZIMUTH_SAMPLES, TRANSFER_CANDIDATES,
    TRANSFER_CORRECTION_FRACTIONS, TRANSFER_EPHEMERIS_ORBIT_FRACTION, TRANSFER_EPHEMERIS_SAMPLES,
    TRANSFER_FIRST_CORRECTION_FRACTION, TRANSFER_FIRST_CORRECTION_S, TRANSFER_HORIZON_FACTOR,
    TRANSFER_LINE_SEARCH_HALVINGS, TRANSFER_MAX_ITERATIONS, TRANSFER_MIN_EPHEMERIS_STEP_S,
    TRANSFER_MIN_HORIZON_S, TRANSFER_MIN_STEP_S, TRANSFER_PATH_POINTS, TRANSFER_REACH_MARGIN,
    TRANSFER_SPEED_FRACTIONS, TRANSFER_SPEED_REFINE_FRACTION, TRANSFER_SPEED_REFINE_ROUNDS,
    TRANSFER_STEP_FRACTION, TWO_BODY_LEFT_COLOR, TWO_BODY_RIGHT_COLOR,
};

// The gravitation integrator that now fills the `Scene::step` seam.
pub use crate::core::integrator::{gravitational_acceleration, integrate_gravity};

// Planning a craft's path between two bodies under the scene's gravity, and flying it.
pub use crate::core::transfer::{
    coasting_reach, physical_radius_m, plan_correction, plan_departure, solve_coast,
    solve_coast_with, Coast, Flyby, Forecast, NoTransfer, PlanProgress, Reach, TransferPlan,
    Voyage,
};

// Spacecraft: what a craft is, the roster `G` cycles, planning and flying a flight, and its report.
pub use crate::input::mission::{launch_input_system, LaunchInputPlugin, LaunchRequest};
pub use crate::render::mission::{
    draw_mission_system, mission_system, pick_route_system, Mission, MissionPhase, MissionPlugin,
};
pub use crate::spacecraft::constants::{
    AEROJET_260_EXHAUST_SPEED_M_S, AEROJET_260_THRUST_N, BURN_DONE_M_S, CRAFT_COLOR,
    CRAFT_RADIUS_M, CREWED_CRAFT_MASS_KG, CREW_MAX_ACCELERATION_G, CRUISE_STEP_FRACTION,
    DEPARTURE_BURN_STEPS, LANDING_START_BRAKING_DISTANCES, LANDING_START_RADII, LAUNCH_ALTITUDE_M,
    ORBIT_SEARCH_CANDIDATES, ORBIT_SEARCH_DIRECTIONS, ORBIT_SEARCH_MAX_FRACTION,
    ORBIT_SEARCH_MIN_FRACTION, ORBIT_SEARCH_REFINE_STEPS, ORBIT_SEARCH_SAMPLES,
    ORBIT_SOLVE_WINDOW_FRACTION, POWERED_AIM_CLEARANCE_RADII, POWERED_BRAKE_FRACTION,
    POWERED_CANCEL_CHECK_STEPS, POWERED_CLEARANCE_RADII, POWERED_HORIZON_FACTOR,
    POWERED_MAX_SUBSTEP_S, POWERED_MIN_SUBSTEP_S, POWERED_SUBSTEP_FRACTION,
    POWERED_TOUCHDOWN_SPEED_M_S, PROBE_MASS_KG, PROBE_MAX_DEPARTURE_SPEED_M_S,
};
pub use crate::spacecraft::guidance::{
    blocking, braking_m_s2, match_velocity, segment_entry_m, steer, Law, Sighting, Steering,
};
pub use crate::spacecraft::lambert::lambert;
// Configuration files: `config/*.toml`, read once at startup into the resources above.
pub use crate::config::constants::{
    CAMERA_FILE, CONFIG_DIR_NAME, CONFIG_DIR_VAR, PERFORMANCE_FILE, SPACECRAFT_FILE, TRAILS_FILE,
    UI_FILE,
};
pub use crate::config::{config_dir, Config, ConfigError};

pub use crate::spacecraft::{
    central_body, departure_burn, plan_correction_for, plan_flight, plan_transfer_correction,
    surface_gravity_m_s2, Engine, Fleet, Flight, FlightPlan, FlightReport, NoFlight, Phase,
    Propulsion, Spacecraft,
};

// Body physics: the triangle hull that bounds a prop, the contact test over
// those hulls, and the elastic response that keeps props from phasing through
// one another.
pub use crate::collision::trimesh::{TriMesh, Triangle};

pub use crate::collision::tessellate::{collision_mesh, contact_mesh};

pub use crate::collision::contact::{
    aabbs_overlap, contact_between_meshes, contact_between_props, Contact,
};

pub use crate::collision::resolve::{resolve_contacts, substep_count};

// The second contact response: a black hole takes what it touches.
pub use crate::collision::absorb::absorb_contacts;

// The `collision` layer's tuning surface: hull shape, narrow phase, response.
pub use crate::collision::constants::{
    EDGE_AXIS_BUDGET, MAX_SUBSTEPS, MAX_SUBSTEP_DISPLACEMENT_UNITS, MIN_AXIS_LENGTH,
    NORMAL_TIE_TOLERANCE, PLANE_THICKNESS_UNITS, RESTITUTION, SPHERE_SUBDIVISIONS,
    SURFACE_EPSILON_UNITS,
};

// Re-export the render-bridge surface so consumers can write
// `physics_sandbox::transform_from(...)` without knowing its exact module
// path (keeps the public API stable even if we later split the bridge).
pub use crate::render::bridge::{sync_scene_to_ecs_system, transform_from};

// Analytic eclipse shadows: the material every body is drawn with, and the geometry behind it.
pub use crate::render::eclipse::{
    star_visible_fraction, star_visible_fraction_all, BodyMaterial, EclipseExtension,
    EclipseUniform,
};

// Performance: the quality-versus-frame-time switches, read from the environment, and the benchmark.
pub use crate::performance::{
    AntiAliasing, BenchConfig, PerformancePlugin, PerformanceSettings, SettingsError,
};

// Re-export the scene-plugin surface so consumers can write
// `physics_sandbox::{ScenePlugin, MirroredBody}` without
// depending on the exact module path.
pub use crate::render::plugin::{
    advance_clocks_system, clamp_dt, spawn_scene_entities, step_scene_system,
    sync_mirror_entities_system, update_eclipse_occluders_system, MirroredBody, ScenePlugin,
};

// Motion trails: the last few units of each body's path, fading out behind it.
pub use crate::render::trail::{
    apply_experiment_trail_length, draw_trails_system, record_trails_system, toggle_trails_system,
    TrailConfig, TrailGizmos, TrailPath, TrailPlugin, TrailSegment, Trails,
};

// The floor grid: a tracing aid drawn on the ground plane as render-only entities.
pub use crate::render::grid::{
    floor_grid_segments, floor_half_size, grid_core_extent_units, spawn_floor_grid, FloorGridLine,
    FloorGridPlugin, GridLayout, GridLineKind, GridSegment,
};

// The `render` layer's own constants: lighting, mesh proxies, and the grid. Camera
// framing is per experiment now, in `experiments::constants`.
pub use crate::render::constants::{
    BACKGROUND_COLOR, CAMERA_FAR_UNITS, CAMERA_NEAR_UNITS, ECLIPSE_SHADER_PATH,
    GRID_AXIS_THICKNESS_UNITS, GRID_AXIS_X_COLOR, GRID_AXIS_Z_COLOR, GRID_COARSE_SPACING_UNITS,
    GRID_CORE_EXTENT_UNITS, GRID_LIFT_UNITS, GRID_MAJOR_COLOR, GRID_MAJOR_EVERY,
    GRID_MAJOR_THICKNESS_UNITS, GRID_MINOR_COLOR, GRID_MINOR_THICKNESS_UNITS, GRID_SPACING_UNITS,
    LIGHT_POSITION_UNITS, LINE_VISUAL_THICKNESS_UNITS, MAX_ECLIPSE_OCCLUDERS,
    MISSION_MARKER_ANGULAR_RADIUS, MISSION_MIN_WATCH_S, MISSION_PLANNED_PATH_ALPHA,
    PLACEMENT_DISTANCE_UNITS, PLACEMENT_RADII_CLEARANCE, STAR_EMISSIVE_SCALE,
    STAR_LIGHT_RANGE_UNITS, STAR_LUMENS_PER_SOLAR_LUMINOSITY, TRAIL_HEAD_ALPHA, TRAIL_LENGTH_UNITS,
    TRAIL_LINE_WIDTH_PX, TRAIL_MAX_SAMPLES, TRAIL_MAX_TURN_RADIANS, TRAIL_MIN_SAMPLE_SPACING_UNITS,
    TRAIL_MIN_SPACING_FRACTION, TRAIL_MOON_SAMPLES_PER_ORBIT, TRAIL_TAIL_ALPHA,
    TRAIL_TARGET_SEGMENTS,
};

// Re-export the camera surface. `spawn_scene_entities` attaches
// `FreeFlyController` to the camera entity it spawns, so both halves of the
// contract are reachable from the crate root.
pub use crate::input::camera::{
    cycle_camera_speed_system, orbit_camera_system, toggle_camera_lock_system, CameraLock,
    CameraSettings, CameraSpeed, CameraTarget, FreeFlyCameraPlugin, FreeFlyController, Orbit,
};

// Keyboard control of the simulation clock: pause, single-step, speed.
pub use crate::input::controls::{simulation_controls_system, SimulationControlsPlugin};

// The menu's two halves: the input mode and the `Esc` binding here, the panel in `ui`.
pub use crate::input::menu::{menu_toggle_system, MenuInputPlugin, MenuState};
pub use crate::input::popup::PopupState;

// The object placer's three halves: state and keys here, the panel in `ui`, the spawn in
// `render`.
pub use crate::input::placement::{placement_input_system, PlacementInputPlugin, PlacementState};
pub use crate::render::placement::{place_object_system, placement_position, PlacementPlugin};
pub use crate::ui::placement::{
    placement_panel_system, placement_rows, PlacementPanelPlugin, PlacementRow,
};

// The key bindings as data, read by both `--help` and the menu.
pub use crate::bindings::{
    binding_help_text, bindings_in, key_column_width, BindingGroup, KeyBinding,
};

// The `input` layer's own constants: the camera's feel, and the speed-key factor.
pub use crate::input::constants::{
    CAMERA_BOOST_MULTIPLIER, CAMERA_MOVE_SPEED_UNITS_S, CAMERA_PRECISION_MULTIPLIER,
    LOOK_SENSITIVITY_RAD_PER_PX, PITCH_LIMIT_MARGIN_RAD, TIME_SCALE_DECADE_FACTOR,
    TIME_SCALE_KEY_FACTOR,
};

// Re-export the debug-overlay surface. `DebugUiPlugin` is the only type
// `main.rs` needs to reference, but we also re-export the system fn for
// tests that want to inspect it directly.
pub use crate::ui::overlay::{
    camera_lock_line, camera_speed_line, clock_lines, debug_overlay_system, distance_lines,
    experiment_line, human_duration, mission_overlay_system, prop_lines, units_to_metres,
    DebugUiPlugin,
};

// The `ui` layer's own constants: overlay row budgets and crosshair appearance.
pub use crate::ui::arrival::{
    arrival_popup_system, arrival_rows, duration_label, energy_label, ArrivalPopupConfig,
    ArrivalPopupPlugin,
};
pub use crate::ui::constants::{
    ARRIVAL_LABEL_COLUMN_PX, ARRIVAL_POPUP_WIDTH_PX, COMPASS_DIM_ALPHA, COMPASS_LABEL_SIZE_PX,
    COMPASS_MARGIN_PX, COMPASS_RADIUS_PX, COMPASS_THICKNESS_PX, COMPASS_X_COLOR_SRGBA,
    COMPASS_Y_COLOR_SRGBA, COMPASS_Z_COLOR_SRGBA, CROSSHAIR_ARM_LENGTH_PX, CROSSHAIR_COLOR_SRGBA,
    CROSSHAIR_GAP_PX, CROSSHAIR_THICKNESS_PX, JOULES_PER_TONNE_OF_TNT, MAX_DISTANCE_ROWS,
    MAX_PROP_ROWS, MENU_KEY_COLUMN_PX, MENU_SECTION_GAP_PX, MENU_WIDTH_PX,
    PLACEMENT_PANEL_MARGIN_PX, PLACEMENT_PANEL_WIDTH_PX,
};

// Re-export the compass surface. `compass_axes` is public so the projection is
// property-testable independently of egui.
pub use crate::ui::menu::{binding_rows, menu_system, settings_rows, MenuPanelPlugin, SettingsRow};

pub use crate::ui::compass::{
    compass_axes, compass_system, CompassAxis, CompassConfig, CompassPlugin,
};

// Re-export the crosshair surface. `crosshair_segments` is public so the
// geometry is reusable (and property-testable) independently of egui.
pub use crate::ui::crosshair::{
    crosshair_segments, crosshair_system, CrosshairConfig, CrosshairPlugin,
};

// Re-export the physics surface so consumers can write
// `physics_sandbox::gravitational_constant_calculator(..)` and
// `physics_sandbox::GRAVITATIONAL_CONSTANT` without naming the layer.
pub use crate::physics_utils::physics_constants::{GRAVITATIONAL_CONSTANT, STANDARD_GRAVITY_M_S2};
pub use crate::physics_utils::physics_formulas::gravitational_constant_calculator;

// Newton's three laws of motion.
pub use crate::physics_utils::newtons_laws::{
    acceleration_from_net_force, inertial_position, net_force, reaction_force,
};

// Kinematics: the constant-acceleration relations and the state that carries
// position, velocity, and acceleration together.
pub use crate::physics_utils::kinematics::{
    average_acceleration, average_velocity, displacement, position_after, speed_from_displacement,
    time_to_reach_velocity, velocity_after, MotionState,
};

// Special relativity: the sub-layer's whole public surface. Flattened like the rest
// of `physics_utils`, so a caller writes `physics_sandbox::lorentz_factor(v)` — but note
// these all return `Option`, unlike every other physics function in the crate. See
// `physics_utils/relativity/README.md` for why `None` beats a benign fallback here.
pub use crate::physics_utils::relativity::constants::SPEED_OF_LIGHT_M_S;
pub use crate::physics_utils::relativity::dynamics::{
    longitudinal_acceleration, longitudinal_force, transverse_acceleration, transverse_force,
};
pub use crate::physics_utils::relativity::length_contraction::{
    contracted_length, length_contraction_deficit, proper_length,
};
pub use crate::physics_utils::relativity::lorentz::{
    beta_from_velocity, is_subluminal, lorentz_factor, lorentz_factor_minus_one,
};
pub use crate::physics_utils::relativity::momentum::{
    momentum_excess, relativistic_momentum, velocity_from_momentum,
};
pub use crate::physics_utils::relativity::time_dilation::{
    clock_lag_seconds, dilated_time, proper_time,
};
pub use crate::physics_utils::relativity::velocity::{compose_velocities, transform_velocity};
// Note `relativistic_kinetic_energy` rather than `kinetic_energy`: the parent layer's Newtonian
// `½mv²` is re-exported under that name, and a caller should have to say which one they mean.
pub use crate::physics_utils::relativity::energy::{
    relativistic_kinetic_energy, rest_energy, total_energy, velocity_after_work, work_done,
};

// Conservation of momentum and energy, plus the collision solvers that follow
// from them.
pub use crate::physics_utils::conservation_laws::{
    elastic_collision_1d, energy_is_conserved, gravitational_potential_energy, kinetic_energy,
    kinetic_energy_is_conserved, momentum, momentum_is_conserved, perfectly_inelastic_collision_1d,
    total_kinetic_energy, total_mechanical_energy, total_momentum,
};
