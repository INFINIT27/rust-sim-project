//! Unit tests for the [`crate::experiments::objects`] module and the
//! [`STELLAR_OBJECTS`](crate::experiments::constants::STELLAR_OBJECTS) table.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/experiments/objects.rs` via
//! `#[path]`, not as a standalone integration test.
//!
//! # What these tests are for
//!
//! The catalog makes two kinds of claim and they need two kinds of test. The **masses and
//! radii** are measurements, so they are checked against published figures in the units they
//! are published in — a mistyped exponent produces a self-consistent scene with wrong
//! physics, which nothing else here would catch. The **drawn radii** are decisions, and the
//! decision is different for each of the three regimes in the table, so each regime is pinned
//! *as* a regime: to-scale for the dwarfs, floored for the two compact objects, true-size for
//! the supermassive black hole.
//!
//! That second group is the important one. The whole risk with this table is that a floor
//! chosen deliberately comes to look like a scale everyone assumes is real.

use super::*;
use crate::experiments::constants::STELLAR_OBJECTS;

const G: f64 = 6.674e-11;
const C: f64 = 2.997_924_58e8;

fn object(name: &str) -> &'static StellarObject {
    stellar_object(name).unwrap_or_else(|| panic!("no `{name}` in the catalog"))
}

// -----------------------------------------------------------------------
// The table.
// -----------------------------------------------------------------------

#[test]
fn every_row_is_valid() {
    assert_eq!(STELLAR_OBJECTS.len(), 5);

    for candidate in STELLAR_OBJECTS {
        assert!(
            candidate.is_valid(),
            "{} is not a valid row: {candidate:?}",
            candidate.name
        );
    }
}

#[test]
fn no_two_rows_share_a_name() {
    // `stellar_object` resolves by name, so a duplicate would make one of them unreachable.
    for (index, candidate) in STELLAR_OBJECTS.iter().enumerate() {
        for other in &STELLAR_OBJECTS[index + 1..] {
            assert_ne!(
                candidate.name, other.name,
                "{} appears twice",
                candidate.name
            );
        }
    }
}

#[test]
fn lookup_is_case_insensitive_and_trimmed() {
    assert!(stellar_object("NEUTRON STAR").is_some());
    assert!(stellar_object("  white dwarf ").is_some());
    assert!(stellar_object("").is_none());
    assert!(stellar_object("Death Star").is_none());
}

#[test]
fn every_mass_fits_in_an_f32_without_losing_the_object() {
    // `Prop::mass_kg` is `f32`, whose ceiling is 3.4e38. Sgr A* is 8.5e36 — comfortable, but
    // it is why the catalog stops short of M87* at 1.3e40, and that reasoning deserves a
    // test rather than only a comment.
    for candidate in STELLAR_OBJECTS {
        let narrowed = candidate.mass_kg as f32;

        assert!(
            narrowed.is_finite() && narrowed > 0.0,
            "{} does not survive the f32 narrowing: {}",
            candidate.name,
            candidate.mass_kg
        );
        assert!(
            (f64::from(narrowed) / candidate.mass_kg - 1.0).abs() < 1.0e-6,
            "{} loses precision in f32",
            candidate.name
        );
    }
}

// -----------------------------------------------------------------------
// The measurements, against published figures.
// -----------------------------------------------------------------------

#[test]
fn the_masses_are_the_published_ones_in_solar_masses() {
    // How these objects are actually quoted, which is the form an error would be visible in.
    let expected = [
        ("Red dwarf", 0.122),
        ("White dwarf", 1.018),
        ("Neutron star", 1.4),
        ("Black hole", 10.0),
        ("Sgr A*", 4.297e6),
    ];

    for (name, solar_masses) in expected {
        let got = object(name).solar_masses();

        assert!(
            (got / solar_masses - 1.0).abs() < 0.01,
            "{name} is {got} solar masses, expected {solar_masses}"
        );
    }
}

