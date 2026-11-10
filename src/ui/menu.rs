//! The menu panel: what `Esc` puts on screen.
//!
//! Two halves in two layers, and this is the second one. [`crate::input::menu`] owns whether the
//! menu is open, which page it shows, and the keys (including the capture of a key being
//! rebound); this module draws the pages and edits the settings they show.
//!
//! # The pages
//!
//! - **Main**, where the menu always opens: Settings, Configuration, Exit (which quits the app),
//!   and a read-only summary of the run ([`settings_rows`]).
//! - **Settings**, with two tabs. *Controls* lists every binding ([`crate::bindings::binding_rows`]);
//!   a rebindable one is a button, and clicking it waits for the next key. *Video* has the
//!   anti-aliasing mode, vsync, and the floor's eclipse shadows.
//! - **Configuration**, one tab per configuration file: camera, interface (crosshair, compass,
//!   arrival pop-up), trails, and the spacecraft fleet.
//!
//! Every editing page has **Reset**, which puts that page's settings back to the built-in
//! defaults, and **Save**, which writes that page's file in `config/`. Changes apply as they are
//! made; Save only makes them survive a restart. Saving is done by the `config` layer, which this
//! one may not import: the page posts a [`SaveTarget`] in [`MenuRequests`] and shows the
//! [`MenuRequests::status`] it gets back.
//!
//! # Edits are written back only when they change something
//!
//! Each page edits a copy of its resource and assigns it back only if it differs. Handing egui the
//! resource itself would mark it changed every frame the page is open, and the systems that apply
//! a change (the camera's tuning, the anti-aliasing mode) would redo their work every frame.
//!
//! # Interactive widgets are safe here
//!
//! Every other module in this layer is careful *not* to claim input, because the camera needs it.
//! The menu takes clicks and keys, and that is safe because **while the menu is open, every other
//! binding is inert** (amendment A13): the two are never live at the same time.
//!
//! # Testing
//!
//! `EguiContexts::ctx_mut` needs a real window, so the painting is not tested headlessly. What
//! the pages show comes from pure functions ([`settings_rows`], [`crate::bindings::binding_rows`],
//! [`anti_aliasing_label`]) and what they change is plain data, both of which are tested. See
//! `tst/ui/README.md`.

use bevy::app::{App, AppExit, Plugin, Update};
use bevy::ecs::event::EventWriter;
use bevy::ecs::system::{Res, ResMut, Resource, SystemParam};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::bindings::{binding_rows, BindingGroup, KeyMap};
use crate::constants::METRES_PER_UNIT;
use crate::experiments::catalog::ExperimentSpec;
use crate::input::camera::CameraSettings;
use crate::input::menu::{ConfigTab, MenuPage, MenuState, SettingsTab};
use crate::performance::{AntiAliasing, PerformanceSettings};
use crate::physics_utils::physics_constants::STANDARD_GRAVITY_M_S2;
use crate::render::mission::Mission;
use crate::render::trail::TrailConfig;
use crate::spacecraft::{Fleet, Propulsion};
use crate::time::simulation::SimulationClock;
use crate::ui::arrival::ArrivalPopupConfig;
use crate::ui::compass::CompassConfig;
use crate::ui::constants::{
    MENU_KEY_COLUMN_PX, MENU_MAIN_BUTTON_WIDTH_PX, MENU_SCROLL_HEIGHT_PX, MENU_SECTION_GAP_PX,
    MENU_WIDTH_PX,
};
use crate::ui::crosshair::CrosshairConfig;

/// One row of the settings list: a label and its current value.
///
/// A pair of owned `String`s rather than `&'static str`, because the values are formatted
/// numbers — the point of the list is that it shows what the app is doing *now*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsRow {
    /// What the setting is called.
    pub label: &'static str,
    /// Its current value, formatted for display.
    pub value: String,
}

