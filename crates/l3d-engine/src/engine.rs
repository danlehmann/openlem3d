//! What the engine keeps for the game, and the game's side of the loop.
//! Each time round the runner takes in what happened to the window since
//! the last frame, moves the clocks on and calls [`Game::frame`]; the game
//! does the rest, in an order of its own.

use crate::asset::Assets;
use crate::audio::Audio;
use crate::image::Image;
use crate::input::Input;
use crate::time::Clocks;
use crate::window::{CursorIcon, Window, WindowHandle};

/// The window, its input, the clocks, the pictures and the sound output.
pub struct Engine {
    pub window: Window,
    /// The system's window, while it can be drawn into (Android takes it
    /// away while the app is in the background).
    pub handle: Option<WindowHandle>,
    /// Counts the handles given: a new one needs a new surface.
    pub handle_generation: u64,
    /// The pointer's look over the window.
    pub cursor: CursorIcon,
    pub input: Input,
    pub time: Clocks,
    pub images: Assets<Image>,
    pub audio: Audio,
    exit: bool,
}

impl Engine {
    pub fn new(window: Window) -> Self {
        crate::logging::init();
        Engine {
            window,
            handle: None,
            handle_generation: 0,
            cursor: CursorIcon::default(),
            input: Input::default(),
            time: Clocks::default(),
            images: Assets::default(),
            audio: Audio::open(),
            exit: false,
        }
    }

    /// Asks the app to quit after this frame.
    pub fn exit(&mut self) {
        self.exit = true;
    }

    pub(crate) fn should_exit(&self) -> bool {
        self.exit
    }
}

/// The game the runner runs.
pub trait Game: 'static {
    fn engine(&mut self) -> &mut Engine;

    /// Runs one frame: the input since the last one is in `engine().input`
    /// and the clocks have moved on.
    fn frame(&mut self);
}

/// Opens the window and runs `game` until it quits.
pub fn run<G: Game>(game: G) {
    crate::runner::run(game);
}