#[test]
fn the_radii_are_the_published_ones() {
    let expected = [
        ("Red dwarf", 1.070e8),
        ("White dwarf", 5.850e6),
        ("Neutron star", 1.100e4),
    ];

    for (name, radius_m) in expected {
        let got = object(name).radius_m;

        assert!(
            (got / radius_m - 1.0).abs() < 1.0e-3,
            "{name} has radius {got} m, expected {radius_m}"
        );
    }
}

#[test]
fn a_black_hole_radius_is_its_schwarzschild_radius() {
    // `2GM/c²`, recomputed from the mass rather than compared to the table's own figure —
    // this is the one radius in the catalog that is *derived* from the mass, so if the two
    // ever disagree the table is internally inconsistent.
    for name in ["Black hole", "Sgr A*"] {
        let candidate = object(name);
        let expected = 2.0 * G * candidate.mass_kg / (C * C);

        assert!(
            (candidate.radius_m / expected - 1.0).abs() < 1.0e-3,
            "{name} has radius {} m but 2GM/c² is {expected}",
            candidate.radius_m
        );
    }
}

#[test]
fn a_white_dwarf_is_a_suns_mass_in_an_earths_volume() {
    // The fact that makes a white dwarf worth having in the catalog, and the reason it needs
    // no special drawn radius: it is Earth-sized, which is exactly what the planets' scale
    // was built for.
    let dwarf = object("White dwarf");

    assert!((dwarf.solar_masses() - 1.0).abs() < 0.05);
    assert!(
        (dwarf.radius_m / crate::experiments::constants::EARTH_RADIUS_M - 0.92).abs() < 0.05,
        "a white dwarf should be within a few percent of an Earth radius"
    );
}

// -----------------------------------------------------------------------
// The three drawn-size regimes, each pinned as a regime.
// -----------------------------------------------------------------------

#[test]
fn every_object_is_stored_at_its_true_size() {
    // Amendment A16 collapsed three drawn-size regimes into none. Before it, the dwarfs used the
    // planets' 157x exaggeration, the neutron star and stellar hole were *floored* at 0.30 and
    // 0.806 units because 1.7e-3 was invisible, and Sgr A* alone was true. All three were working
    // around one thing: the drawn radius was also the collision radius. It no longer is.
    for candidate in STELLAR_OBJECTS {
        let stored = candidate.radius_units();
        let truth = (candidate.radius_m / crate::constants::METRES_PER_UNIT) as f32;

        assert!(
            (stored / truth - 1.0).abs() < 1.0e-6,
            "{} is stored at {stored} against a true {truth}",
            candidate.name
        );
    }
}

#[test]
fn the_compact_objects_are_genuinely_microscopic_and_that_is_fine_now() {
    // A neutron star is 1.1e-5 units and a 10 M☉ horizon 2.95e-5 — six orders of magnitude below
    // the red dwarf. Storing them honestly is only possible because the renderer floors *apparent*
    // size rather than stored size; before A16 each needed a stand-in radius that also had to
    // serve as its collision radius.
    for name in ["Neutron star", "Black hole"] {
        let candidate = object(name);

        assert!(
            candidate.radius_units() < 1.0e-4,
            "{name} is {} units, and it should be tiny",
            candidate.radius_units()
        );
    }

    // The true ratio between them, which no longer needs preserving by hand.
    let ratio = object("Black hole").radius_m / object("Neutron star").radius_m;
    assert!((ratio - 2.685).abs() < 0.01, "got {ratio}");
}

