//! The performance switches, parsed once from the environment at startup.

use std::fmt;
use std::time::Duration;

use bevy::prelude::{Msaa, Resource};
use bevy::window::PresentMode;

use crate::performance::constants::{
    ANTI_ALIASING_VAR, BENCH_SECONDS_VAR, BENCH_VAR, BENCH_WARMUP_VAR, DEFAULT_ANTI_ALIASING,
    DEFAULT_BENCH_SECONDS, DEFAULT_BENCH_WARMUP_SECONDS, DEFAULT_FLOOR_ECLIPSE, DEFAULT_VSYNC,
    FLOOR_ECLIPSE_VAR, VSYNC_VAR,
};

/// How edges are smoothed.
///
/// The two families cost differently on a tile-based GPU: MSAA multiplies the size of every render
/// target, so every extra pass (the trails' transparent pass is one) pays for it again, while SMAA
/// and FXAA are single post-processing passes over a single-sampled image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntiAliasing {
    Msaa4,
    Msaa2,
    Smaa,
    Fxaa,
    Off,
}

impl AntiAliasing {
    /// The global MSAA setting this mode needs. The post-process modes need it off: they run on
    /// the resolved image and MSAA would only add cost under them.
    pub fn msaa(self) -> Msaa {
        match self {
            Self::Msaa4 => Msaa::Sample4,
            Self::Msaa2 => Msaa::Sample2,
            Self::Smaa | Self::Fxaa | Self::Off => Msaa::Off,
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "msaa4" | "msaa" => Some(Self::Msaa4),
            "msaa2" => Some(Self::Msaa2),
            "smaa" => Some(Self::Smaa),
            "fxaa" => Some(Self::Fxaa),
            "off" | "none" => Some(Self::Off),
            _ => None,
        }
    }
}

/// How long the frame-time benchmark warms up and records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchConfig {
    pub warmup: Duration,
    pub window: Duration,
}

/// Every performance switch the app reads, as one ECS resource.
///
/// Built by [`PerformanceSettings::from_env`] before the `App` exists, because vsync has to be
/// known when the window is created. [`Default`] is the shipped configuration.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PerformanceSettings {
    pub anti_aliasing: AntiAliasing,
    /// Whether the floor slab computes eclipse shadows. Spheres always do.
    pub floor_eclipse: bool,
    pub vsync: bool,
    /// `Some` when the frame-time benchmark is on.
    pub bench: Option<BenchConfig>,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            anti_aliasing: DEFAULT_ANTI_ALIASING,
            floor_eclipse: DEFAULT_FLOOR_ECLIPSE,
            vsync: DEFAULT_VSYNC,
            bench: None,
        }
    }
}

/// An environment variable set to a value the app does not understand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsError {
    pub variable: &'static str,
    pub value: String,
    pub expected: &'static str,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}={:?} is not valid; expected {}",
            self.variable, self.value, self.expected
        )
    }
}

impl std::error::Error for SettingsError {}

impl PerformanceSettings {
    /// Read the process environment. An unset variable takes its default; a misspelt value is an
    /// error rather than a silent default, because a benchmark that quietly ran the wrong
    /// configuration is worse than one that refused to start.
    pub fn from_env() -> Result<Self, SettingsError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// [`Self::from_env`] against any lookup, so the parsing is testable without touching the
    /// process environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, SettingsError> {
        let mut settings = Self::default();

        if let Some(value) = lookup(ANTI_ALIASING_VAR) {
            settings.anti_aliasing = AntiAliasing::parse(&value).ok_or(SettingsError {
                variable: ANTI_ALIASING_VAR,
                value,
                expected: "msaa4, msaa2, smaa, fxaa, or off",
            })?;
        }
        if let Some(value) = lookup(FLOOR_ECLIPSE_VAR) {
            settings.floor_eclipse = parse_switch(FLOOR_ECLIPSE_VAR, value)?;
        }
        if let Some(value) = lookup(VSYNC_VAR) {
            settings.vsync = parse_switch(VSYNC_VAR, value)?;
        }
        if let Some(value) = lookup(BENCH_VAR) {
            if parse_switch(BENCH_VAR, value)? {
                settings.bench = Some(BenchConfig {
                    warmup: parse_seconds(
                        BENCH_WARMUP_VAR,
                        lookup(BENCH_WARMUP_VAR),
                        DEFAULT_BENCH_WARMUP_SECONDS,
                    )?,
                    window: parse_seconds(
                        BENCH_SECONDS_VAR,
                        lookup(BENCH_SECONDS_VAR),
                        DEFAULT_BENCH_SECONDS,
                    )?,
                });
            }
        }

        Ok(settings)
    }

    /// The window's present mode. The benchmark overrides vsync: a frame rate capped at the
    /// display's refresh measures the display.
    pub fn present_mode(&self) -> PresentMode {
        if self.vsync && self.bench.is_none() {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }
}

fn parse_switch(variable: &'static str, value: String) -> Result<bool, SettingsError> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Ok(true),
        "off" | "0" | "false" | "no" => Ok(false),
        _ => Err(SettingsError {
            variable,
            value,
            expected: "on or off",
        }),
    }
}

fn parse_seconds(
    variable: &'static str,
    value: Option<String>,
    default: f64,
) -> Result<Duration, SettingsError> {
    let Some(value) = value else {
        return Ok(Duration::from_secs_f64(default));
    };
    match value.parse::<f64>() {
        Ok(seconds) if seconds.is_finite() && seconds > 0.0 => Ok(Duration::from_secs_f64(seconds)),
        _ => Err(SettingsError {
            variable,
            value,
            expected: "a positive number of seconds",
        }),
    }
}

#[cfg(test)]
#[path = "../../tst/performance/settings_tests.rs"]
mod tests;
