//! Tests for [`crate::compute::device`] that open a GPU, `#[path]`-included into it when the
//! `gpu-tests` feature is on.

use super::*;

#[test]
fn a_device_found_describes_itself() {
    let gpu = GpuDevice::request().expect("a GPU");
    let description = gpu.describe();
    assert!(description.contains(&gpu.info.name), "{description}");
}
