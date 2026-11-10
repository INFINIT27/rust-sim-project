//! Tests for [`crate::config`], `#[path]`-included as a unit-test module of it: the shipped files,
//! defaults for anything left out, and an error naming the key for anything wrong.

use super::*;
use crate::config::files::{CameraFile, PerformanceFile, SpacecraftFile, TrailsFile, UiFile};
use crate::performance::AntiAliasing;
use crate::spacecraft::{Propulsion, Spacecraft};

/// The shipped `config/` directory.
fn shipped() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CONFIG_DIR_NAME)
}

/// A fresh, empty directory under the target directory, for files written by a test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("config-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_shipped_files_are_the_built_in_configuration() {
    // Every value in `config/` is the built-in default, so shipping the files changes nothing
    // until someone edits them, and a default changed in code without the file fails here.
    let config = Config::load(&shipped()).expect("the shipped files must load");
    assert_eq!(config.fleet, Fleet::default());
    assert_eq!(config.camera, CameraSettings::default());
    assert_eq!(config.crosshair, CrosshairConfig::default());
    assert_eq!(config.compass, CompassConfig::default());
    assert_eq!(config.arrival, ArrivalPopupConfig::default());
    assert_eq!(config.trails, TrailConfig::default());
    assert_eq!(config.performance, PerformanceSettings::default());
}