/// The settings list: what is running, how fast, and what is being drawn.
///
/// Pure, so the whole content of the panel is testable without a window. Reads the same
/// resources the app runs on, so a row cannot disagree with reality.
///
/// # Why these rows
///
/// They are the answers to the questions the overlay does not already answer, plus the two
/// a user is most likely to have changed by accident with a stray keypress. `Time scale` and
/// `Simulation` are here because `]`, `[`, and `P` are easy to hit and their effect is
/// invisible in a still frame; `Trails` because `T` is a toggle with no other indicator;
/// `Seed` because it is what makes a run repeatable and it is otherwise only in the overlay.
///
/// # Examples
///
/// ```
/// use physics_sandbox::{settings_rows, ExperimentId, SimulationClock, TrailConfig};
///
/// let spec = ExperimentId::TwoBody.build_default().spec;
/// let clock = SimulationClock::new(spec.time_scale);
/// let rows = settings_rows(&spec, &clock, &TrailConfig::default());
///
/// assert!(rows.iter().any(|row| row.label == "Experiment"));
/// assert!(rows.iter().any(|row| row.value == "Running"));
/// ```
pub fn settings_rows(
    spec: &ExperimentSpec,
    clock: &SimulationClock,
    trails: &TrailConfig,
) -> Vec<SettingsRow> {
    let row = |label: &'static str, value: String| SettingsRow { label, value };

    vec![
        row(
            "Experiment",
            format!("{} ({})", spec.id.name(), spec.id.slug()),
        ),
        row("Seed", format!("{}", spec.seed)),
        row(
            "Time scale",
            format!("{:.3e} simulated seconds per real second", clock.scale()),
        ),
        row(
            "Simulation",
            if clock.is_held() {
                "Held for a flight plan".to_string()
            } else if clock.is_paused() {
                "Paused".to_string()
            } else {
                "Running".to_string()
            },
        ),
        row(
            "Elapsed",
            format!("{:.3e} simulated seconds", clock.elapsed_seconds()),
        ),
        row(
            "Trails",
            if trails.visible {
                format!("On, {} units of path", trails.length_units)
            } else {
                "Off".to_string()
            },
        ),
        row("Unit scale", format!("1 unit = {:.0e} m", METRES_PER_UNIT)),
    ]
}

/// A page whose settings the menu can save: one configuration file each.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SaveTarget {
    /// The key bindings: `controls.toml`.
    Controls,
    /// The video settings: `performance.toml`.
    Video,
    /// `camera.toml`.
    Camera,
    /// `ui.toml`.
    Interface,
    /// `trails.toml`.
    Trails,
    /// `spacecraft.toml`.
    Spacecraft,
}

/// What the menu has asked of the layer that writes files, and what it heard back.
///
/// The menu sets a save request; `config::save_requested_system` takes it, writes the file, and
/// sets the status the menu then shows under its buttons.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct MenuRequests {
    save: Option<SaveTarget>,
    status: Option<String>,
}

impl MenuRequests {
    /// Ask for `target`'s file to be saved.
    pub fn request_save(&mut self, target: SaveTarget) {
        self.save = Some(target);
        self.status = None;
    }

    /// The save asked for, if any, clearing it. Taken by whoever writes the file.
    pub fn take_save(&mut self) -> Option<SaveTarget> {
        self.save.take()
    }

    /// Report the outcome of the last save.
    pub fn set_status(&mut self, status: String) {
        self.status = Some(status);
    }

    /// The outcome of the last save, for the menu to show.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Forget the last outcome, as moving to another page does.
    pub fn clear_status(&mut self) {
        self.status = None;
    }
}

/// How an anti-aliasing mode is named on the video page.
pub fn anti_aliasing_label(mode: AntiAliasing) -> &'static str {
    match mode {
        AntiAliasing::Msaa4 => "MSAA 4x (sharpest, default)",
        AntiAliasing::Msaa2 => "MSAA 2x",
        AntiAliasing::Smaa => "SMAA (post-process)",
        AntiAliasing::Fxaa => "FXAA (fastest post-process)",
        AntiAliasing::Off => "Off",
    }
}

/// Every anti-aliasing mode, in the order the video page lists them.
const ANTI_ALIASING_MODES: [AntiAliasing; 5] = [
    AntiAliasing::Msaa4,
    AntiAliasing::Msaa2,
    AntiAliasing::Smaa,
    AntiAliasing::Fxaa,
    AntiAliasing::Off,
];

