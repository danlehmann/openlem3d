# winit 0.30.13, patched

A copy of [winit](https://github.com/rust-windowing/winit) 0.30.13 as
published on crates.io (Apache-2.0, see `LICENSE`), without its examples and
tests, used through `[patch.crates-io]` in the workspace `Cargo.toml`.

The only change is in `src/platform_impl/android/mod.rs` (marked "openlem3d
patch"): winit 0.30 reports every Android pointer event as a touch, so a mouse
loses its buttons, hovering and wheel. The patch reports mouse pointers as
`CursorMoved`, `MouseInput`, `MouseWheel`, `CursorEntered` and `CursorLeft`
instead, and their movement as `DeviceEvent::MouseMotion`.

Remove this copy once Bevy uses a winit that handles Android mice.
