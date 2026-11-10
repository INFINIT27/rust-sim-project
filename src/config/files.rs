//! What each configuration file may contain, and how it becomes the resource the app runs on.
//!
//! One `…File` type per file, kept apart from the runtime types it builds so the file format is a
//! choice of this layer rather than of every layer it configures. Each one:
//!
//! - **defaults every field** (`#[serde(default)]`) from the runtime type's own `Default`, so a file
//!   need only list what it changes and an empty file changes nothing;
//! - **rejects unknown keys** (`#[serde(deny_unknown_fields)]`), so a misspelt key is an error
//!   rather than a setting that silently does nothing;
//! - **checks its values** in `into_runtime`, returning a message naming the bad key.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bindings::{key_name, parse_key, Action, KeyMap};

use crate::core::scene::Color;
use crate::input::camera::CameraSettings;
use crate::performance::{AntiAliasing, PerformanceSettings};
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::render::trail::TrailConfig;
use crate::spacecraft::constants::{CRAFT_COLOR, CRAFT_RADIUS_M, LAUNCH_ALTITUDE_M};
use crate::spacecraft::{Engine, Fleet, Propulsion, Spacecraft};
use crate::ui::arrival::ArrivalPopupConfig;
use crate::ui::compass::CompassConfig;
use crate::ui::crosshair::CrosshairConfig;

/// `Ok` when `value` is finite and above zero, otherwise a message naming `key`.
fn positive(key: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!(
            "`{key}` must be a finite number above zero, got {value}"
        ))
    }
}

/// `Ok` when `value` is finite and at least zero, otherwise a message naming `key`.
fn non_negative(key: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!(
            "`{key}` must be a finite number, zero or above, got {value}"
        ))
    }
}

/// `Ok` when `value` is in `[0, 1]`, otherwise a message naming `key`.
fn unit_interval(key: &str, value: f64) -> Result<(), String> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("`{key}` must be between 0 and 1, got {value}"))
    }
}

// ---------------------------------------------------------------------------------------------
// camera.toml
// ---------------------------------------------------------------------------------------------

/// `camera.toml`: the tuning every new camera starts with.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CameraFile {
    /// Movement speed, in world units per second.
    pub move_speed_units_s: f32,
    /// What `Shift` multiplies the speed by.
    pub boost_multiplier: f32,
    /// What the precision speed mode (`C`) multiplies the speed by.
    pub precision_multiplier: f32,
    /// Mouse-look sensitivity, in radians per pixel moved.
    pub look_sensitivity_rad_per_px: f32,
}

impl Default for CameraFile {
    fn default() -> Self {
        Self::from_runtime(&CameraSettings::default())
    }
}

impl CameraFile {
    /// The file that describes `settings`, for saving.
    pub fn from_runtime(settings: &CameraSettings) -> Self {
        Self {
            move_speed_units_s: settings.move_speed,
            boost_multiplier: settings.boost_mult,
            precision_multiplier: settings.precision_mult,
            look_sensitivity_rad_per_px: settings.look_sensitivity,
        }
    }
}

