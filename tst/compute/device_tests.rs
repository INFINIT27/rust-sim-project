//! Tests for [`crate::compute::device`], `#[path]`-included into it.
//!
//! Finding a GPU depends on the machine: that is tested in `device_gpu_tests.rs`.

use super::*;

#[test]
fn the_switch_is_on_unless_it_says_off() {
    for on in [None, Some("on"), Some("1"), Some("yes"), Some("anything")] {
        assert!(gpu_physics_enabled_by(on), "{on:?}");
    }
    for off in ["off", "OFF", " 0 ", "false", "No"] {
        assert!(!gpu_physics_enabled_by(Some(off)), "{off}");
    }
}