/// Every resource the menu reads or edits. Grouped so the system stays within Bevy's and clippy's
/// argument limits. The video settings and the mission are optional: an app without
/// `PerformancePlugin` or `MissionPlugin` still has a menu, without those pages' contents.
#[derive(SystemParam)]
pub struct MenuResources<'w> {
    spec: Res<'w, ExperimentSpec>,
    clock: Res<'w, SimulationClock>,
    keymap: ResMut<'w, KeyMap>,
    trails: ResMut<'w, TrailConfig>,
    camera: ResMut<'w, CameraSettings>,
    crosshair: ResMut<'w, CrosshairConfig>,
    compass: ResMut<'w, CompassConfig>,
    arrival: ResMut<'w, ArrivalPopupConfig>,
    performance: Option<ResMut<'w, PerformanceSettings>>,
    mission: Option<ResMut<'w, Mission>>,
    requests: ResMut<'w, MenuRequests>,
}

/// Edit a copy of `value` with `edit`, and write it back only if it changed, so a page that is
/// merely open does not mark its resource changed every frame.
fn edit_copy<T: Resource + Clone + PartialEq>(value: &mut ResMut<T>, edit: impl FnOnce(&mut T)) {
    let mut copy = (**value).clone();
    edit(&mut copy);
    if copy != **value {
        **value = copy;
    }
}

/// Paint the menu, when it is open.
///
/// Draws nothing at all while [`MenuState::is_open`] is `false`, which is the common case.
///
/// # Panics
///
/// Inherits `EguiContexts::ctx_mut()`'s requirement that the primary window already has an
/// egui context, so this system needs a real windowed app. See `tst/ui/README.md`.
pub fn menu_system(
    mut contexts: EguiContexts,
    mut menu: ResMut<MenuState>,
    mut resources: MenuResources,
    mut exit: EventWriter<AppExit>,
) {
    if !menu.is_open() {
        return;
    }

    let ctx = contexts.ctx_mut();
    let page = menu.page();
    let title = match page {
        MenuPage::Main => "Menu",
        MenuPage::Settings(_) => "Settings",
        MenuPage::Configuration(_) => "Configuration",
    };

    egui::Window::new(title)
        .id(egui::Id::new("physics_sandbox_menu"))
        .default_width(MENU_WIDTH_PX)
        .collapsible(false)
        .resizable(false)
        // Anchored to the centre: this is a modal panel, not a tool window, and the corners
        // are taken by the overlay (top-left) and the compass (top-right).
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| match page {
            MenuPage::Main => main_page(ui, &mut menu, &resources, &mut exit),
            MenuPage::Settings(tab) => {
                let next = sub_page_header(
                    ui,
                    &[
                        ("Controls", MenuPage::Settings(SettingsTab::Controls)),
                        ("Video", MenuPage::Settings(SettingsTab::Video)),
                    ],
                    page,
                );
                body(ui, |ui| match tab {
                    SettingsTab::Controls => controls_tab(ui, &mut menu, &mut resources),
                    SettingsTab::Video => video_tab(ui, &mut resources),
                });
                footer(ui, &mut resources.requests);
                if let Some(next) = next {
                    resources.requests.clear_status();
                    menu.show(next);
                }
            }
            MenuPage::Configuration(tab) => {
                let next = sub_page_header(
                    ui,
                    &[
                        ("Camera", MenuPage::Configuration(ConfigTab::Camera)),
                        ("Interface", MenuPage::Configuration(ConfigTab::Interface)),
                        ("Trails", MenuPage::Configuration(ConfigTab::Trails)),
                        ("Spacecraft", MenuPage::Configuration(ConfigTab::Spacecraft)),
                    ],
                    page,
                );
                body(ui, |ui| match tab {
                    ConfigTab::Camera => camera_tab(ui, &mut resources),
                    ConfigTab::Interface => interface_tab(ui, &mut resources),
                    ConfigTab::Trails => trails_tab(ui, &mut resources),
                    ConfigTab::Spacecraft => spacecraft_tab(ui, &mut resources),
                });
                footer(ui, &mut resources.requests);
                if let Some(next) = next {
                    resources.requests.clear_status();
                    menu.show(next);
                }
            }
        });
}

