//! Playing with a gamepad (ours): no pointer to steer. In a level the
//! original's highlight marks the lemming skills go to (the panel's arrow),
//! and the D-pad steps it from lemming to lemming on screen; the bumpers
//! choose the skill and A gives it. The sticks move the camera
//! (`camera_controls`). On the other screens the D-pad and the left stick
//! jump the pointer from button to button, A presses and B goes back.
//!
//! In a level: A gives the skill, B pauses, X rides along with the
//! highlighted lemming (and back), Y the next camera view; the D-pad's up
//! and down the release rate; the left trigger runs the level back and the
//! right one forward as fast as they are pulled (`backspace_rewind`); the
//! right stick's click highlights the lemming nearest the middle again;
//! View opens the options and Menu is Esc (the level's menu).

use l3d_engine::input::{KeyCode, MouseButton, PadButton};
use l3d_engine::math::Vec2;
use l3d_sim::Skill;

use crate::World;
use crate::hud::Highlight;
use crate::menu::AppState;
use crate::ui::{UiNode, px};
use l3d_engine::math::Rect;
use l3d_engine::prelude::Color;

/// How far a stick must be pushed to step to the next button, and how far
/// back to let it step again.
const STICK_STEP: f32 = 0.6;
const STICK_REST: f32 = 0.3;
/// The frame round the button the gamepad is on: above everything.
const FRAME_Z: i32 = 1000;
/// How long both bumpers must be held to nuke (seconds).
const NUKE_HOLD: f32 = 1.0;
/// Held, the D-pad (or a stick pushed) steps again after this long, then
/// this often (seconds): quickly down a list or through a screen's
/// settings. Android's own key repeat (`ViewConfiguration`'s key repeat
/// timeout and delay), so that holding feels as elsewhere on a TV.
const REPEAT_AFTER: f32 = 0.4;
const REPEAT_EVERY: f32 = 0.05;

/// What the gamepad pressed for the game, let go again.
#[derive(Default)]
pub struct GamepadState {
    /// When a replay was last running (real seconds), and whether the A
    /// that took over from it is still held: that A gives no skill.
    replay_seen: f32,
    a_took_over: bool,
    /// Keys pressed for a frame, let go the next.
    tapped: Vec<KeyCode>,
    /// Keys held while a button is: (button, key).
    held: Vec<(PadButton, KeyCode)>,
    /// The left mouse button pressed for a frame, on a button.
    clicked: bool,
    /// The left stick is back near the middle, so a push steps again.
    stick_ready: bool,
    /// The way held on a screen of buttons, and when it steps again.
    step_repeat: Option<(Vec2, f32)>,
    /// The gamepad has been used in this level: it keeps a lemming
    /// highlighted.
    playing: Option<u32>,
    /// The gamepad was used last, not the mouse.
    in_use: bool,
    /// The button the frame is on, by its middle (as `Ui::button_rects`),
    /// and the frame.
    focus: Option<Vec2>,
    frame: Option<crate::ui::NodeId>,
    /// The screen the gamepad was last on: a new one starts afresh.
    screen: Option<(AppState, bool)>,
    /// Both bumpers held to nuke: since when (real seconds), and the skill
    /// selected before the first of them stepped it.
    nuke_since: Option<f32>,
    before_bumper: Option<Option<Skill>>,
    /// Letters typed on the code screen, and the last one's.
    letters: usize,
    letter: u8,
    /// The last letter is still being chosen (up and down change it).
    choosing: bool,
    /// The way held on the D-pad or left stick in a list, and when it
    /// steps next (real seconds).
    repeat: Option<(i32, f32)>,
}

/// Whether the gamepad, rather than the mouse, was used last.
pub(crate) fn in_use(w: &World) -> bool {
    w.gamepad.in_use
}

