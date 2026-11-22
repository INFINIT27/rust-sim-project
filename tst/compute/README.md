# `tst/compute/` — tests for the GPU physics

> **Purpose:** Tests for [`src/compute/`](../../src/compute/README.md): the GPU switch, finding a
> device, and the particle stepper held to the CPU path it mirrors, with every algorithm.
> **Edit here to:** add or fix a GPU physics test. Each `*_tests.rs` is `#[path]`-included into its
> subject.
> **Not here:** the code under test, in `src/compute/`.

| File | Subject | Shape of the tests |
| --- | --- | --- |
| `device_tests.rs` | `device.rs` | 2 unit tests: the `RGA_GPU_PHYSICS` switch is on unless it says off, and a device found describes itself. |
| `particles_tests.rs` | `particles.rs` | 7 unit tests and 2 measurements. The shader's workgroup is the constant; only scenes made entirely of particles go to the GPU, and one that cannot is left alone; the exact solver agrees with the CPU to the rounding over an hour of settling, heat and spin included; a contact on the GPU rebounds at its restitution; `experiment_1` with each of the four algorithm combinations meets on time, merges with nothing escaping, and holds energy within `1%` of the binding energy; and `experiment_8` keeps its angular momentum with the exact solver (`0.1%`) and with Barnes–Hut (`1%`, its forces not being equal and opposite). The ignored `measure_gpu_speed` and `measure_moon_forming_detail` print the tables in `src/compute/README.md` (`RGA_MEASURE_SIZES=2000,20000` sets the detail). |

## Machines without a GPU

A test that needs a GPU asks for one and, finding none, prints `skipped: no GPU on this machine`
and passes, so the suite runs anywhere. Set `RGA_REQUIRE_GPU` to make a missing GPU a failure.
CI installs Mesa's software Vulkan driver (`mesa-vulkan-drivers`, lavapipe) so these tests run
there on the CPU through Vulkan.