/// The first page: the way into everything else, Exit, and what the run is doing.
fn main_page(
    ui: &mut egui::Ui,
    menu: &mut MenuState,
    resources: &MenuResources,
    exit: &mut EventWriter<AppExit>,
) {
    let size = egui::vec2(MENU_MAIN_BUTTON_WIDTH_PX, 0.0);
    ui.vertical_centered(|ui| {
        if ui
            .add(egui::Button::new("Settings").min_size(size))
            .clicked()
        {
            menu.show(MenuPage::Settings(SettingsTab::default()));
        }
        if ui
            .add(egui::Button::new("Configuration").min_size(size))
            .clicked()
        {
            menu.show(MenuPage::Configuration(ConfigTab::default()));
        }
        if ui.add(egui::Button::new("Exit").min_size(size)).clicked() {
            exit.send(AppExit::Success);
        }
    });

    ui.add_space(MENU_SECTION_GAP_PX);
    ui.separator();
    ui.label(egui::RichText::new("This run").strong());
    egui::Grid::new("menu_settings")
        .num_columns(2)
        .min_col_width(MENU_KEY_COLUMN_PX)
        .show(ui, |ui| {
            let rows = settings_rows(&resources.spec, &resources.clock, &resources.trails);
            for row in rows {
                ui.label(row.label);
                ui.label(row.value);
                ui.end_row();
            }
        });
    ui.add_space(MENU_SECTION_GAP_PX);
    ui.label(egui::RichText::new("Esc closes the menu.").weak());
}

/// A sub-page's top row: Back, then one tab per entry. Returns the page to go to, if one was
/// clicked.
fn sub_page_header(
    ui: &mut egui::Ui,
    tabs: &[(&str, MenuPage)],
    current: MenuPage,
) -> Option<MenuPage> {
    let mut next = None;
    ui.horizontal(|ui| {
        if ui.button("< Back").clicked() {
            next = Some(MenuPage::Main);
        }
        ui.separator();
        for (label, page) in tabs {
            if ui.selectable_label(current == *page, *label).clicked() && current != *page {
                next = Some(*page);
            }
        }
    });
    ui.separator();
    next
}

/// A page's contents, in a scroll area so a long page fits a small window.
fn body(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical()
        .max_height(MENU_SCROLL_HEIGHT_PX)
        .auto_shrink([false, true])
        .show(ui, contents);
}

/// Reset and Save, for the page that drew them. Returns `(reset, save)` clicks.
fn reset_and_save(ui: &mut egui::Ui) -> (bool, bool) {
    ui.add_space(MENU_SECTION_GAP_PX);
    ui.separator();
    ui.horizontal(|ui| {
        let reset = ui
            .button("Reset to defaults")
            .on_hover_text("Put this page back to the built-in values")
            .clicked();
        let save = ui
            .button("Save")
            .on_hover_text("Write this page to its file in config/, so it survives a restart")
            .clicked();
        (reset, save)
    })
    .inner
}

/// The outcome of the last save, under every sub-page.
fn footer(ui: &mut egui::Ui, requests: &mut MenuRequests) {
    if let Some(status) = requests.status() {
        ui.label(egui::RichText::new(status).weak());
    }
}

/// One labelled number in a two-column grid.
fn number_row<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    speed: f64,
) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).range(range).speed(speed));
    ui.end_row();
}

fn settings_grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .min_col_width(MENU_KEY_COLUMN_PX)
        .show(ui, rows);
}