/// Takes in the gamepad, after the frame's other input and before the
/// game's systems read it.
pub(crate) fn input(w: &mut World) {
    let pad = w.e.input.pad.clone();
    let g = &mut w.gamepad;
    let keys = &mut w.e.input.keys;
    for k in g.tapped.drain(..) {
        keys.release(k);
    }
    g.held.retain(|&(b, k)| {
        let on = pad.buttons.pressed(b);
        if !on {
            keys.release(k);
        }
        on
    });
    if std::mem::take(&mut g.clicked) {
        w.e.input.mouse.release(MouseButton::Left);
    }
    w.panel.nuke_held = false;
    if !pad.connected {
        w.gamepad.in_use = false;
        w.e.window.cursor_hidden = false;
        frame(w, None);
        return;
    }
    let menu = w.state != AppState::Playing || w.confirm.question.is_some();
    // The pointer is hidden while the gamepad is in use (a frame shows the
    // button it is on), until the mouse moves or clicks.
    let used = pad.buttons.get_just_pressed().len() > 0
        || pad.left != Vec2::ZERO
        || pad.right != Vec2::ZERO;
    let mouse = &w.e.input;
    if mouse.motion.delta != Vec2::ZERO || mouse.mouse.get_just_pressed().len() > 0 {
        w.gamepad.in_use = false;
    } else if used {
        w.gamepad.in_use = true;
    }
    w.e.window.cursor_hidden = w.gamepad.in_use;
    // Menu is Esc everywhere (owner: swapped with View, the options); B
    // goes back outside levels; View closes the options again.
    if pad.buttons.just_pressed(PadButton::Start)
        || (menu && w.state != AppState::Code && pad.buttons.just_pressed(PadButton::East))
        || (w.state == AppState::Options && pad.buttons.just_pressed(PadButton::Select))
    {
        tap(w, KeyCode::Escape);
    }
    if w.state != AppState::Code {
        (w.gamepad.letters, w.gamepad.choosing) = (0, false);
    }
    // The frame is on a button only where `buttons` puts it, this frame.
    frame(w, None);
    // The level's preview: its choices as on the briefing (starting on
    // Continue), B back to the list.
    if w.state == AppState::Playing && w.preview.active {
        if pad.buttons.just_pressed(PadButton::East) {
            tap(w, KeyCode::Escape);
        } else {
            buttons(w, &pad);
        }
        return;
    }
    // The level's menu (restart, carry on, leave): its choices framed,
    // starting on Cancel in the middle; B (Esc) carries on.
    if w.confirm.question.is_some() {
        buttons(w, &pad);
        // The sticks only work the menu, not the camera behind it, nor the
        // triggers the time.
        let pad = &mut w.e.input.pad;
        (pad.left, pad.right) = (Vec2::ZERO, Vec2::ZERO);
        (pad.left_trigger, pad.right_trigger) = (0.0, 0.0);
    } else if w.state == AppState::Menu {
        list(w, &pad);
    } else if w.state == AppState::Code {
        code(w, &pad);
    } else if w.state == AppState::Options {
        let way = stepped(w, &pad);
        let b = &pad.buttons;
        let slide = b.just_pressed(PadButton::RightBumper) as i32
            - b.just_pressed(PadButton::LeftBumper) as i32;
        crate::options::gamepad(w, way, b.just_pressed(PadButton::South), slide);
    } else if menu {
        buttons(w, &pad);
    } else {
        level(w, &pad);
    }
}

fn tap(w: &mut World, key: KeyCode) {
    w.e.input.keys.press(key);
    w.gamepad.tapped.push(key);
}

fn hold(w: &mut World, button: PadButton, key: KeyCode) {
    w.e.input.keys.press(key);
    w.gamepad.held.push((button, key));
}

