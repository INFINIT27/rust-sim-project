//! Tests for [`crate::compute::device`], `#[path]`-included into it.
//!
//! The switch is pure and tested everywhere. Finding a GPU depends on the machine, so that test
//! only checks that whatever is found describes itself.

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

#[test]
fn a_device_found_describes_itself() {
    match GpuDevice::request() {
        Some(gpu) => {
            let description = gpu.describe();
            assert!(description.contains(&gpu.info.name), "{description}");
        }
        None => eprintln!("skipped: no GPU on this machine"),
    }
}