#[test]
fn a_black_hole_captures_at_its_tidal_radius_not_its_horizon() {
    // The physics that keeps absorption observable *and* correct. A hole does not destroy matter
    // at its event horizon — it destroys it where the gravity gradient beats the infalling body's
    // self-gravity, `R_t ≈ R_star · (M_hole/M_star)^{1/3}`. For a solar-type star that is 50,000x
    // the horizon of a 10 M☉ hole.
    const RSUN_M: f64 = 6.957e8;

    for name in ["Black hole", "Sgr A*"] {
        let candidate = object(name);
        let capture = candidate
            .capture_radius_m
            .unwrap_or_else(|| panic!("{name} must define a capture radius"));
        let expected = RSUN_M * (candidate.mass_kg / 1.989e30).cbrt();

        assert!(
            (capture / expected - 1.0).abs() < 0.01,
            "{name} captures at {capture} m, and the tidal radius is {expected}"
        );
        assert!(
            capture > candidate.radius_m,
            "and it must be outside the horizon, or the star would cross before being torn apart"
        );
    }

    // Only the black holes: everything else touches at its own surface.
    for name in ["Red dwarf", "White dwarf", "Neutron star"] {
        assert!(
            object(name).capture_radius_m.is_none(),
            "{name} has no capture radius"
        );
        assert_eq!(
            object(name).contact_radius_units(),
            object(name).radius_units()
        );
    }
}

#[test]
fn the_supermassive_black_hole_is_the_one_object_visible_without_help() {
    // Its event horizon is 12.7 units — larger than the Sun's 0.696 — so it is the only object in
    // the catalog whose true size exceeds the apparent-size floor at any sensible distance.
    let hole = object("Sgr A*");

    assert!(
        (hole.radius_units() - 12.693).abs() < 0.01,
        "got {}",
        hole.radius_units()
    );
    assert!(
        hole.radius_units()
            > crate::experiments::solar_system::radius_units(
                crate::experiments::constants::SOLAR_SYSTEM_SUN_RADIUS_M
            ),
        "and it dwarfs the Sun, which is the comparison worth making"
    );
}

#[test]
fn the_compact_objects_are_genuinely_invisible_at_range_and_that_is_the_truth() {
    // Amendment A17 removed the apparent-size floor, so nothing lifts a small body to a visible
    // minimum. A neutron star is 1.1e-5 units: from the 40-unit placement distance it subtends
    // 0.001 px and is simply not drawn. You have to fly to it, and this test records that as the
    // deliberate outcome rather than leaving it to be rediscovered as a bug.
    let pixels_per_radian = 1600.0 / (45.0_f32).to_radians();
    let distance = crate::render::constants::PLACEMENT_DISTANCE_UNITS;

    let visible =
        |candidate: &StellarObject| 2.0 * candidate.radius_units() / distance * pixels_per_radian;

    assert!(
        visible(object("Sgr A*")) > 100.0,
        "a supermassive horizon is unmissable"
    );
    assert!(
        visible(object("Red dwarf")) > 5.0,
        "and a red dwarf is a disc"
    );
    assert!(
        visible(object("Neutron star")) < 0.01,
        "while a neutron star is nothing at all until you reach it"
    );
}

// -----------------------------------------------------------------------
// Turning a row into a prop.
// -----------------------------------------------------------------------

#[test]
fn every_object_becomes_a_valid_prop_at_rest() {
    let at = Vec3::new(1.0, 2.0, 3.0);

    for candidate in STELLAR_OBJECTS {
        let prop = candidate.to_prop(at);

        assert!(prop.is_valid(), "{} made an invalid prop", candidate.name);
        assert_eq!(prop.body.center(), at);
        assert_eq!(
            prop.velocity_units_s,
            Vec3::ZERO,
            "a placed object falls from rest"
        );
        assert!(
            prop.responds_to_forces(),
            "{} must be movable and massive",
            candidate.name
        );
        assert_eq!(prop.mass_kg, candidate.mass_kg as f32);
    }
}

#[test]
fn the_prop_carries_the_true_radius_and_the_capture_radius_separately() {
    for candidate in STELLAR_OBJECTS {
        let prop = candidate.to_prop(Vec3::ZERO);

        match prop.body {
            Body::Sphere { radius, .. } => assert_eq!(radius, candidate.radius_units()),
            other => panic!("{} is not a sphere: {other:?}", candidate.name),
        }

        // A contact radius is set only where it genuinely differs from the surface.
        match candidate.capture_radius_m {
            Some(_) => assert_eq!(
                prop.contact_radius_units,
                Some(candidate.contact_radius_units())
            ),
            None => assert_eq!(prop.contact_radius_units, None),
        }
    }
}