/// Up or down held on the D-pad or left stick: whether to step now (at
/// once, then after a pause, then quickly) and which way (down positive).
fn repeated(w: &mut World, pad: &l3d_engine::input::Pad) -> Option<i32> {
    let b = &pad.buttons;
    let now = w.e.time.real.elapsed_secs();
    let way = (b.pressed(PadButton::Down) || pad.left.y < -STICK_STEP) as i32
        - (b.pressed(PadButton::Up) || pad.left.y > STICK_STEP) as i32;
    match (way, w.gamepad.repeat) {
        (0, _) => {
            w.gamepad.repeat = None;
            None
        }
        (_, Some((held, next))) if held == way => (now >= next).then(|| {
            w.gamepad.repeat = Some((way, now + REPEAT_EVERY));
            way
        }),
        _ => {
            w.gamepad.repeat = Some((way, now + REPEAT_AFTER));
            Some(way)
        }
    }
}

/// Typing `key` (and its text) as if on the keyboard.
fn type_key(w: &mut World, key_code: Option<KeyCode>, logical: l3d_engine::input::Key) {
    w.e.input.keyboard.push(l3d_engine::input::KeyboardInput {
        key_code,
        text: match &logical {
            l3d_engine::input::Key::Character(s) => Some(s.clone()),
            _ => None,
        },
        logical_key: logical,
        state: l3d_engine::input::ButtonState::Pressed,
        repeat: false,
    });
}

/// The code screen: up and down step the letter being chosen through the
/// alphabet (the first push types an A), A confirms it (the eighth enters
/// the code; with none being chosen, A enters what is there), B rubs out
/// the last letter or, with none, goes back to the title, as Menu does.
/// The screen reads typed keys only, so keys are typed here rather than
/// pressed (`tap`).
fn code(w: &mut World, pad: &l3d_engine::input::Pad) {
    use l3d_engine::input::{Key, NamedKey};
    let b = &pad.buttons;
    let letter = |c: u8| Key::Character((c as char).to_string().into());
    let rub_out = |w: &mut World| {
        type_key(w, Some(KeyCode::Backspace), Key::Named(NamedKey::Backspace));
        w.gamepad.letters -= 1;
    };
    if b.just_pressed(PadButton::Start)
        || (b.just_pressed(PadButton::East) && w.gamepad.letters == 0)
    {
        type_key(w, Some(KeyCode::Escape), Key::Named(NamedKey::Escape));
        return;
    }
    if b.just_pressed(PadButton::East) {
        rub_out(w);
        w.gamepad.choosing = false;
        return;
    }
    if let Some(way) = repeated(w, pad) {
        let g = &w.gamepad;
        let c = if g.choosing {
            // Down goes on through the alphabet, up back.
            let c = (g.letter - b'A' + if way > 0 { 1 } else { 25 }) % 26 + b'A';
            rub_out(w);
            Some(c)
        } else {
            (g.letters < CODE_LETTERS).then_some(b'A')
        };
        if let Some(c) = c {
            type_key(w, None, letter(c));
            let g = &mut w.gamepad;
            (g.letters, g.letter, g.choosing) = (g.letters + 1, c, true);
        }
    }
    if b.just_pressed(PadButton::South) {
        let g = &mut w.gamepad;
        let chose = std::mem::take(&mut g.choosing);
        if (!chose && g.letters > 0) || g.letters == CODE_LETTERS {
            type_key(w, Some(KeyCode::Enter), Key::Named(NamedKey::Enter));
            w.gamepad.letters = 0;
        }
    }
}

/// A level code's length (`codes::LEVEL_CODES`).
const CODE_LETTERS: usize = 8;

/// The level list: up and down (held, quickly) move the highlighted row,
/// the bumpers a page, left and right the rating, A chooses (the keys'
/// doing, `menu::menu_input`).
fn list(w: &mut World, pad: &l3d_engine::input::Pad) {
    let b = &pad.buttons;
    if let Some(way) = repeated(w, pad) {
        tap(
            w,
            if way > 0 {
                KeyCode::ArrowDown
            } else {
                KeyCode::ArrowUp
            },
        );
    }
    for (button, key) in [
        (PadButton::LeftBumper, KeyCode::PageUp),
        (PadButton::RightBumper, KeyCode::PageDown),
        (PadButton::Left, KeyCode::ArrowLeft),
        (PadButton::Right, KeyCode::ArrowRight),
        (PadButton::South, KeyCode::Enter),
    ] {
        if b.just_pressed(button) {
            tap(w, key);
        }
    }
}

