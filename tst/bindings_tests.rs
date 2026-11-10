//! Unit tests for the [`crate::bindings`] module and the tables it reads
//! ([`ACTIONS`](crate::constants::ACTIONS), [`FIXED_BINDINGS`](crate::constants::FIXED_BINDINGS),
//! [`BINDABLE_KEYS`](crate::constants::BINDABLE_KEYS)).
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/bindings.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! The input systems act through the [`KeyMap`], so the bindings shown are the bindings used.
//! What these tests cover is the tables and the map: one key per action, every row renderable,
//! `--help` and the menu drawn from the same rows, and a rebind that never leaves a key doing
//! two things.

use super::*;
use crate::constants::{ACTIONS, BINDABLE_KEYS, FIXED_BINDINGS};

#[test]
fn the_actions_table_is_in_the_order_of_the_enum() {
    // `Action::info` indexes the table by the variant's position.
    assert_eq!(ACTIONS.len(), Action::ALL.len());
    for (index, action) in Action::ALL.into_iter().enumerate() {
        assert_eq!(ACTIONS[index].action, action, "row {index}");
        assert_eq!(action.info().action, action);
    }
}

#[test]
fn every_action_has_a_unique_name_and_a_label() {
    for (index, info) in ACTIONS.iter().enumerate() {
        assert!(
            !info.label.trim().is_empty(),
            "{:?} has no label",
            info.action
        );
        assert!(!info.name.is_empty());
        assert_eq!(Action::from_name(info.name), Some(info.action));
        for other in &ACTIONS[index + 1..] {
            assert_ne!(info.name, other.name);
        }
    }
}

#[test]
fn no_two_actions_start_on_the_same_key() {
    let defaults = KeyMap::default();
    for (index, &first) in Action::ALL.iter().enumerate() {
        for &second in &Action::ALL[index + 1..] {
            assert_ne!(
                defaults.key(first),
                defaults.key(second),
                "{first:?} and {second:?} share a default key"
            );
        }
    }
}

#[test]
fn every_default_key_can_be_bound_and_escape_cannot() {
    for info in ACTIONS {
        assert!(is_bindable(info.default_key), "{:?}", info.default_key);
    }
    assert!(!is_bindable(KeyCode::Escape), "Esc is the menu's, always");
}

#[test]
fn every_fixed_row_is_renderable() {
    for binding in FIXED_BINDINGS {
        assert!(binding.is_valid(), "{binding:?}");
    }
}

#[test]
fn every_group_has_at_least_one_binding() {
    let rows = binding_rows(&KeyMap::default());
    for group in BindingGroup::ALL {
        assert!(
            rows.iter().any(|row| row.group == group),
            "{group:?} has no bindings"
        );
    }
}

#[test]
fn the_rows_list_every_binding_once_in_group_order() {
    let rows = binding_rows(&KeyMap::default());
    assert_eq!(rows.len(), ACTIONS.len() + FIXED_BINDINGS.len());
    let order: Vec<usize> = rows
        .iter()
        .map(|row| {
            BindingGroup::ALL
                .iter()
                .position(|g| *g == row.group)
                .unwrap()
        })
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] <= pair[1]));
    for action in Action::ALL {
        assert_eq!(
            rows.iter().filter(|row| row.rebind == Some(action)).count(),
            1,
            "{action:?}"
        );
    }
}

#[test]
fn the_rows_follow_a_rebind() {
    let mut keys = KeyMap::default();
    keys.set(Action::Pause, KeyCode::KeyK);
    let rows = binding_rows(&keys);
    let pause = rows
        .iter()
        .find(|row| row.rebind == Some(Action::Pause))
        .unwrap();
    assert_eq!(pause.keys, "K");
}

#[test]
fn a_rebind_onto_a_taken_key_swaps_rather_than_doubling_up() {
    let mut keys = KeyMap::default();
    let displaced = keys.set(Action::MoveForward, KeyCode::KeyS);
    assert_eq!(displaced, Some(Action::MoveBack));
    assert_eq!(keys.key(Action::MoveForward), KeyCode::KeyS);
    assert_eq!(keys.key(Action::MoveBack), KeyCode::KeyW);

    assert_eq!(
        keys.set(Action::MoveForward, KeyCode::KeyS),
        None,
        "already there"
    );
    keys.reset();
    assert_eq!(keys, KeyMap::default());
}

#[test]
fn a_key_map_reacts_to_its_own_keys_only() {
    let mut keys = KeyMap::default();
    keys.set(Action::Pause, KeyCode::KeyK);
    let mut input = ButtonInput::<KeyCode>::default();
    input.press(KeyCode::KeyP);
    assert!(
        !keys.just_pressed(&input, Action::Pause),
        "P no longer pauses"
    );
    input.press(KeyCode::KeyK);
    assert!(keys.just_pressed(&input, Action::Pause));
    assert!(keys.pressed(&input, Action::Pause));
}

#[test]
fn overrides_from_a_file_are_taken_as_given_and_a_clash_is_an_error() {
    let keys = KeyMap::with_overrides([(Action::Pause, KeyCode::KeyK)]).unwrap();
    assert_eq!(keys.key(Action::Pause), KeyCode::KeyK);
    assert_eq!(keys.key(Action::ToggleTrails), KeyCode::KeyT);

    let error = KeyMap::with_overrides([(Action::Pause, KeyCode::KeyT)]).unwrap_err();
    assert!(
        error.contains("pause") && error.contains("toggle_trails"),
        "{error}"
    );
}

#[test]
fn every_bindable_key_round_trips_through_its_name() {
    for key in BINDABLE_KEYS {
        assert_eq!(parse_key(&key_name(key)), Some(key), "{key:?}");
        assert!(!key_label(key).is_empty());
    }
    assert_eq!(parse_key("Escape"), None);
    assert_eq!(parse_key("NotAKey"), None);
}

#[test]
fn keys_read_the_way_people_write_them() {
    assert_eq!(key_label(KeyCode::KeyW), "W");
    assert_eq!(key_label(KeyCode::Digit3), "3");
    assert_eq!(key_label(KeyCode::ControlLeft), "Left Ctrl");
    assert_eq!(key_label(KeyCode::BracketRight), "]");
    assert_eq!(key_label(KeyCode::ArrowUp), "Up");
    assert_eq!(key_label(KeyCode::Numpad7), "Numpad 7");
    assert_eq!(key_label(KeyCode::Space), "Space");
}

#[test]
fn help_and_the_menu_draw_from_the_same_rows() {
    let text = binding_help_text();
    for row in binding_rows(&KeyMap::default()) {
        assert!(
            text.contains(row.action),
            "--help is missing {:?}",
            row.action
        );
        assert!(text.contains(&row.keys), "--help is missing {:?}", row.keys);
    }
    for group in BindingGroup::ALL {
        assert!(text.contains(&format!("{}:", group.title())));
    }
}
