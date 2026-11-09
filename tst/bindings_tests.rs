//! Unit tests for the [`crate::bindings`] module and the
//! [`KEY_BINDINGS`](crate::constants::KEY_BINDINGS) table it reads.
//!
//! Compiled as a private `#[cfg(test)] mod tests` inside `src/bindings.rs` via `#[path]`,
//! not as a standalone integration test.
//!
//! # What is checkable here, and what is not
//!
//! The table is *documentation*: nothing in the type system ties a row to the
//! `just_pressed` call that implements it, so no test here can prove the documented keys
//! are the real ones. What these tests do cover is everything that can be wrong *within*
//! the table — a duplicated key, an empty label, a group with a heading and no rows, a
//! renderer that drops a binding — plus the property that actually motivated putting the
//! table at the crate root: **`--help` and the menu draw from the same rows**, so they
//! cannot disagree.
//!
//! The complementary check lives in `tst/input/controls_tests.rs`, which asserts no two
//! *implementations* claim one key.

use super::*;

#[test]
fn every_row_is_renderable() {
    for binding in KEY_BINDINGS {
        assert!(
            binding.is_valid(),
            "{:?} has an empty key or action, which draws as an unexplained gap",
            binding
        );
    }
}

#[test]
fn no_two_rows_claim_the_same_key() {
    // A duplicated key string means the menu shows one key doing two things — either a
    // real collision in the code, or a stale row left behind by a rename. Both are worth
    // failing over.
    for (index, binding) in KEY_BINDINGS.iter().enumerate() {
        for other in &KEY_BINDINGS[index + 1..] {
            assert_ne!(
                binding.keys, other.keys,
                "`{}` appears twice: {:?} and {:?}",
                binding.keys, binding.action, other.action
            );
        }
    }
}

#[test]
fn every_group_has_at_least_one_binding() {
    // An empty group would render as a heading with nothing under it. `binding_help_text`
    // and `binding_rows` both skip empties, so this is about the *table* rather than the
    // rendering: a group nobody uses should be deleted from `BindingGroup`.
    for group in BindingGroup::ALL {
        assert!(
            bindings_in(group).next().is_some(),
            "{:?} has no bindings",
            group
        );
    }
}

#[test]
fn every_binding_belongs_to_a_listed_group() {
    // `BindingGroup::ALL` is what both renderers iterate, so a row in a group missing from
    // that array would be silently invisible in `--help` *and* in the menu.
    for binding in KEY_BINDINGS {
        assert!(
            BindingGroup::ALL.contains(&binding.group),
            "{:?} is in a group that is not in BindingGroup::ALL",
            binding
        );
    }
}

#[test]
fn the_groups_partition_the_table() {
    let counted: usize = BindingGroup::ALL
        .into_iter()
        .map(|group| bindings_in(group).count())
        .sum();

    assert_eq!(
        counted,
        KEY_BINDINGS.len(),
        "grouping must neither drop nor duplicate a row"
    );
}

#[test]
fn the_key_column_fits_the_longest_key() {
    let width = key_column_width();

    assert!(width > 0);
    for binding in KEY_BINDINGS {
        assert!(
            binding.keys.chars().count() <= width,
            "`{}` is wider than the column it is padded into",
            binding.keys
        );
    }
}

#[test]
fn the_help_text_lists_every_binding_under_its_own_heading() {
    // The property the whole module exists for: what `--help` prints is the table, in
    // full. A binding the terminal never mentions is a binding nobody finds.
    let text = binding_help_text();

    for binding in KEY_BINDINGS {
        assert!(
            text.contains(binding.keys),
            "--help does not mention `{}`",
            binding.keys
        );
        assert!(
            text.contains(binding.action),
            "--help does not explain `{}`",
            binding.keys
        );
    }

    for group in BindingGroup::ALL {
        assert!(
            text.contains(&format!("{}:", group.title())),
            "--help is missing the {:?} heading",
            group
        );
    }
}

#[test]
fn the_help_text_is_indented_and_newline_terminated() {
    let text = binding_help_text();

    assert!(text.ends_with('\n'), "a caller appends directly after this");
    for line in text.lines() {
        assert!(
            line.starts_with("    "),
            "{line:?} would break out of the usage text's indentation"
        );
    }
}

#[test]
fn the_camera_group_comes_first_and_the_window_group_last() {
    // Display order, asserted because it is a judgement rather than an accident: movement
    // is what a new viewer needs to do anything, and quitting is what they need once.
    assert_eq!(BindingGroup::ALL.first(), Some(&BindingGroup::Camera));
    assert_eq!(BindingGroup::ALL.last(), Some(&BindingGroup::Window));
}

#[test]
fn escape_is_the_menu_key_and_only_the_menu_key() {
    // `Esc` used to release the cursor and nothing else; amendment A13 gave it the menu,
    // and the release became a consequence. If a second row ever claims `Esc`, the key has
    // two documented meanings and the menu is the one that will lose.
    let escape: Vec<_> = KEY_BINDINGS
        .iter()
        .filter(|binding| binding.keys == "Esc")
        .collect();

    assert_eq!(escape.len(), 1);
    assert_eq!(escape[0].group, BindingGroup::Menu);
    assert!(
        escape[0].action.contains("cursor"),
        "the row must still say the cursor is released, since that is what it used to do"
    );
}

#[test]
fn the_bindings_the_help_text_promises_are_the_ones_the_menu_draws() {
    // The two consumers, compared directly. This is the test that would have caught the
    // duplication this module was created to remove.
    let from_menu: Vec<(&str, &str)> = crate::ui::menu::binding_rows()
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .collect();
    let from_table: Vec<(&str, &str)> = KEY_BINDINGS
        .iter()
        .map(|binding| (binding.keys, binding.action))
        .collect();

    assert_eq!(from_menu, from_table);
}
