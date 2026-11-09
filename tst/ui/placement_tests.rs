//! Unit tests for the [`crate::ui::placement`] module.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/ui/placement.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! Nothing here calls `placement_panel_system`: `EguiContexts::ctx_mut()` needs a real window,
//! so the panel's content lives in the pure [`placement_rows`] and the tests live there. Same
//! split as the overlay, crosshair, compass, and menu — see `tst/ui/README.md`.

use super::*;
use crate::experiments::constants::STELLAR_OBJECTS;

fn open_at(index: usize) -> PlacementState {
    let mut placer = PlacementState::default();
    placer.open();
    for _ in 0..index {
        placer.select_next();
    }
    placer
}

#[test]
fn every_object_gets_a_row() {
    // A picker that silently omitted an object would make it unreachable: the arrow keys walk
    // the table, but only the rows tell you what you are walking through.
    let rows = placement_rows(&PlacementState::default());

    assert_eq!(rows.len(), STELLAR_OBJECTS.len());
    for (row, object) in rows.iter().zip(STELLAR_OBJECTS) {
        assert_eq!(row.name, object.name);
    }
}

#[test]
fn exactly_one_row_is_selected() {
    // The marker is how you know what `Enter` will place. Two would be a lie and none would
    // leave the key unexplained.
    for index in 0..STELLAR_OBJECTS.len() {
        let rows = placement_rows(&open_at(index));

        assert_eq!(rows.iter().filter(|row| row.selected).count(), 1);
        assert!(
            rows[index].selected,
            "row {index} should be the selected one"
        );
    }
}

#[test]
fn the_selection_follows_the_state() {
    let first = placement_rows(&open_at(0));
    let second = placement_rows(&open_at(1));

    assert!(first[0].selected && !first[1].selected);
    assert!(second[1].selected && !second[0].selected);
}

#[test]
fn every_row_quotes_the_mass_and_the_note() {
    // The point of these objects *is* the numbers — 1.4 solar masses in a 22 km ball. A list
    // of five names would waste the only line each row gets.
    for row in placement_rows(&PlacementState::default()) {
        assert!(!row.name.trim().is_empty());
        assert!(row.detail.contains("M☉"), "{:?} has no mass", row.detail);
        assert!(row.detail.contains('—'), "{:?} has no note", row.detail);
    }
}

#[test]
fn the_masses_shown_are_the_catalog_masses() {
    // Formatted, but not invented: a row must not be able to disagree with the object it will
    // place. Parsed back out of the label rather than string-matched, because the format
    // switches to scientific notation above a thousand solar masses — see `solar_mass_label`.
    for (row, object) in placement_rows(&PlacementState::default())
        .iter()
        .zip(STELLAR_OBJECTS)
    {
        let shown: f64 = row
            .detail
            .split(" M☉")
            .next()
            .and_then(|number| number.parse().ok())
            .unwrap_or_else(|| panic!("{:?} does not open with a mass", row.detail));

        assert!(
            (shown / object.solar_masses() - 1.0).abs() < 1.0e-3,
            "{} shows {shown} M☉ against a catalog {}",
            object.name,
            object.solar_masses()
        );
    }
}

#[test]
fn the_rows_are_in_catalog_order() {
    // The arrow keys move by index, so display order has to match table order or `Down` would
    // appear to jump around.
    let names: Vec<&str> = placement_rows(&PlacementState::default())
        .into_iter()
        .map(|row| row.name)
        .collect();
    let expected: Vec<&str> = STELLAR_OBJECTS.iter().map(|object| object.name).collect();

    assert_eq!(names, expected);
}

#[test]
fn a_closed_picker_still_has_rows_to_draw() {
    // `placement_rows` is pure and says nothing about visibility — the *system* checks
    // `is_open`. Keeping the two separate is what makes the content testable at all.
    assert_eq!(
        placement_rows(&PlacementState::default()).len(),
        STELLAR_OBJECTS.len()
    );
}

#[test]
fn the_panel_is_wide_enough_for_its_longest_row() {
    use crate::ui::constants::PLACEMENT_PANEL_WIDTH_PX;

    // The same crude check the menu's width gets, and the same reason: at egui's default body
    // size a character is roughly 7 px, and a row that wraps turns a scannable list into a
    // paragraph. Sgr A*'s row is the long one, because its mass has seven digits.
    let longest = placement_rows(&PlacementState::default())
        .iter()
        .map(|row| row.name.chars().count() + row.detail.chars().count() + 4)
        .max()
        .unwrap_or(0);

    assert!(
        (longest as f32) * 7.0 <= PLACEMENT_PANEL_WIDTH_PX * 1.05,
        "the longest row is {longest} characters against {PLACEMENT_PANEL_WIDTH_PX} px"
    );
}