#[test]
fn only_the_black_holes_absorb() {
    // The flag that picks the contact response. A red dwarf bounces; an event horizon does
    // not, and getting this backwards would be invisible until something touched something.
    for candidate in STELLAR_OBJECTS {
        let expected = candidate.name.contains("Black hole") || candidate.name == "Sgr A*";

        assert_eq!(
            candidate.absorbs, expected,
            "{} has absorbs = {}",
            candidate.name, candidate.absorbs
        );
        assert_eq!(candidate.to_prop(Vec3::ZERO).absorbs, expected);
    }
}

#[test]
fn a_black_hole_is_nearly_black() {
    // It is the one object whose colour is load-bearing rather than decorative: an event
    // horizon emits nothing, and a grey sphere would read as a rock.
    for name in ["Black hole", "Sgr A*"] {
        let colour = object(name).color;
        let brightest = colour.r.max(colour.g).max(colour.b);

        assert!(brightest < 0.05, "{name} is too bright: {brightest}");
        assert_eq!(colour.a, 1.0, "and it is opaque, not translucent");
    }
}

#[test]
fn every_note_says_something_a_name_does_not() {
    // The picker shows the note beside the name, and the point of these objects is the
    // numbers. A note that just repeated the name would waste the only line it gets.
    for candidate in STELLAR_OBJECTS {
        assert!(!candidate.note.trim().is_empty());
        assert!(
            candidate.note.contains("Msun") || candidate.note.contains("million"),
            "{}'s note should quote a mass: {:?}",
            candidate.name,
            candidate.note
        );
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "fails Validation_Rules")]
fn an_invalid_object_panics_in_debug_rather_than_placing_a_broken_prop() {
    let mut broken = STELLAR_OBJECTS[0];
    broken.radius_m = 0.0;

    let _ = broken.to_prop(Vec3::ZERO);
}

#[test]
fn an_invalid_object_is_caught_by_validation() {
    // With no exaggeration left to hide inside, a bad measurement fails `is_valid` rather than
    // being quietly replaced — which is what `Scene::add`'s debug assert then reports.
    let mut broken = STELLAR_OBJECTS[0];
    broken.radius_m = 0.0;
    assert!(!broken.is_valid());

    broken = STELLAR_OBJECTS[3];
    broken.capture_radius_m = Some(-1.0);
    assert!(!broken.is_valid());
}

#[test]
fn the_three_stars_light_the_scene_and_the_black_holes_do_not() {
    // A star is self-luminous and a black hole is not, and that is a fact about the object rather
    // than a lighting preference — so placing a red dwarf lights whatever is around it, at 1/600th
    // of the Sun's output, which is its real relative brightness.
    let expected = [
        ("Red dwarf", 0.0017),
        ("White dwarf", 0.056),
        ("Neutron star", 1.0e-5),
        ("Black hole", 0.0),
        ("Sgr A*", 0.0),
    ];

    for (name, luminosity) in expected {
        let candidate = object(name);

        assert_eq!(
            candidate.luminosity_solar, luminosity,
            "{name} emits the wrong amount of light"
        );
        assert_eq!(
            candidate.to_prop(Vec3::ZERO).luminosity_solar,
            luminosity,
            "{name}'s prop must carry it through to the renderer"
        );
    }

    // An accretion disc would emit; this crate does not model one, so a hole is dark.
    for name in ["Black hole", "Sgr A*"] {
        assert!(
            !object(name).to_prop(Vec3::ZERO).is_luminous(),
            "{name} must be dark"
        );
    }
    for name in ["Red dwarf", "White dwarf", "Neutron star"] {
        assert!(
            object(name).to_prop(Vec3::ZERO).is_luminous(),
            "{name} must be a light source"
        );
    }
}

#[test]
fn a_negative_luminosity_is_rejected() {
    let mut broken = STELLAR_OBJECTS[0];
    broken.luminosity_solar = -1.0;
    assert!(!broken.is_valid());

    broken.luminosity_solar = f32::NAN;
    assert!(!broken.is_valid());
}
