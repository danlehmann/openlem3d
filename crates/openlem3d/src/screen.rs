//! The part of the window the game uses: all of it, except where the system
//! draws over it, such as the caption bar of an Android desktop window
//! (`platform::insets`). The camera's viewport covers just this part, so the
//! UI lays itself out inside it; code that measures the screen or places
//! things by the pointer uses [`Screen`] rather than the window.

use bevy::camera::Viewport;
use bevy::prelude::*;

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Screen>()
            .add_systems(First, (update, full_screen));
    }
}

/// The usable part of the window, in logical pixels.
#[derive(Resource, Clone, Copy)]
pub struct Screen {
    /// Its top-left corner in the window.
    pub min: Vec2,
    pub size: Vec2,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            min: Vec2::ZERO,
            size: Vec2::ONE,
        }
    }
}

impl Screen {
    /// A window position (the cursor's or a touch's) relative to the screen.
    pub fn on_screen(&self, p: Vec2) -> Vec2 {
        p - self.min
    }

    /// The cursor's position on the screen, if it is in the window.
    pub fn cursor(&self, window: &Window) -> Option<Vec2> {
        window.cursor_position().map(|p| self.on_screen(p))
    }
}

/// Fills the screen where the game has it to itself (see
/// `platform::full_screen`) whenever the window gains focus.
fn full_screen(mut focus: MessageReader<bevy::window::WindowFocused>) {
    if focus.read().any(|f| f.focused) {
        crate::platform::full_screen();
    }
}

/// How often to ask the system for the insets, seconds; they change only when
/// the window does, but asking is not free on Android.
const INSETS_POLL: f32 = 0.25;

fn update(
    time: Res<Time<Real>>,
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera>,
    mut screen: ResMut<Screen>,
    mut insets: Local<Option<(f32, [u32; 4])>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let now = time.elapsed_secs();
    if insets.is_none_or(|(at, _)| now - at >= INSETS_POLL) {
        *insets = Some((now, crate::platform::insets()));
    }
    let [left, top, right, bottom] = insets.map_or([0; 4], |(_, i)| i);
    let full = UVec2::new(window.physical_width(), window.physical_height());
    let min = UVec2::new(left, top).min(full);
    let size = (full - min)
        .saturating_sub(UVec2::new(right, bottom))
        .max(UVec2::ONE);
    let viewport = (min != UVec2::ZERO || size != full).then(|| Viewport {
        physical_position: min,
        physical_size: size,
        ..default()
    });
    for mut camera in &mut cameras {
        let same = match (&camera.viewport, &viewport) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.physical_position == b.physical_position && a.physical_size == b.physical_size
            }
            _ => false,
        };
        if !same {
            camera.viewport = viewport.clone();
        }
    }
    let scale = window.scale_factor();
    let new = Screen {
        min: min.as_vec2() / scale,
        size: size.as_vec2() / scale,
    };
    if new.min != screen.min || new.size != screen.size {
        *screen = new;
    }
}