/// The way the D-pad or the left stick (pushed, then let back) stepped this
/// frame, screen down positive; zero if neither.
fn stepped(w: &mut World, pad: &l3d_engine::input::Pad) -> Vec2 {
    let mut way = Vec2::ZERO;
    let mut held = Vec2::ZERO;
    for (button, dir) in [
        (PadButton::Up, Vec2::NEG_Y),
        (PadButton::Down, Vec2::Y),
        (PadButton::Left, Vec2::NEG_X),
        (PadButton::Right, Vec2::X),
    ] {
        if pad.buttons.just_pressed(button) {
            way += dir;
        }
        if pad.buttons.pressed(button) {
            held += dir;
        }
    }
    let now = w.e.time.real.elapsed_secs();
    let g = &mut w.gamepad;
    let stick = Vec2::new(pad.left.x, -pad.left.y);
    if stick.length() < STICK_REST {
        g.stick_ready = true;
    } else if g.stick_ready && stick.length() > STICK_STEP {
        g.stick_ready = false;
        way += stick;
    }
    if held == Vec2::ZERO && stick.length() > STICK_STEP {
        held = stick;
    }
    // Held, it steps again: after a while, then often.
    if held == Vec2::ZERO {
        g.step_repeat = None;
    } else if way != Vec2::ZERO {
        g.step_repeat = Some((held, now + REPEAT_AFTER));
    } else if let Some((_, next)) = g.step_repeat
        && now >= next
    {
        g.step_repeat = Some((held, now + REPEAT_EVERY));
        way = held;
    }
    way
}

/// The screens other than a level's: the D-pad and the left stick move the
/// pointer to the nearest button that way, A clicks it.
fn buttons(w: &mut World, pad: &l3d_engine::input::Pad) {
    let way = stepped(w, pad);
    // Over a level (its menu, its preview) only the overlay's buttons, not
    // the panel's under it.
    let within = if w.confirm.question.is_some() {
        crate::confirm::root(w)
    } else if w.preview.active {
        crate::briefing::preview_overlay_root(w)
    } else {
        None
    };
    let targets = w.ui.button_rects(within);
    // A new screen starts afresh.
    let screen = (w.state, w.preview.active);
    if w.gamepad.screen != Some(screen) {
        w.gamepad.screen = Some(screen);
        w.gamepad.focus = None;
    }
    let mut focus = w
        .gamepad
        .focus
        .and_then(|m| targets.iter().position(|t| t.1.distance(m) < 1.0));
    if focus.is_none() && w.gamepad.in_use && !targets.is_empty() {
        // On the briefing and previewing, Continue, so that A carries on
        // (owner); on the title, Play; elsewhere the button nearest the
        // middle.
        let briefing = w.state == AppState::Briefing || w.preview.active;
        let first = if briefing {
            crate::briefing::first_choice(w)
        } else if w.state == AppState::Title {
            crate::title::play_button(w)
        } else {
            None
        }
        .and_then(|id| w.ui.middle(id));
        let mid = Vec2::new(w.screen.pixels.z as f32, w.screen.pixels.w as f32) / 2.0;
        let goal = first.unwrap_or(mid);
        focus = (0..targets.len()).min_by(|&a, &b| {
            targets[a]
                .1
                .distance(goal)
                .total_cmp(&targets[b].1.distance(goal))
        });
    } else if let Some(i) = focus
        && way != Vec2::ZERO
    {
        let boxes: Vec<Rect> = targets.iter().map(|t| t.0).collect();
        focus = Some(crate::options::neighbour(&boxes, i, way).unwrap_or(i));
    }
    w.gamepad.focus = focus.map(|i| targets[i].1);
    let on = focus.filter(|_| w.gamepad.in_use).map(|i| targets[i]);
    // The frame round the button, and the (unseen) pointer on it, so that
    // A clicks it.
    frame(w, on.map(|t| t.0));
    let Some((_, middle)) = on else {
        return;
    };
    let offset = w.screen.min * w.e.window.scale_factor();
    w.e.window
        .set_physical_cursor_position(Some(middle + offset));
    if pad.buttons.just_pressed(PadButton::South) {
        w.e.input.mouse.press(MouseButton::Left);
        w.gamepad.clicked = true;
    }
}

