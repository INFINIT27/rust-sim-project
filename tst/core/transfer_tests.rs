//! Tests for [`crate::core::transfer`]: that a planned departure really reaches Mars, and that a
//! voyage flown through the live integrator — frame-sized steps, `f32` positions, corrections and
//! all — touches down on it.

use super::*;
use crate::core::scene::Color;
use crate::experiments::solar_system::solar_system;

const DEPARTURE_SPEED_M_S: f64 = 15_400.0;
const LAUNCH_ALTITUDE_M: f64 = 200_000.0;
/// One 60 Hz frame at the solar system's shipped clock of `1e6` simulated seconds per second.
const FRAME_S: f64 = 1.0e6 / 60.0;

/// The scene of `experiment` and the ids of the two named bodies in it.
fn pair(
    experiment: crate::experiments::catalog::Experiment,
    origin: &str,
    target: &str,
) -> (Scene, BodyId, BodyId) {
    let id = |name: &str| {
        experiment
            .names
            .0
            .iter()
            .find(|(_, named)| *named == name)
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("no body named {name}"))
    };
    let (origin, target) = (id(origin), id(target));
    (experiment.scene, origin, target)
}

fn earth_and_mars() -> (Scene, BodyId, BodyId) {
    pair(solar_system(), "Earth", "Mars")
}

/// Fly a voyage frame by frame through the live integrator the way the app does: a 60 Hz frame
/// at the solar system's clock, slowed so the flight lasts at least `MISSION_MIN_WATCH_S`, with a
/// correction whenever one falls due.
fn fly_voyage(scene: Scene, origin: BodyId, target: BodyId) -> (Scene, Voyage) {
    let plan = plan_departure(
        &scene,
        origin,
        target,
        DEPARTURE_SPEED_M_S,
        LAUNCH_ALTITUDE_M,
        &PlanProgress::default(),
    )
    .expect("a path exists");
    fly_plan(scene, origin, target, plan)
}

/// Fly `plan` frame by frame, as [`fly_voyage`] does.
fn fly_plan(
    mut scene: Scene,
    origin: BodyId,
    target: BodyId,
    plan: TransferPlan,
) -> (Scene, Voyage) {
    let frame_s =
        FRAME_S.min(plan.flight_seconds / (crate::render::constants::MISSION_MIN_WATCH_S * 60.0));
    let frame = TimeStep::from_seconds(frame_s).unwrap();
    let mut voyage = Voyage::launch(&mut scene, rocket(), origin, target, plan, 0.0);
    let mut now = 0.0;
    while voyage.touchdown_offset_units.is_none() && now < 1.3 * voyage.arrival_at_s {
        if voyage.next_correction_at().is_some_and(|at| now >= at) {
            let fix = plan_correction(
                &scene,
                voyage.craft,
                target,
                voyage.arrival_at_s - now,
                voyage.next_flyby(now).as_ref(),
            );
            voyage.apply_correction(&mut scene, fix, now);
        }
        scene.step_observed(frame, |scene| voyage.observe(scene));
        now += frame_s;
    }
    (scene, voyage)
}