/// Settings > Controls: every binding, the rebindable ones as buttons.
fn controls_tab(ui: &mut egui::Ui, menu: &mut MenuState, resources: &mut MenuResources) {
    ui.label(
        egui::RichText::new(
            "Click a key, then press the new one. Esc cancels. A key already in use swaps.",
        )
        .weak(),
    );
    let rows = binding_rows(&resources.keymap);
    for group in BindingGroup::ALL {
        ui.add_space(0.5 * MENU_SECTION_GAP_PX);
        ui.label(egui::RichText::new(group.title()).strong());
        egui::Grid::new(format!("menu_bindings_{}", group.title()))
            .num_columns(2)
            .min_col_width(MENU_KEY_COLUMN_PX)
            .show(ui, |ui| {
                for row in rows.iter().filter(|row| row.group == group) {
                    match row.rebind {
                        Some(action) => {
                            let waiting = menu.rebinding() == Some(action);
                            let text = if waiting {
                                "press a key...".to_string()
                            } else {
                                row.keys.clone()
                            };
                            let button = egui::Button::new(egui::RichText::new(text).monospace())
                                .selected(waiting);
                            if ui.add(button).clicked() {
                                if waiting {
                                    menu.cancel_rebinding();
                                } else {
                                    menu.start_rebinding(action);
                                }
                            }
                        }
                        None => {
                            ui.label(egui::RichText::new(&row.keys).monospace().weak());
                        }
                    }
                    ui.label(row.action);
                    ui.end_row();
                }
            });
    }
    let (reset, save) = reset_and_save(ui);
    if reset {
        menu.cancel_rebinding();
        if *resources.keymap != KeyMap::default() {
            resources.keymap.reset();
        }
    }
    if save {
        resources.requests.request_save(SaveTarget::Controls);
    }
}

/// Settings > Video: anti-aliasing, vsync, and the floor's eclipse shadows.
fn video_tab(ui: &mut egui::Ui, resources: &mut MenuResources) {
    let Some(performance) = resources.performance.as_mut() else {
        ui.label("Video settings are not available in this app.");
        return;
    };
    edit_copy(performance, |settings| {
        settings_grid(ui, "menu_video", |ui| {
            ui.label("Anti-aliasing");
            egui::ComboBox::from_id_source("menu_anti_aliasing")
                .selected_text(anti_aliasing_label(settings.anti_aliasing))
                .show_ui(ui, |ui| {
                    for mode in ANTI_ALIASING_MODES {
                        ui.selectable_value(
                            &mut settings.anti_aliasing,
                            mode,
                            anti_aliasing_label(mode),
                        );
                    }
                });
            ui.end_row();
            ui.label("Vsync");
            ui.checkbox(&mut settings.vsync, "Wait for the display's refresh");
            ui.end_row();
            ui.label("Floor shadows");
            ui.checkbox(&mut settings.floor_eclipse, "Eclipse shadows on the floor");
            ui.end_row();
        });
    });
    let (reset, save) = reset_and_save(ui);
    if reset {
        // The benchmark is a run, not a setting, so a reset leaves it as it was.
        edit_copy(performance, |settings| {
            *settings = PerformanceSettings {
                bench: settings.bench,
                ..PerformanceSettings::default()
            };
        });
    }
    if save {
        resources.requests.request_save(SaveTarget::Video);
    }
}

/// Configuration > Camera: `camera.toml`.
fn camera_tab(ui: &mut egui::Ui, resources: &mut MenuResources) {
    edit_copy(&mut resources.camera, |camera| {
        settings_grid(ui, "menu_camera", |ui| {
            number_row(
                ui,
                "Speed (units/s)",
                &mut camera.move_speed,
                0.001..=1.0e6,
                0.5,
            );
            number_row(
                ui,
                "Fast multiplier",
                &mut camera.boost_mult,
                0.001..=1.0e4,
                0.1,
            );
            number_row(
                ui,
                "Slow multiplier",
                &mut camera.precision_mult,
                0.0001..=1.0,
                0.001,
            );
            number_row(
                ui,
                "Look (rad / px)",
                &mut camera.look_sensitivity,
                0.00001..=0.1,
                0.0001,
            );
        });
    });
    let (reset, save) = reset_and_save(ui);
    if reset {
        edit_copy(&mut resources.camera, |camera| {
            *camera = CameraSettings::default()
        });
    }
    if save {
        resources.requests.request_save(SaveTarget::Camera);
    }
}