impl CameraFile {
    /// The [`CameraSettings`] this file describes, or a message naming the bad key.
    pub fn into_runtime(self) -> Result<CameraSettings, String> {
        positive("move_speed_units_s", self.move_speed_units_s.into())?;
        positive("boost_multiplier", self.boost_multiplier.into())?;
        positive("precision_multiplier", self.precision_multiplier.into())?;
        positive(
            "look_sensitivity_rad_per_px",
            self.look_sensitivity_rad_per_px.into(),
        )?;
        Ok(CameraSettings {
            move_speed: self.move_speed_units_s,
            boost_mult: self.boost_multiplier,
            precision_mult: self.precision_multiplier,
            look_sensitivity: self.look_sensitivity_rad_per_px,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// ui.toml
// ---------------------------------------------------------------------------------------------

/// `ui.toml`: one table per on-screen element.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct UiFile {
    /// `[crosshair]`.
    pub crosshair: CrosshairFile,
    /// `[compass]`.
    pub compass: CompassFile,
    /// `[arrival]`, the pop-up shown when a spacecraft lands.
    pub arrival: ArrivalFile,
}

/// `[crosshair]` in `ui.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CrosshairFile {
    /// Length of each of the four strokes, in pixels.
    pub arm_length_px: f32,
    /// Empty gap at the centre, in pixels.
    pub gap_px: f32,
    /// Stroke width, in pixels.
    pub thickness_px: f32,
    /// sRGB colour and opacity, each `0`–`255`.
    pub color_srgba: [u8; 4],
}

impl Default for CrosshairFile {
    fn default() -> Self {
        Self::from_runtime(&CrosshairConfig::default())
    }
}

impl CrosshairFile {
    /// The file that describes `config`, for saving.
    pub fn from_runtime(config: &CrosshairConfig) -> Self {
        Self {
            arm_length_px: config.arm_length_px,
            gap_px: config.gap_px,
            thickness_px: config.thickness_px,
            color_srgba: config.color,
        }
    }
}

/// `[compass]` in `ui.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CompassFile {
    /// Length of each axis, in pixels.
    pub radius_px: f32,
    /// Distance from the window's top-right corner, in pixels.
    pub margin_px: f32,
    /// Stroke width, in pixels.
    pub thickness_px: f32,
    /// Axis-label text size, in pixels.
    pub label_size_px: f32,
}

impl Default for CompassFile {
    fn default() -> Self {
        Self::from_runtime(&CompassConfig::default())
    }
}

impl CompassFile {
    /// The file that describes `config`, for saving.
    pub fn from_runtime(config: &CompassConfig) -> Self {
        Self {
            radius_px: config.radius_px,
            margin_px: config.margin_px,
            thickness_px: config.thickness_px,
            label_size_px: config.label_size_px,
        }
    }
}

/// `[arrival]` in `ui.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ArrivalFile {
    /// The pop-up's width, in pixels.
    pub width_px: f32,
    /// The width of its label column, in pixels.
    pub label_column_px: f32,
}

impl Default for ArrivalFile {
    fn default() -> Self {
        Self::from_runtime(&ArrivalPopupConfig::default())
    }
}

impl ArrivalFile {
    /// The file that describes `config`, for saving.
    pub fn from_runtime(config: &ArrivalPopupConfig) -> Self {
        Self {
            width_px: config.width_px,
            label_column_px: config.label_column_px,
        }
    }
}

impl UiFile {
    /// The three UI resources this file describes, or a message naming the bad key.
    pub fn into_runtime(
        self,
    ) -> Result<(CrosshairConfig, CompassConfig, ArrivalPopupConfig), String> {
        let Self {
            crosshair,
            compass,
            arrival,
        } = self;
        non_negative("crosshair.arm_length_px", crosshair.arm_length_px.into())?;
        non_negative("crosshair.gap_px", crosshair.gap_px.into())?;
        positive("crosshair.thickness_px", crosshair.thickness_px.into())?;
        positive("compass.radius_px", compass.radius_px.into())?;
        non_negative("compass.margin_px", compass.margin_px.into())?;
        positive("compass.thickness_px", compass.thickness_px.into())?;
        positive("compass.label_size_px", compass.label_size_px.into())?;
        positive("arrival.width_px", arrival.width_px.into())?;
        positive("arrival.label_column_px", arrival.label_column_px.into())?;
        Ok((
            CrosshairConfig {
                arm_length_px: crosshair.arm_length_px,
                gap_px: crosshair.gap_px,
                thickness_px: crosshair.thickness_px,
                color: crosshair.color_srgba,
            },
            CompassConfig {
                radius_px: compass.radius_px,
                margin_px: compass.margin_px,
                thickness_px: compass.thickness_px,
                label_size_px: compass.label_size_px,
            },
            ArrivalPopupConfig {
                width_px: arrival.width_px,
                label_column_px: arrival.label_column_px,
            },
        ))
    }
}

// ---------------------------------------------------------------------------------------------
// trails.toml
// ---------------------------------------------------------------------------------------------

/// `trails.toml`: the motion trails behind moving bodies.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TrailsFile {
    /// Whether trails are shown at launch (`T` toggles them).
    pub visible: bool,
    /// Trail length, in world units. An experiment that sets its own length overrides this.
    pub length_units: f32,
    /// The smallest distance between two recorded points, in world units.
    pub min_spacing_units: f32,
    /// The most points one trail keeps.
    pub max_samples: usize,
    /// Opacity at the body, `0`–`1`.
    pub head_alpha: f32,
    /// Opacity at the far end, `0`–`1`.
    pub tail_alpha: f32,
}