fn rocket() -> Prop {
    Prop::new(
        Body::Sphere {
            pose: Pose {
                position: DVec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            radius: 1.0e-8,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        },
        DVec3::ZERO,
        722.0,
    )
}

#[test]
fn a_departure_plan_passes_through_the_targets_centre_at_its_chosen_speed_under_the_cap() {
    let (scene, earth, mars) = earth_and_mars();
    let plan = plan_departure(
        &scene,
        earth,
        mars,
        DEPARTURE_SPEED_M_S,
        LAUNCH_ALTITUDE_M,
        &PlanProgress::default(),
    )
    .expect("Mars is reachable at 15.4 km/s");

    assert!(
        plan.miss_m <= TRANSFER_AIM_TOLERANCE_M,
        "miss {} m",
        plan.miss_m
    );
    assert!(plan.flight_seconds > 0.0 && plan.flight_seconds.is_finite());

    // The speed relative to Earth at 200 km is what leaves 15.4 km/s once clear of Earth's pull.
    let earth_prop = scene.get(earth).unwrap();
    let relative = (plan.velocity_units_s - earth_prop.velocity_units_s).length() * METRES_PER_UNIT;
    let launch_radius = physical_radius_m(earth_prop) + LAUNCH_ALTITUDE_M;
    let mu = GRAVITATIONAL_CONSTANT * f64::from(earth_prop.mass_kg);
    let v_infinity = (relative * relative - 2.0 * mu / launch_radius).sqrt();
    assert!(
        (v_infinity - plan.departure_speed_m_s).abs() < 5.0
            && plan.departure_speed_m_s <= DEPARTURE_SPEED_M_S + 1.0e-9,
        "v∞ {v_infinity} m/s, chosen {} m/s, cap {DEPARTURE_SPEED_M_S}",
        plan.departure_speed_m_s
    );

    let last = *plan.path_units.last().unwrap();
    let first = plan.path_units[0];
    // The drawn path is `f32`, the launch point `f64`: at Earth's `150` units an `f32` resolves
    // `1.5e-5` units, so the path starts at the launch point to within that.
    assert!(
        (first.as_dvec3() - plan.position_units).length() < 2.0e-5,
        "the path starts at the launch point"
    );
    assert!(
        (last - first).length() > 50.0,
        "the path reaches out to Mars's orbit"
    );
}

fn assert_landed(scene: &Scene, voyage: &Voyage) {
    let offset = voyage
        .touchdown_offset_units
        .expect("the rocket reaches its target");
    let radius = physical_radius_m(scene.get(voyage.target).unwrap()) / METRES_PER_UNIT;
    assert!(
        (offset.length() - radius).abs() < 1.0e-3 * radius,
        "on the surface"
    );
    assert!(voyage.corrections > TRANSFER_CORRECTION_FRACTIONS.len());
    assert!(
        voyage.correction_delta_v_m_s < 0.1 * DEPARTURE_SPEED_M_S,
        "corrections are trims, not a second launch: {} m/s",
        voyage.correction_delta_v_m_s
    );
}

/// Flies the whole voyage, about 2,500 frames, through the live integrator. That takes about 13 s
/// in release and minutes in debug, so it is opt-in: `cargo test --release -- --ignored a_voyage`.
#[test]
#[ignore = "flies a 489-day voyage frame by frame; run with --release -- --ignored"]
fn a_voyage_from_earth_touches_down_on_mars() {
    let (scene, earth, mars) = earth_and_mars();
    let (scene, voyage) = fly_voyage(scene, earth, mars);
    assert_landed(&scene, &voyage);
}

#[test]
fn a_hop_from_earth_lands_on_the_moon_within_hours() {
    let (scene, earth, moon) = pair(solar_system(), "Earth", "Moon");
    let (scene, voyage) = fly_voyage(scene, earth, moon);
    assert!(voyage.arrival_at_s - voyage.launched_at_s < 86_400.0);
    assert_landed(&scene, &voyage);
}

#[test]
fn a_hop_between_two_of_jupiters_moons_lands() {
    let (scene, io, europa) = pair(crate::experiments::jovian::jovian_system(), "Io", "Europa");
    let (scene, voyage) = fly_voyage(scene, io, europa);
    assert_landed(&scene, &voyage);
}

#[test]
fn a_body_cannot_be_flown_to_itself() {
    let (scene, earth, _) = earth_and_mars();
    assert_eq!(
        plan_departure(
            &scene,
            earth,
            earth,
            DEPARTURE_SPEED_M_S,
            LAUNCH_ALTITUDE_M,
            &PlanProgress::default()
        ),
        Err(NoTransfer::InvalidRoute)
    );
}

#[test]
fn distant_moon_systems_fold_into_their_planet_and_conserve_mass_and_momentum() {
    let (mut scene, earth, mars) = earth_and_mars();
    let names = solar_system().names;
    let id = |name: &str| names.0.iter().find(|(_, n)| *n == name).unwrap().0;
    let (moon, jupiter, io) = (id("Moon"), id("Jupiter"), id("Io"));
    let mass = |scene: &Scene| {
        scene
            .props
            .iter()
            .map(|p| f64::from(p.mass_kg))
            .sum::<f64>()
    };
    let momentum = |scene: &Scene| {
        scene
            .props
            .iter()
            .map(|p| p.velocity_units_s * f64::from(p.mass_kg))
            .sum::<DVec3>()
    };
    let (mass_before, momentum_before) = (mass(&scene), momentum(&scene));
    let jupiter_mass = scene.get(jupiter).unwrap().mass_kg;

    fold_distant_moons(&mut scene, Some(earth), mars);

    assert!(
        scene.get(io).is_none(),
        "Jupiter's moons are folded on the way to Mars"
    );
    assert!(
        scene.get(moon).is_some(),
        "the Moon is kept: the rocket leaves through it"
    );
    assert!(scene.get(jupiter).unwrap().mass_kg > jupiter_mass);
    assert!((mass(&scene) - mass_before).abs() < 1.0e-6 * mass_before);
    // The system's total is near zero by construction (barycentric frame), so the drift is
    // measured against the largest single body's momentum rather than against the total.
    let largest = scene
        .props
        .iter()
        .map(|p| p.velocity_units_s.length() * f64::from(p.mass_kg))
        .fold(0.0, f64::max);
    let drift = (momentum(&scene) - momentum_before).length();
    assert!(
        drift < 1.0e-6 * largest,
        "momentum drift {drift} against {largest}"
    );
}

#[test]
fn a_hop_between_moons_keeps_their_siblings() {
    let experiment = crate::experiments::jovian::jovian_system();
    let names = experiment.names.clone();
    let (mut scene, io, europa) = pair(experiment, "Io", "Europa");
    let before = scene.props.len();
    fold_distant_moons(&mut scene, Some(io), europa);
    assert_eq!(scene.props.len(), before, "{:?}", names);
}

#[test]
fn a_segment_entering_a_sphere_is_caught_where_it_enters() {
    let entry = segment_entry(DVec3::new(-2.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0), 1.0).unwrap();
    assert!((entry - DVec3::new(-1.0, 0.0, 0.0)).length() < 1.0e-6);
}

#[test]
fn a_segment_passing_beside_a_sphere_or_stopping_short_is_not_an_entry() {
    assert_eq!(
        segment_entry(DVec3::new(-2.0, 1.5, 0.0), DVec3::new(2.0, 1.5, 0.0), 1.0),
        None
    );
    assert_eq!(
        segment_entry(DVec3::new(-3.0, 0.0, 0.0), DVec3::new(-2.0, 0.0, 0.0), 1.0),
        None
    );
}

#[test]
fn a_segment_starting_inside_is_caught_at_its_start() {
    let start = DVec3::new(0.5, 0.0, 0.0);
    assert_eq!(
        segment_entry(start, DVec3::new(3.0, 0.0, 0.0), 1.0),
        Some(start)
    );
}

#[test]
fn a_cancelled_search_stops_and_reports_no_plan() {
    let (scene, earth, mars) = earth_and_mars();
    let progress = PlanProgress::default();
    progress.cancel();
    let started = std::time::Instant::now();
    assert_eq!(
        plan_departure(
            &scene,
            earth,
            mars,
            DEPARTURE_SPEED_M_S,
            LAUNCH_ALTITUDE_M,
            &progress
        ),
        Err(NoTransfer::Cancelled)
    );
    assert!(
        started.elapsed().as_secs_f64() < 1.0,
        "cancelling returns at once"
    );
}

#[test]
fn the_search_reports_the_horizon_it_reached() {
    let (scene, earth, mars) = earth_and_mars();
    let progress = PlanProgress::default();
    let plan = plan_departure(
        &scene,
        earth,
        mars,
        DEPARTURE_SPEED_M_S,
        LAUNCH_ALTITUDE_M,
        &progress,
    )
    .unwrap();
    assert!(progress.horizon_s() >= plan.flight_seconds);
}

#[test]
fn flights_and_aims_stop_as_soon_as_their_search_is_cancelled() {
    // Deterministic stand-in for the bug the app showed: a cancel landing while the search was
    // inside its flights and Newton iterations waited `142 s` for them. Build a long ephemeris,
    // cancel, and check that a decade-long flight and an aim both give up at once.
    let (scene, earth, saturn) = pair(solar_system(), "Earth", "Saturn");
    let progress = PlanProgress::default();
    let ephemeris = Ephemeris::build(
        &scene,
        None,
        saturn,
        Some(earth),
        20.0 * 3.15576e7,
        &progress,
    )
    .unwrap();
    let start = to_metres(scene.get(earth).unwrap().body.center()) + DVec3::new(1.0e7, 0.0, 0.0);
    let velocity = to_metres(scene.get(earth).unwrap().velocity_units_s) * 1.3;
    progress.cancel();

    let started = std::time::Instant::now();
    let flight = fly(
        &ephemeris,
        start,
        velocity,
        ephemeris.target,
        (0.0, f64::INFINITY),
        f64::INFINITY,
        false,
    );
    assert!(
        flight.crashed,
        "a cancelled flight is reported as failed, so no aim can use it"
    );
    let launch = |p: &[f64; 2]| Some((start, velocity + DVec3::new(p[0], p[1], 0.0)));
    assert!(aim(
        &ephemeris,
        [0.0; 2],
        [1.0; 2],
        [10.0; 2],
        &Goal::centre(ephemeris.target, (0.0, f64::INFINITY)),
        None,
        &launch
    )
    .is_none());
    assert!(
        started.elapsed().as_secs_f64() < 0.1,
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn the_reach_bound_puts_jupiter_beyond_a_direct_flight_from_mercury() {
    // Mercury to Jupiter at Voyager 2's speed: the case that searched past a thousand years.
    let (scene, mercury, jupiter) = pair(solar_system(), "Mercury", "Jupiter");
    let reach = coasting_reach(&scene, mercury, jupiter, DEPARTURE_SPEED_M_S).unwrap();
    assert!(reach.target_closest_m > TRANSFER_REACH_MARGIN * reach.reach_m);
    const AU_M: f64 = 1.495_978_707e11;
    assert!(
        (reach.reach_m / AU_M - 3.05).abs() < 0.05,
        "reach {} AU",
        reach.reach_m / AU_M
    );
    assert!(
        (reach.target_closest_m / AU_M - 5.2).abs() < 0.1,
        "Jupiter {} AU",
        reach.target_closest_m / AU_M
    );
    assert!(
        (reach.needed_speed_m_s - 17_500.0).abs() < 200.0,
        "needs {} m/s",
        reach.needed_speed_m_s
    );
}

#[test]
fn reachable_and_inward_targets_pass_the_reach_check() {
    for (origin, target) in [
        ("Earth", "Mars"),
        ("Earth", "Jupiter"),
        ("Jupiter", "Mercury"),
    ] {
        let (scene, o, t) = pair(solar_system(), origin, target);
        let reach = coasting_reach(&scene, o, t, DEPARTURE_SPEED_M_S).unwrap();
        assert!(
            reach.target_closest_m <= TRANSFER_REACH_MARGIN * reach.reach_m,
            "{origin}→{target}: {reach:?}"
        );
    }
}

#[test]
fn bodies_orbiting_different_primaries_are_not_bounded() {
    let (scene, earth, moon) = pair(solar_system(), "Earth", "Moon");
    assert_eq!(
        coasting_reach(&scene, earth, moon, DEPARTURE_SPEED_M_S),
        None
    );
}

#[test]
fn a_gravity_assisted_voyage_flown_live_passes_its_planet_and_lands() {
    // Europa to Callisto through a pass of Ganymede, planned with the assist search alone and flown
    // frame by frame through the live integrator, correction burns around the flyby included.
    let experiment = crate::experiments::jovian::jovian_system();
    let names = experiment.names.clone();
    let (scene, europa, callisto) = pair(experiment, "Europa", "Callisto");
    let ganymede = names.0.iter().find(|(_, n)| *n == "Ganymede").unwrap().0;
    // Doubling horizons, as `plan_departure` searches, but with the direct search left out so the
    // assisted route is what comes back.
    let progress = PlanProgress::default();
    let plan = (0..10)
        .map(|k| 86_400.0 * 2.0_f64.powi(k))
        .find_map(|horizon| {
            assisted_departure(
                &scene,
                europa,
                callisto,
                DEPARTURE_SPEED_M_S,
                LAUNCH_ALTITUDE_M,
                horizon,
                horizon,
                &progress,
            )
        })
        .expect("a route through Ganymede");
    assert!(
        plan.flybys.iter().any(|f| f.body == ganymede),
        "the planned route passes Ganymede: {:?}",
        plan.flybys
    );

    let (scene, voyage) = fly_plan(scene, europa, callisto, plan);
    assert!(
        voyage.flybys.iter().any(|f| f.body == ganymede),
        "the flown route still passes Ganymede"
    );
    assert_landed(&scene, &voyage);
}

/// The whole search, assists included, out to `TRANSFER_ASSIST_ONLY_HORIZON_S`: about 150 s in
/// release, so opt-in.
#[test]
#[ignore = "runs the full 12-year assist search; about 150 s in release"]
fn a_target_out_of_reach_with_no_assist_found_is_reported_with_the_numbers() {
    let (scene, mercury, jupiter) = pair(solar_system(), "Mercury", "Jupiter");
    let result = plan_departure(
        &scene,
        mercury,
        jupiter,
        DEPARTURE_SPEED_M_S,
        LAUNCH_ALTITUDE_M,
        &PlanProgress::default(),
    );
    assert!(
        matches!(result, Err(NoTransfer::OutOfReach(_))),
        "{result:?}"
    );
}

/// The pass a correction found in the Europa → Ganymede → Callisto voyage, against the one it was
/// planned around: `30` hours late, approaching `59°` off, `12,491 km` out against `6,421 km`, yet
/// on the aim point within the plane perpendicular to the planned approach. Offsets and
/// velocities are the measured ones, rounded to the metre and the m/s.
fn planned_and_late_ganymede_passes() -> (
    crate::core::transfer::flight::AimPoint,
    crate::core::transfer::flight::Approach,
    crate::core::transfer::flight::Approach,
) {
    use crate::core::transfer::flight::{miss_basis, AimPoint, Approach};
    let planned_velocity = DVec3::new(8_683.89, 0.0, 6_013.39);
    let planned_offset = DVec3::new(-3_675_490.0, 470.0, 5_265_506.0);
    let basis = miss_basis(planned_velocity);
    let point = AimPoint {
        basis,
        offset_m: [planned_offset.dot(basis.0), planned_offset.dot(basis.1)],
        hold_direction: true,
    };
    let planned = Approach {
        time_s: 1_386_226.0,
        distance_m: planned_offset.length(),
        offset_m: planned_offset,
        relative_velocity_m_s: planned_velocity,
    };
    let late_offset = DVec3::new(-12_468_784.0, 477.0, -761_585.0);
    let late = Approach {
        time_s: 1_496_913.0,
        distance_m: late_offset.length(),
        offset_m: late_offset,
        relative_velocity_m_s: DVec3::new(-761.93, 0.0, 11_518.59),
    };
    (point, planned, late)
}

#[test]
fn a_correction_counts_the_planned_pass_as_on_its_aim_point() {
    let (point, planned, _) = planned_and_late_ganymede_passes();
    assert_eq!(point.off_plane_m(&planned), 0.0);
}

#[test]
fn a_correction_rejects_a_later_pass_that_approaches_from_another_direction() {
    // In the plane the aim point is compared in, the late pass sits within 51 km of the planned
    // point, so only its approach direction gives it away.
    let (point, _, late) = planned_and_late_ganymede_passes();
    let in_plane = [
        late.offset_m.dot(point.basis.0),
        late.offset_m.dot(point.basis.1),
    ];
    let across = (in_plane[0] - point.offset_m[0]).hypot(in_plane[1] - point.offset_m[1]);
    assert!(across < 60_000.0, "in-plane miss {across} m");

    let off = point.off_plane_m(&late);
    let ganymede_radius_m = 2_634_100.0;
    assert!(
        off > ganymede_radius_m,
        "a pass 59° off strays {off} m from the planned approach, beyond Ganymede's radius"
    );
}

#[test]
fn the_turn_limit_allows_a_slightly_bent_approach_and_rejects_the_late_pass() {
    // A pass bent by under the limit counts in full; the late pass, 59° off, is far outside it.
    let (point, planned, late) = planned_and_late_ganymede_passes();
    let turn = |approach: &crate::core::transfer::flight::Approach| {
        approach
            .relative_velocity_m_s
            .angle_between(planned.relative_velocity_m_s)
    };
    assert!(turn(&late) > 5.0 * TRANSFER_FLYBY_MAX_TURN_RAD);

    let bend = 0.5 * TRANSFER_FLYBY_MAX_TURN_RAD;
    let rotation = bevy::math::DQuat::from_rotation_y(bend);
    let bent = crate::core::transfer::flight::Approach {
        offset_m: rotation * planned.offset_m,
        relative_velocity_m_s: rotation * planned.relative_velocity_m_s,
        ..planned
    };
    assert!((turn(&bent) - bend).abs() < 1.0e-9);
    assert_eq!(point.off_plane_m(&bent), 0.0);
}

#[test]
fn the_direction_check_is_off_unless_asked_for() {
    // The assist search aims at new passes, not a planned one, so it must not be held to one.
    let (point, _, late) = planned_and_late_ganymede_passes();
    let free = crate::core::transfer::flight::AimPoint {
        hold_direction: false,
        ..point
    };
    assert_eq!(free.off_plane_m(&late), 0.0);
}