#[test]
fn every_file_is_optional() {
    let config = Config::load(Path::new("no/such/directory")).unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn an_empty_file_changes_nothing() {
    assert_eq!(
        parse("", CameraFile::into_runtime).unwrap(),
        CameraSettings::default()
    );
    assert_eq!(
        parse("", UiFile::into_runtime).unwrap(),
        (
            CrosshairConfig::default(),
            CompassConfig::default(),
            ArrivalPopupConfig::default()
        )
    );
    assert_eq!(
        parse("", TrailsFile::into_runtime).unwrap(),
        TrailConfig::default()
    );
    assert_eq!(
        parse("", PerformanceFile::into_runtime).unwrap(),
        PerformanceSettings::default()
    );
    assert_eq!(
        parse("", SpacecraftFile::into_runtime).unwrap(),
        Fleet::default(),
        "a file with no [[craft]] keeps the built-in fleet"
    );
}

#[test]
fn a_key_changes_only_its_own_setting() {
    let camera = parse("move_speed_units_s = 75.0", CameraFile::into_runtime).unwrap();
    assert_eq!(
        camera,
        CameraSettings {
            move_speed: 75.0,
            ..CameraSettings::default()
        }
    );

    let (crosshair, compass, _) = parse(
        "[crosshair]\ncolor_srgba = [255, 0, 0, 255]",
        UiFile::into_runtime,
    )
    .unwrap();
    assert_eq!(crosshair.color, [255, 0, 0, 255]);
    assert_eq!(crosshair.gap_px, CrosshairConfig::default().gap_px);
    assert_eq!(compass, CompassConfig::default());

    let performance = parse(
        "anti_aliasing = \"FXAA\"\nvsync = false",
        PerformanceFile::into_runtime,
    )
    .unwrap();
    assert_eq!(performance.anti_aliasing, AntiAliasing::Fxaa);
    assert!(!performance.vsync);
    assert_eq!(
        performance.floor_eclipse,
        PerformanceSettings::default().floor_eclipse
    );
}

#[test]
fn a_misspelt_key_is_an_error_naming_it() {
    let error = parse("move_sped_units_s = 75.0", CameraFile::into_runtime).unwrap_err();
    assert!(error.contains("move_sped_units_s"), "{error}");

    let error = parse("[crosshiar]\ngap_px = 1.0", UiFile::into_runtime).unwrap_err();
    assert!(error.contains("crosshiar"), "{error}");
}

#[test]
fn a_value_out_of_range_is_an_error_naming_its_key() {
    let error = parse(
        "look_sensitivity_rad_per_px = -1.0",
        CameraFile::into_runtime,
    )
    .unwrap_err();
    assert!(error.contains("look_sensitivity_rad_per_px"), "{error}");

    let error = parse("tail_alpha = 2.0", TrailsFile::into_runtime).unwrap_err();
    assert!(error.contains("tail_alpha"), "{error}");

    let error = parse("max_samples = 1", TrailsFile::into_runtime).unwrap_err();
    assert!(error.contains("max_samples"), "{error}");

    let error = parse("[compass]\nradius_px = 0.0", UiFile::into_runtime).unwrap_err();
    assert!(error.contains("compass.radius_px"), "{error}");

    let error = parse("anti_aliasing = \"ssaa\"", PerformanceFile::into_runtime).unwrap_err();
    assert!(error.contains("anti_aliasing"), "{error}");

    let error = parse("move_speed_units_s = \"fast\"", CameraFile::into_runtime).unwrap_err();
    assert!(
        error.contains("move_speed_units_s"),
        "a wrong type names its key too: {error}"
    );
}

const ONE_PROBE: &str = r#"
[[craft]]
name = "Pathfinder"
mass_kg = 500.0

[craft.propulsion]
kind = "coasting"
max_departure_speed_m_s = 12000.0
"#;

#[test]
fn a_fleet_can_be_replaced_and_optional_craft_keys_take_the_built_in_values() {
    let fleet = parse(ONE_PROBE, SpacecraftFile::into_runtime).unwrap();
    assert_eq!(fleet.craft().len(), 1);
    let probe = fleet.selected();
    let built_in = Spacecraft::coasting_probe();
    assert_eq!(probe.name, "Pathfinder");
    assert_eq!(probe.mass_kg, 500.0);
    assert_eq!(probe.radius_m, built_in.radius_m);
    assert_eq!(probe.color, built_in.color);
    assert_eq!(probe.launch_altitude_m, built_in.launch_altitude_m);
    assert_eq!(
        probe.propulsion,
        Propulsion::Coasting {
            max_departure_speed_m_s: 12_000.0
        }
    );
}

#[test]
fn an_engine_limit_left_out_is_full_thrust_and_one_given_is_in_g() {
    let text = r#"
[[craft]]
name = "Tug"
mass_kg = 1000.0
[craft.propulsion]
kind = "direct"
[craft.propulsion.engine]
name = "Small"
thrust_n = 20000.0
specific_impulse_s = 300.0

[[craft]]
name = "Liner"
mass_kg = 1000.0
[craft.propulsion]
kind = "transfer"
max_acceleration_g = 2.0
[craft.propulsion.engine]
name = "Small"
thrust_n = 20000.0
specific_impulse_s = 300.0
"#;
    let fleet = parse(text, SpacecraftFile::into_runtime).unwrap();
    let [tug, liner] = fleet.craft() else {
        panic!("two craft expected");
    };
    let g = crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
    assert_eq!(tug.acceleration_m_s2(), Some(20.0), "20 kN on 1 t");
    assert_eq!(liner.acceleration_m_s2(), Some(2.0 * g));
    assert_eq!(liner.engine().unwrap().exhaust_speed_m_s, 300.0 * g);
}

#[test]
fn an_invalid_craft_is_an_error_naming_it() {
    let error = parse(
        &ONE_PROBE.replace("mass_kg = 500.0", "mass_kg = 0.0"),
        SpacecraftFile::into_runtime,
    )
    .unwrap_err();
    assert!(error.contains("Pathfinder"), "{error}");

    let error = parse(
        &ONE_PROBE.replace("kind = \"coasting\"", "kind = \"warp\""),
        SpacecraftFile::into_runtime,
    )
    .unwrap_err();
    assert!(error.contains("warp"), "{error}");

    let error = parse(
        &ONE_PROBE.replace(
            "mass_kg = 500.0",
            "mass_kg = 500.0\ncolor = [2.0, 0.0, 0.0, 1.0]",
        ),
        SpacecraftFile::into_runtime,
    )
    .unwrap_err();
    assert!(error.contains("color"), "{error}");
}

#[test]
fn a_bad_file_on_disk_is_an_error_naming_the_file() {
    let dir = scratch("bad_file");
    std::fs::write(dir.join(TRAILS_FILE), "length_units = -3.0").unwrap();
    let error = Config::load(&dir).unwrap_err();
    assert_eq!(error.path, dir.join(TRAILS_FILE));
    assert!(error.to_string().contains("length_units"), "{error}");
}

#[test]
fn files_on_disk_are_read() {
    let dir = scratch("good_files");
    std::fs::write(dir.join(SPACECRAFT_FILE), ONE_PROBE).unwrap();
    std::fs::write(dir.join(TRAILS_FILE), "visible = false").unwrap();
    let config = Config::load(&dir).unwrap();
    assert_eq!(config.fleet.selected().name, "Pathfinder");
    assert!(!config.trails.visible);
    assert_eq!(
        config.camera,
        CameraSettings::default(),
        "an absent file is still optional"
    );
}

#[test]
fn the_environment_overrides_the_performance_file() {
    let file = parse(
        "anti_aliasing = \"smaa\"\nvsync = false",
        PerformanceFile::into_runtime,
    )
    .unwrap();
    let env = |key: &str| {
        (key == crate::performance::constants::ANTI_ALIASING_VAR).then(|| "off".to_string())
    };
    let settings = PerformanceSettings::from_lookup_over(file, env).unwrap();
    assert_eq!(
        settings.anti_aliasing,
        AntiAliasing::Off,
        "the variable wins"
    );
    assert!(
        !settings.vsync,
        "and the file holds where no variable is set"
    );
}

#[test]
fn inserted_settings_survive_the_plugins_that_own_them() {
    use bevy::MinimalPlugins;

    let config = Config {
        fleet: parse(ONE_PROBE, SpacecraftFile::into_runtime).unwrap(),
        camera: CameraSettings {
            move_speed: 99.0,
            ..CameraSettings::default()
        },
        ..Config::default()
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    config.insert_into(&mut app);
    app.add_plugins(crate::render::mission::MissionPlugin);
    app.add_plugins(crate::input::camera::FreeFlyCameraPlugin);

    assert_eq!(
        app.world().resource::<Mission>().fleet.selected().name,
        "Pathfinder"
    );
    assert_eq!(app.world().resource::<CameraSettings>().move_speed, 99.0);
}