/// Shows the frame round `rect` (logical pixels on the screen), as the
/// configuration screen frames its settings; none, hides it.
fn frame(w: &mut World, rect: Option<Rect>) {
    let id = match w.gamepad.frame.filter(|&id| w.ui.contains(id)) {
        Some(id) => id,
        None => {
            let id = w.ui.root(
                UiNode::new()
                    .border(px(2), Color::srgb(1.0, 0.9, 0.2))
                    .z(FRAME_Z)
                    .hidden(),
            );
            w.gamepad.frame = Some(id);
            id
        }
    };
    let Some(n) = w.ui.get_mut(id) else { return };
    match rect {
        Some(r) => {
            n.node.left = px(r.min.x - 3.0);
            n.node.top = px(r.min.y - 3.0);
            n.node.width = px(r.width() + 6.0);
            n.node.height = px(r.height() + 6.0);
            n.set_visible(true);
        }
        None => n.set_visible(false),
    }
}

/// How long after a replay ends by itself A still only takes over (s).
const REPLAY_GRACE: f32 = 1.0;

/// In a level.
fn level(w: &mut World, pad: &l3d_engine::input::Pad) {
    let b = &pad.buttons;
    // A demo (shown while idle) can't be taken over: any button ends it, as
    // any key does (`briefing::end_demo`).
    if let Some(back_to) = w.game.replay.as_ref().and_then(|r| r.demo) {
        // A trigger pulled (which would run the time) ends it too.
        let pulled = pad.left_trigger > 0.5 || pad.right_trigger > 0.5;
        let p = &mut w.e.input.pad;
        (p.left_trigger, p.right_trigger) = (0.0, 0.0);
        if b.get_just_pressed().len() > 0 || pulled {
            w.game.replay = None;
            w.current.loaded = None;
            w.next_state = Some(back_to);
        }
        return;
    }
    // The gamepad highlights the lemming nearest the middle, again when
    // the one highlighted is gone.
    // During a replay, stepping to a lemming takes over from it (as a click
    // does, `hud::take_over`) and steps; A only takes over.
    // A pressed just after a replay ended by itself (it reached where the
    // player restarted) is meant to stop it too, so it only takes over.
    let now = w.e.time.real.elapsed_secs();
    let replaying = w.game.replay.is_some();
    if replaying
        && [PadButton::Left, PadButton::Right, PadButton::South]
            .into_iter()
            .any(|p| b.just_pressed(p))
    {
        w.game.replay = None;
    }
    if b.just_pressed(PadButton::South) && (replaying || now - w.gamepad.replay_seen < REPLAY_GRACE)
    {
        w.gamepad.a_took_over = true;
    } else if !b.pressed(PadButton::South) {
        w.gamepad.a_took_over = false;
    }
    if replaying {
        w.gamepad.replay_seen = now;
    }
    let took_over = w.gamepad.a_took_over;
    let size = w.screen.size;
    let (sim, highlight) = (w.game.sim.as_mut(), &mut w.highlight);
    let Some(sim) = sim else { return };
    let used = b.get_just_pressed().len() > 0 || pad.left != Vec2::ZERO || pad.right != Vec2::ZERO;
    if used {
        w.gamepad.playing = w.current.loaded;
    }
    let active = w.gamepad.playing.is_some() && w.gamepad.playing == w.current.loaded;
    let following = w.lemming_cam.following();
    if following.is_none()
        && (active && highlight.lemming.is_none() || b.just_pressed(PadButton::RightStick))
    {
        let selected = (w.selected.0, w.selected.1);
        *highlight = Highlight {
            on: true,
            lemming: crate::hud::nearest_to_centre(sim, &w.camera, size),
            last_skill: None,
        };
        (w.selected.0, w.selected.1) = selected;
    }
    // Playing with the gamepad, a skill is always selected: the first with
    // any left, again when the one selected runs out (quietly: the panel
    // shows it).
    if active
        && w.selected
            .0
            .is_none_or(|s| sim.skills_left[s as usize] == 0)
        && let Some(i) = (0..9).find(|&i| sim.skills_left[i] > 0)
    {
        w.selected.0 = Some(Skill::ALL[i]);
    }
    // Both bumpers held a second nuke (ours, as the bomb's double click):
    // the skill the first one stepped to goes back, and the bomb flashes
    // meanwhile.
    let both = b.pressed(PadButton::LeftBumper) && b.pressed(PadButton::RightBumper);
    let now = w.e.time.real.elapsed_secs();
    if both {
        if w.gamepad.nuke_since.is_none() {
            w.gamepad.nuke_since = Some(now);
            if let Some(before) = w.gamepad.before_bumper.take() {
                w.selected.0 = before;
            }
        }
        if w.gamepad.nuke_since.is_some_and(|t| now - t >= NUKE_HOLD) && !sim.nuked {
            w.sfx.play("VOXFX/GEDDON1");
            sim.nuke();
        }
    } else {
        w.gamepad.nuke_since = None;
        if !b.pressed(PadButton::LeftBumper) && !b.pressed(PadButton::RightBumper) {
            w.gamepad.before_bumper = None;
        }
    }
    w.panel.nuke_held = both && !sim.nuked;
    // The bumpers: the skill before or after, of those left.
    let step = if both {
        0
    } else {
        b.just_pressed(PadButton::RightBumper) as i32 - b.just_pressed(PadButton::LeftBumper) as i32
    };
    if step != 0 {
        // Another skill: a turner waiting for its side is let go.
        w.pending.0 = None;
        w.gamepad.before_bumper = Some(w.selected.0);
        let at = w
            .selected
            .0
            .and_then(|s| Skill::ALL.iter().position(|&k| k == s))
            .map_or(if step > 0 { -1 } else { 0 }, |i| i as i32);
        let next = (1..=9)
            .map(|k| (at + step * k).rem_euclid(9) as usize)
            .find(|&i| sim.skills_left[i] > 0);
        if let Some(i) = next {
            w.selected.set(Some(Skill::ALL[i]));
        }
    }
    let riding = following.map(|_| w.view.head);
    // A turner waiting for its side: the D-pad points it, any way on
    // screen, to the arrow nearest that way; B lets it go.
    let pending = w.pending.0.filter(|_| !w.game.paused);
    let dpad = Vec2::new(
        b.just_pressed(PadButton::Right) as i32 as f32
            - b.just_pressed(PadButton::Left) as i32 as f32,
        b.just_pressed(PadButton::Down) as i32 as f32 - b.just_pressed(PadButton::Up) as i32 as f32,
    );
    if let Some(i) = pending {
        let point = match riding {
            // Riding along, the arrows lie along the lemming's sides as
            // seen with the head turned (up and down looking aside): the
            // way pressed picks the arrow it points most towards; along
            // the heading, neither.
            Some(head) => {
                let right = crate::hud::riding_axes(head).0;
                let along = dpad.normalize_or_zero().dot(right);
                (along.abs() > 0.3).then(|| size / 2.0 + right * along.signum() * size.x * 0.25)
            }
            None if dpad != Vec2::ZERO => Some(
                crate::hud::to_screen(
                    &w.camera,
                    size,
                    crate::hud::lemming_centre(&sim.lemmings[i]),
                )
                .map_or(size / 2.0, |(p, _)| p + dpad.normalize() * 80.0),
            ),
            None => None,
        };
        if let Some(point) = point {
            w.pending.0 = None;
            if let Some(ok) =
                crate::hud::give_pending_turner(sim, &w.camera, size, riding, i, point)
            {
                w.sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
            }
        }
        if b.just_pressed(PadButton::East) {
            w.pending.0 = None;
        }
    }
    let side = if pending.is_some() { 0 } else { dpad.x as i32 };
    if side != 0 {
        match highlight.lemming {
            // Riding along: on to the next or the one before.
            _ if following.is_some() => {
                let n = sim.lemmings.len();
                let f = following.unwrap_or(0);
                if let Some(next) = (1..n)
                    .map(|k| (f as i32 + side * k as i32).rem_euclid(n as i32) as usize)
                    .find(|&j| !sim.lemmings[j].gone)
                {
                    w.lemming_cam
                        .ride(next, highlight, &mut w.selected, &w.view);
                }
            }
            Some(i) => {
                if let Some(j) = beside(sim, &w.camera, size, i, side as f32) {
                    highlight.lemming = Some(j);
                    w.sfx.play("SPOTFX/SCALE");
                }
            }
            _ => {}
        }
    }
    // A: the skill to the highlighted lemming.
    if b.just_pressed(PadButton::South)
        && !took_over
        && w.game.replay.is_none()
        && !w.game.paused
        && let (Some(i), Some(s)) = (highlight.lemming, w.selected.0)
        && sim.skills_left[s as usize] > 0
    {
        if s == Skill::Turner {
            if sim.can_assign(i, s) {
                w.pending.0 = Some(i);
            } else {
                w.sfx.play("VOXFX/UH_UH1");
            }
        } else {
            let ok = sim.assign(i, s);
            w.sfx.play(if ok { "VOXFX/OK2" } else { "VOXFX/UH_UH1" });
        }
    }
    if b.just_pressed(PadButton::West) {
        let selected = (w.selected.0, w.selected.1);
        w.lemming_cam.face(highlight, &mut w.selected, &mut w.view);
        (w.selected.0, w.selected.1) = selected;
    }
    if b.just_pressed(PadButton::East) && pending.is_none() {
        tap(w, KeyCode::KeyP);
    }
    if b.just_pressed(PadButton::North) {
        let key = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
        ][(w.preset.0 + 1) % 4];
        tap(w, key);
    }
    if b.just_pressed(PadButton::Select) {
        tap(w, KeyCode::F12);
    }
    for (button, key) in [
        (PadButton::Up, KeyCode::Equal),
        (PadButton::Down, KeyCode::Minus),
    ] {
        if b.just_pressed(button) && pending.is_none() {
            hold(w, button, key);
        }
    }
}

/// The lemming next to lemming `i` on screen to its right (`way` 1) or
/// left (−1), of those on screen in order across it (top to bottom where
/// level), round from the last to the first: every one in view can be
/// reached either way (owner: the nearest that way skipped some).
fn beside(
    sim: &l3d_sim::Simulation,
    camera: &crate::scene_render::SceneCamera,
    size: Vec2,
    i: usize,
    way: f32,
) -> Option<usize> {
    let mut shown: Vec<(usize, Vec2)> = (0..sim.lemmings.len())
        .filter(|&j| !sim.lemmings[j].gone)
        .filter_map(|j| {
            let (p, _) =
                crate::hud::to_screen(camera, size, crate::hud::lemming_centre(&sim.lemmings[j]))?;
            (p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y).then_some((j, p))
        })
        .collect();
    shown.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.1.y.total_cmp(&b.1.y)));
    if shown.is_empty() {
        return None;
    }
    let n = shown.len() as i32;
    let at = match shown.iter().position(|&(j, _)| j == i) {
        Some(k) => (k as i32 + way as i32).rem_euclid(n),
        // Off screen: from the side it comes in at.
        None if way > 0.0 => 0,
        None => n - 1,
    };
    Some(shown[at as usize].0)
}
