//! Tests for [`crate::projects::interstellar::wormhole`], `#[path]`-included into it.

use super::*;

fn settings() -> WormholeSettings {
    WormholeSettings {
        throat_units: 5.0,
        half_length: 0.5,
        lensing_width: 0.05,
    }
}

#[test]
fn the_shape_is_the_settings_in_world_units() {
    assert_eq!(
        settings().shape(),
        WormholeShape::dneg(5.0, 2.5, f64::from(0.05_f32) * 5.0).unwrap()
    );
}

#[test]
fn settings_outside_their_ranges_are_invalid_and_draw_a_usable_shape() {
    assert!(settings().is_valid());
    for bad in [
        WormholeSettings {
            throat_units: 0.0,
            ..settings()
        },
        WormholeSettings {
            half_length: -1.0,
            ..settings()
        },
        WormholeSettings {
            lensing_width: f32::NAN,
            ..settings()
        },
    ] {
        assert!(!bad.is_valid());
        assert!(bad.shape().throat() > 0.0);
    }
}

#[test]
fn the_shipped_settings_are_valid() {
    assert!(WormholeSettings::default().is_valid());
}

#[test]
fn the_scene_is_an_empty_wormhole() {
    let experiment = wormhole();
    assert!(is_wormhole(&experiment.spec));
    assert!(experiment.spec.is_valid());
    assert!(experiment.scene.props.is_empty());
    let space = experiment.scene.wormhole.as_ref().unwrap();
    assert_eq!(space.shape, WormholeSettings::default().shape());
}
