//! Unit tests for [`crate::performance::settings`].

use std::collections::HashMap;
use std::time::Duration;

use super::*;

fn parse(pairs: &[(&str, &str)]) -> Result<PerformanceSettings, SettingsError> {
    let env: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    PerformanceSettings::from_lookup(|key| env.get(key).cloned())
}

#[test]
fn an_empty_environment_is_the_shipped_configuration() {
    let settings = parse(&[]).unwrap();
    assert_eq!(settings, PerformanceSettings::default());
    assert_eq!(settings.anti_aliasing, AntiAliasing::Msaa4);
    assert!(settings.floor_eclipse);
    assert!(settings.vsync);
    assert_eq!(settings.bench, None);
    assert_eq!(settings.present_mode(), PresentMode::AutoVsync);
}

#[test]
fn every_anti_aliasing_mode_parses_case_insensitively() {
    for (value, mode) in [
        ("msaa4", AntiAliasing::Msaa4),
        ("MSAA2", AntiAliasing::Msaa2),
        ("Smaa", AntiAliasing::Smaa),
        ("fxaa", AntiAliasing::Fxaa),
        ("off", AntiAliasing::Off),
    ] {
        assert_eq!(
            parse(&[("RGA_AA", value)]).unwrap().anti_aliasing,
            mode,
            "{value}"
        );
    }
}

#[test]
fn post_process_modes_turn_msaa_off() {
    assert_eq!(AntiAliasing::Msaa4.msaa(), Msaa::Sample4);
    assert_eq!(AntiAliasing::Msaa2.msaa(), Msaa::Sample2);
    for mode in [AntiAliasing::Smaa, AntiAliasing::Fxaa, AntiAliasing::Off] {
        assert_eq!(mode.msaa(), Msaa::Off, "{mode:?}");
    }
}

#[test]
fn switches_turn_off() {
    let settings = parse(&[("RGA_FLOOR_ECLIPSE", "off"), ("RGA_VSYNC", "0")]).unwrap();
    assert!(!settings.floor_eclipse);
    assert!(!settings.vsync);
    assert_eq!(settings.present_mode(), PresentMode::AutoNoVsync);
}

#[test]
fn a_misspelt_value_is_an_error_naming_the_variable() {
    let error = parse(&[("RGA_AA", "smmaa")]).unwrap_err();
    assert_eq!(error.variable, "RGA_AA");
    assert_eq!(error.value, "smmaa");
    assert!(error.to_string().contains("RGA_AA=\"smmaa\""));

    assert_eq!(
        parse(&[("RGA_FLOOR_ECLIPSE", "maybe")])
            .unwrap_err()
            .variable,
        "RGA_FLOOR_ECLIPSE"
    );
}

#[test]
fn the_benchmark_uses_default_timing_and_forces_vsync_off() {
    let settings = parse(&[("RGA_BENCH", "1"), ("RGA_VSYNC", "on")]).unwrap();
    assert_eq!(
        settings.bench,
        Some(BenchConfig {
            warmup: Duration::from_secs(3),
            window: Duration::from_secs(10),
        })
    );
    assert_eq!(settings.present_mode(), PresentMode::AutoNoVsync);
}

#[test]
fn benchmark_timing_is_configurable_and_validated() {
    let settings = parse(&[
        ("RGA_BENCH", "on"),
        ("RGA_BENCH_WARMUP", "0.5"),
        ("RGA_BENCH_SECONDS", "2"),
    ])
    .unwrap();
    let bench = settings.bench.unwrap();
    assert_eq!(bench.warmup, Duration::from_millis(500));
    assert_eq!(bench.window, Duration::from_secs(2));

    for bad in ["0", "-1", "soon", "inf"] {
        assert!(
            parse(&[("RGA_BENCH", "on"), ("RGA_BENCH_SECONDS", bad)]).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn bench_off_ignores_its_timing_variables() {
    let settings = parse(&[("RGA_BENCH", "off"), ("RGA_BENCH_SECONDS", "soon")]).unwrap();
    assert_eq!(settings.bench, None);
}