/// Configuration > Interface: `ui.toml`.
fn interface_tab(ui: &mut egui::Ui, resources: &mut MenuResources) {
    ui.label(egui::RichText::new("Crosshair").strong());
    edit_copy(&mut resources.crosshair, |crosshair| {
        settings_grid(ui, "menu_crosshair", |ui| {
            number_row(
                ui,
                "Arm length (px)",
                &mut crosshair.arm_length_px,
                0.0..=200.0,
                0.2,
            );
            number_row(
                ui,
                "Centre gap (px)",
                &mut crosshair.gap_px,
                0.0..=200.0,
                0.2,
            );
            number_row(
                ui,
                "Thickness (px)",
                &mut crosshair.thickness_px,
                0.1..=20.0,
                0.1,
            );
            ui.label("Colour");
            ui.color_edit_button_srgba_unmultiplied(&mut crosshair.color);
            ui.end_row();
        });
    });
    ui.add_space(MENU_SECTION_GAP_PX);
    ui.label(egui::RichText::new("Compass").strong());
    edit_copy(&mut resources.compass, |compass| {
        settings_grid(ui, "menu_compass", |ui| {
            number_row(
                ui,
                "Axis length (px)",
                &mut compass.radius_px,
                1.0..=400.0,
                0.5,
            );
            number_row(
                ui,
                "Corner margin (px)",
                &mut compass.margin_px,
                0.0..=400.0,
                0.5,
            );
            number_row(
                ui,
                "Thickness (px)",
                &mut compass.thickness_px,
                0.1..=20.0,
                0.1,
            );
            number_row(
                ui,
                "Label size (px)",
                &mut compass.label_size_px,
                1.0..=72.0,
                0.2,
            );
        });
    });
    ui.add_space(MENU_SECTION_GAP_PX);
    ui.label(egui::RichText::new("Arrival pop-up").strong());
    edit_copy(&mut resources.arrival, |arrival| {
        settings_grid(ui, "menu_arrival", |ui| {
            number_row(ui, "Width (px)", &mut arrival.width_px, 100.0..=2000.0, 1.0);
            number_row(
                ui,
                "Label column (px)",
                &mut arrival.label_column_px,
                20.0..=1000.0,
                1.0,
            );
        });
    });
    let (reset, save) = reset_and_save(ui);
    if reset {
        edit_copy(&mut resources.crosshair, |c| {
            *c = CrosshairConfig::default()
        });
        edit_copy(&mut resources.compass, |c| *c = CompassConfig::default());
        edit_copy(&mut resources.arrival, |c| {
            *c = ArrivalPopupConfig::default()
        });
    }
    if save {
        resources.requests.request_save(SaveTarget::Interface);
    }
}

/// Configuration > Trails: `trails.toml`.
fn trails_tab(ui: &mut egui::Ui, resources: &mut MenuResources) {
    edit_copy(&mut resources.trails, |trails| {
        settings_grid(ui, "menu_trails", |ui| {
            ui.label("Shown");
            ui.checkbox(&mut trails.visible, "Draw motion trails");
            ui.end_row();
            number_row(
                ui,
                "Length (units)",
                &mut trails.length_units,
                0.001..=1.0e5,
                0.1,
            );
            number_row(
                ui,
                "Point spacing (units)",
                &mut trails.min_spacing_units,
                0.0001..=1.0e3,
                0.001,
            );
            number_row(ui, "Most points", &mut trails.max_samples, 2..=100_000, 1.0);
            ui.label("Opacity at the body");
            ui.add(egui::Slider::new(&mut trails.head_alpha, 0.0..=1.0));
            ui.end_row();
            ui.label("Opacity at the end");
            ui.add(egui::Slider::new(&mut trails.tail_alpha, 0.0..=1.0));
            ui.end_row();
        });
    });
    ui.label(
        egui::RichText::new("An experiment that sets its own trail length overrides it at launch.")
            .weak(),
    );
    let (reset, save) = reset_and_save(ui);
    if reset {
        edit_copy(&mut resources.trails, |t| *t = TrailConfig::default());
    }
    if save {
        resources.requests.request_save(SaveTarget::Trails);
    }
}