impl Default for TrailsFile {
    fn default() -> Self {
        Self::from_runtime(&TrailConfig::default())
    }
}

impl TrailsFile {
    /// The file that describes `config`, for saving.
    pub fn from_runtime(config: &TrailConfig) -> Self {
        Self {
            visible: config.visible,
            length_units: config.length_units,
            min_spacing_units: config.min_spacing_units,
            max_samples: config.max_samples,
            head_alpha: config.head_alpha,
            tail_alpha: config.tail_alpha,
        }
    }
}

impl TrailsFile {
    /// The [`TrailConfig`] this file describes, or a message naming the bad key.
    pub fn into_runtime(self) -> Result<TrailConfig, String> {
        positive("length_units", self.length_units.into())?;
        positive("min_spacing_units", self.min_spacing_units.into())?;
        if self.max_samples < 2 {
            return Err(format!(
                "`max_samples` must be at least 2 to draw a line, got {}",
                self.max_samples
            ));
        }
        unit_interval("head_alpha", self.head_alpha.into())?;
        unit_interval("tail_alpha", self.tail_alpha.into())?;
        Ok(TrailConfig {
            visible: self.visible,
            length_units: self.length_units,
            min_spacing_units: self.min_spacing_units,
            max_samples: self.max_samples,
            head_alpha: self.head_alpha,
            tail_alpha: self.tail_alpha,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// performance.toml
// ---------------------------------------------------------------------------------------------

/// `performance.toml`: the quality-versus-frame-time switches. The `RGA_*`
/// environment variables override it for one run. The benchmark is environment-only, since it is
/// something you run, not something you configure.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PerformanceFile {
    /// `msaa4`, `msaa2`, `smaa`, `fxaa`, or `off`.
    pub anti_aliasing: String,
    /// Whether the floor shows eclipse shadows.
    pub floor_eclipse: bool,
    /// Whether frames wait for the display's refresh.
    pub vsync: bool,
}

impl Default for PerformanceFile {
    fn default() -> Self {
        Self::from_runtime(&PerformanceSettings::default())
    }
}

impl PerformanceFile {
    /// The file that describes `settings`, for saving.
    pub fn from_runtime(settings: &PerformanceSettings) -> Self {
        Self {
            anti_aliasing: settings.anti_aliasing.name().to_string(),
            floor_eclipse: settings.floor_eclipse,
            vsync: settings.vsync,
        }
    }
}

impl PerformanceFile {
    /// The [`PerformanceSettings`] this file describes, before the environment overrides it, or a
    /// message naming the bad key.
    pub fn into_runtime(self) -> Result<PerformanceSettings, String> {
        let anti_aliasing = AntiAliasing::parse(&self.anti_aliasing).ok_or_else(|| {
            format!(
                "`anti_aliasing` must be msaa4, msaa2, smaa, fxaa, or off, got {:?}",
                self.anti_aliasing
            )
        })?;
        Ok(PerformanceSettings {
            anti_aliasing,
            floor_eclipse: self.floor_eclipse,
            vsync: self.vsync,
            bench: None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// spacecraft.toml
// ---------------------------------------------------------------------------------------------

/// `spacecraft.toml`: the fleet, in the order `G` cycles through it. The first craft is selected
/// at launch. A file with no `[[craft]]` entries keeps the built-in fleet.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct SpacecraftFile {
    /// One `[[craft]]` table per spacecraft.
    pub craft: Vec<CraftFile>,
}

/// One `[[craft]]` in `spacecraft.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CraftFile {
    /// The name shown in the HUD and the arrival pop-up.
    pub name: String,
    /// Mass, in kilograms.
    pub mass_kg: f64,
    /// Radius, in metres. Optional.
    #[serde(default = "default_radius_m")]
    pub radius_m: f64,
    /// Linear colour and opacity, each `0`–`1`. Optional.
    #[serde(default = "default_color")]
    pub color: [f32; 4],
    /// Height above the origin's surface it launches from, in metres. Optional.
    #[serde(default = "default_launch_altitude_m")]
    pub launch_altitude_m: f64,
    /// How it flies: `[craft.propulsion]`.
    pub propulsion: PropulsionFile,
}

fn default_radius_m() -> f64 {
    CRAFT_RADIUS_M
}

fn default_color() -> [f32; 4] {
    let Color { r, g, b, a } = CRAFT_COLOR;
    [r, g, b, a]
}

fn default_launch_altitude_m() -> f64 {
    LAUNCH_ALTITUDE_M
}

/// `[craft.propulsion]`: which way the craft flies, picked by `kind`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum PropulsionFile {
    /// No engine: launched at up to a speed, then coasts with correction burns.
    Coasting {
        /// The fastest launch it may be given, in m/s.
        max_departure_speed_m_s: f64,
    },
    /// Burns straight at the target.
    Direct {
        /// `[craft.propulsion.engine]`.
        engine: EngineFile,
        /// Acceleration limit, in g. Optional: the engine's full thrust when left out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_acceleration_g: Option<f64>,
    },
    /// Flies the least-energy transfer.
    Transfer {
        /// `[craft.propulsion.engine]`.
        engine: EngineFile,
        /// Acceleration limit, in g. Optional: the engine's full thrust when left out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_acceleration_g: Option<f64>,
    },
}

/// `[craft.propulsion.engine]`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    /// The engine's name.
    pub name: String,
    /// Thrust, in newtons.
    pub thrust_n: f64,
    /// Specific impulse, in seconds; the exhaust speed is this times standard gravity.
    pub specific_impulse_s: f64,
}

impl EngineFile {
    fn into_runtime(self) -> Engine {
        Engine {
            name: self.name,
            thrust_n: self.thrust_n,
            exhaust_speed_m_s: self.specific_impulse_s * STANDARD_GRAVITY_M_S2,
        }
    }
}

impl CraftFile {
    fn into_runtime(self) -> Spacecraft {
        let mass_kg = self.mass_kg;
        // An unset limit is the engine's own: full thrust on this mass.
        let limit = |engine: &Engine, g: Option<f64>| {
            g.map_or(engine.thrust_n / mass_kg, |g| g * STANDARD_GRAVITY_M_S2)
        };
        let propulsion = match self.propulsion {
            PropulsionFile::Coasting {
                max_departure_speed_m_s,
            } => Propulsion::Coasting {
                max_departure_speed_m_s,
            },
            PropulsionFile::Direct {
                engine,
                max_acceleration_g,
            } => {
                let engine = engine.into_runtime();
                Propulsion::Direct {
                    max_acceleration_m_s2: limit(&engine, max_acceleration_g),
                    engine,
                }
            }
            PropulsionFile::Transfer {
                engine,
                max_acceleration_g,
            } => {
                let engine = engine.into_runtime();
                Propulsion::Transfer {
                    max_acceleration_m_s2: limit(&engine, max_acceleration_g),
                    engine,
                }
            }
        };
        let [r, g, b, a] = self.color;
        Spacecraft {
            name: self.name,
            mass_kg,
            radius_m: self.radius_m,
            color: Color { r, g, b, a },
            launch_altitude_m: self.launch_altitude_m,
            propulsion,
        }
    }
}

impl SpacecraftFile {
    /// The [`Fleet`] this file describes (the built-in one when it lists no craft), or a message
    /// naming the craft that is not valid.
    pub fn into_runtime(self) -> Result<Fleet, String> {
        if self.craft.is_empty() {
            return Ok(Fleet::default());
        }
        let craft: Vec<Spacecraft> = self
            .craft
            .into_iter()
            .map(CraftFile::into_runtime)
            .collect();
        for one in &craft {
            if !one.color.is_valid() {
                return Err(format!(
                    "craft {:?}: `color` values must each be between 0 and 1",
                    one.name
                ));
            }
            if !one.is_valid() {
                return Err(format!(
                    "craft {:?}: mass, radius, thrust, specific impulse, acceleration limit and \
                     departure speed must be above zero, and launch altitude zero or above",
                    one.name
                ));
            }
        }
        Fleet::new(craft).ok_or_else(|| "the fleet is not valid".to_string())
    }
}

impl UiFile {
    /// The file that describes the three UI resources, for saving.
    pub fn from_runtime(
        crosshair: &CrosshairConfig,
        compass: &CompassConfig,
        arrival: &ArrivalPopupConfig,
    ) -> Self {
        Self {
            crosshair: CrosshairFile::from_runtime(crosshair),
            compass: CompassFile::from_runtime(compass),
            arrival: ArrivalFile::from_runtime(arrival),
        }
    }
}

/// `value` to nine significant digits, for a number the file states in other units than the
/// runtime holds (g, specific impulse): `263 s` saves as `263`, not as the `262.99999999999997`
/// that dividing it back out of an exhaust speed can give.
fn tidy(value: f64) -> f64 {
    format!("{value:.8e}").parse().unwrap_or(value)
}

impl EngineFile {
    fn from_runtime(engine: &Engine) -> Self {
        Self {
            name: engine.name.clone(),
            thrust_n: engine.thrust_n,
            specific_impulse_s: tidy(engine.exhaust_speed_m_s / STANDARD_GRAVITY_M_S2),
        }
    }
}

impl CraftFile {
    fn from_runtime(craft: &Spacecraft) -> Self {
        // A limit equal to the engine's full thrust on this mass is the one left out of the file.
        let limit = |engine: &Engine, max_acceleration_m_s2: f64| {
            (max_acceleration_m_s2 != engine.thrust_n / craft.mass_kg)
                .then(|| tidy(max_acceleration_m_s2 / STANDARD_GRAVITY_M_S2))
        };
        let propulsion = match &craft.propulsion {
            Propulsion::Coasting {
                max_departure_speed_m_s,
            } => PropulsionFile::Coasting {
                max_departure_speed_m_s: *max_departure_speed_m_s,
            },
            Propulsion::Direct {
                engine,
                max_acceleration_m_s2,
            } => PropulsionFile::Direct {
                engine: EngineFile::from_runtime(engine),
                max_acceleration_g: limit(engine, *max_acceleration_m_s2),
            },
            Propulsion::Transfer {
                engine,
                max_acceleration_m_s2,
            } => PropulsionFile::Transfer {
                engine: EngineFile::from_runtime(engine),
                max_acceleration_g: limit(engine, *max_acceleration_m_s2),
            },
        };
        let Color { r, g, b, a } = craft.color;
        Self {
            name: craft.name.clone(),
            mass_kg: craft.mass_kg,
            radius_m: craft.radius_m,
            color: [r, g, b, a],
            launch_altitude_m: craft.launch_altitude_m,
            propulsion,
        }
    }
}

impl SpacecraftFile {
    /// The file that describes `fleet`, for saving.
    pub fn from_runtime(fleet: &Fleet) -> Self {
        Self {
            craft: fleet.craft().iter().map(CraftFile::from_runtime).collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// controls.toml
// ---------------------------------------------------------------------------------------------

/// `controls.toml`: `[keys]`, an entry per action (one left out keeps its default key), by its name in
/// [`crate::constants::ACTIONS`], with Bevy's name for the key (`pause = "KeyK"`).
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ControlsFile {
    /// Action name to key name.
    pub keys: BTreeMap<String, String>,
}

impl ControlsFile {
    /// The file that describes `keymap`, for saving. Every action is written, so the saved file
    /// is a complete list of what each key does.
    pub fn from_runtime(keymap: &KeyMap) -> Self {
        Self {
            keys: Action::ALL
                .into_iter()
                .map(|action| (action.info().name.to_string(), key_name(keymap.key(action))))
                .collect(),
        }
    }

    /// The [`KeyMap`] this file describes, or a message naming the bad entry.
    pub fn into_runtime(self) -> Result<KeyMap, String> {
        let overrides = self
            .keys
            .iter()
            .map(|(name, key)| {
                let action = Action::from_name(name)
                    .ok_or_else(|| format!("`keys.{name}` is not an action that can be bound"))?;
                let key = parse_key(key).ok_or_else(|| {
                    format!("`keys.{name}`: {key:?} is not a key that can be bound")
                })?;
                Ok((action, key))
            })
            .collect::<Result<Vec<_>, String>>()?;
        KeyMap::with_overrides(overrides)
    }
}
