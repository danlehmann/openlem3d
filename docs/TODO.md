# To do

In order.

1. **Finish replacing Bevy.** The game now runs on `l3d-engine` (app,
   window, input, time, states, pictures, sound) and uses Bevy only for its
   ECS (`bevy_ecs`). Replace that with plain Rust, one module at a time.
2. **Z-fighting on floors.** In LEVEL.021 "WHICH EXIT ?" (camera 3, or low
   over the checkered floor), streaks of the colours underneath flicker
   through the floor. It predates the Bevy work. Likely two surfaces close
   together, with too little depth precision over the view's 0.05–8000
   range (`projection` in `crates/openlem3d/src/lib.rs`).