/// Configuration > Spacecraft: `spacecraft.toml`.
fn spacecraft_tab(ui: &mut egui::Ui, resources: &mut MenuResources) {
    let Some(mission) = resources.mission.as_mut() else {
        ui.label("This app flies no spacecraft.");
        return;
    };
    let g = STANDARD_GRAVITY_M_S2;
    let mut fleet = mission.fleet.clone();
    let selected = fleet.selected_index();
    let mut select = None;
    for (index, craft) in fleet.craft_mut().iter_mut().enumerate() {
        egui::CollapsingHeader::new(&craft.name)
            .id_source(("menu_craft", index))
            .default_open(index == selected)
            .show(ui, |ui| {
                if ui
                    .radio(index == selected, "Selected for the next launch")
                    .clicked()
                {
                    select = Some(index);
                }
                settings_grid(ui, &format!("menu_craft_{index}"), |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut craft.name);
                    ui.end_row();
                    number_row(ui, "Mass (kg)", &mut craft.mass_kg, 1.0..=1.0e9, 10.0);
                    number_row(ui, "Radius (m)", &mut craft.radius_m, 0.01..=1.0e4, 0.1);
                    number_row(
                        ui,
                        "Launch altitude (m)",
                        &mut craft.launch_altitude_m,
                        0.0..=1.0e9,
                        1_000.0,
                    );
                    ui.label("Colour");
                    let mut rgba = [craft.color.r, craft.color.g, craft.color.b, craft.color.a];
                    if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                        let [r, g, b, a] = rgba;
                        craft.color = crate::core::scene::Color { r, g, b, a };
                    }
                    ui.end_row();
                    match &mut craft.propulsion {
                        Propulsion::Coasting {
                            max_departure_speed_m_s,
                        } => {
                            ui.label("Flies");
                            ui.label("Coasting (no engine)");
                            ui.end_row();
                            number_row(
                                ui,
                                "Top launch speed (m/s)",
                                max_departure_speed_m_s,
                                1.0..=3.0e8,
                                10.0,
                            );
                        }
                        Propulsion::Direct {
                            engine,
                            max_acceleration_m_s2,
                        }
                        | Propulsion::Transfer {
                            engine,
                            max_acceleration_m_s2,
                        } => {
                            ui.label("Engine");
                            ui.text_edit_singleline(&mut engine.name);
                            ui.end_row();
                            number_row(ui, "Thrust (N)", &mut engine.thrust_n, 1.0..=1.0e12, 1.0e4);
                            let mut isp = engine.exhaust_speed_m_s / g;
                            number_row(ui, "Specific impulse (s)", &mut isp, 1.0..=1.0e7, 1.0);
                            engine.exhaust_speed_m_s = isp * g;
                            let mut limit_g = *max_acceleration_m_s2 / g;
                            number_row(
                                ui,
                                "Acceleration limit (g)",
                                &mut limit_g,
                                0.001..=1.0e6,
                                0.05,
                            );
                            *max_acceleration_m_s2 = limit_g * g;
                        }
                    }
                });
            });
    }
    if let Some(index) = select {
        fleet.select(index);
    }
    // The ranges above keep every number valid; a name may be anything but empty.
    if fleet != mission.fleet
        && fleet
            .craft()
            .iter()
            .all(|craft| !craft.name.trim().is_empty())
    {
        mission.fleet = fleet;
    }
    let (reset, save) = reset_and_save(ui);
    if reset && mission.fleet != Fleet::default() {
        mission.fleet = Fleet::default();
    }
    if save {
        resources.requests.request_save(SaveTarget::Spacecraft);
    }
}

/// Draws the menu when `Esc` has opened it.
///
/// Inserts the settings it edits with their defaults when nothing has yet (`init_resource` keeps
/// a value already there, such as one loaded from `config/`), so it runs in any app. Adds
/// `EguiPlugin` if it is not already present.
///
/// # Examples
///
/// ```no_run
/// use bevy::prelude::*;
/// use physics_sandbox::{MenuInputPlugin, MenuPanelPlugin, ScenePlugin};
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(ScenePlugin::default())
///     .add_plugins(MenuInputPlugin)
///     .add_plugins(MenuPanelPlugin)
///     .run();
/// ```
pub struct MenuPanelPlugin;

impl Plugin for MenuPanelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin);
        }

        app.init_resource::<MenuState>()
            .init_resource::<MenuRequests>()
            .init_resource::<KeyMap>()
            .init_resource::<TrailConfig>()
            .init_resource::<CameraSettings>()
            .init_resource::<CrosshairConfig>()
            .init_resource::<CompassConfig>()
            .init_resource::<ArrivalPopupConfig>()
            .add_event::<AppExit>()
            .add_systems(Update, menu_system);
    }
}

#[cfg(test)]
#[path = "../../tst/ui/menu_tests.rs"]
mod tests;
