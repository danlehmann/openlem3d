//! Replaying the view, as the original does: with the replay camera set to
//! Original, a replay (and running the level back) also shows what the
//! player saw at each tick: the camera and its moves, the preset camera
//! chosen, the lemming view, highlighting and the selected skill, and any
//! change of the viewer's own to the view ends the replay there, the
//! viewer taking over. Free (the default, ours), a replay only repeats the
//! player's commands and the viewer moves the camera freely.

use l3d_engine::prelude::*;
use l3d_sim::Skill;

use crate::World;
use crate::lemming_cam::LemmingCam;

/// What the player saw at one tick.
#[derive(Clone, Copy, PartialEq)]
pub struct ViewMoment {
    pos: Vec3,
    yaw: f32,
    roll: f32,
    head: f32,
    nod: f32,
    preset: usize,
    info: Option<crate::LevelInfo>,
    lemming_cam: LemmingCam,
    highlight_on: bool,
    highlight: Option<usize>,
    skill: Option<Skill>,
}

/// The view at each tick of an attempt, in tick order, one per tick
/// reached.
pub type ViewLog = Vec<(u64, ViewMoment)>;

fn moment(w: &World) -> ViewMoment {
    let v = &w.view;
    // Mid-glide, where the glide ends: the level waits for it, so the tick
    // sees no other place.
    let (pos, yaw) = v.glide.as_ref().map_or((v.pos, v.yaw), |g| g.to);
    ViewMoment {
        pos,
        yaw,
        roll: v.roll,
        head: v.head,
        nod: v.nod,
        preset: w.preset.0,
        info: w.info,
        lemming_cam: w.lemming_cam,
        highlight_on: w.highlight.on,
        highlight: w.highlight.lemming,
        skill: w.selected.0,
    }
}

/// Notes the view at the simulation's tick, once per tick; a tick seen
/// again after running back replaces what followed it.
pub(crate) fn record(w: &mut World) {
    let Some(tick) = w.game.sim.as_ref().map(|s| s.tick) else {
        return;
    };
    if w.backwards() || w.game.views.last().is_some_and(|&(t, _)| t == tick) {
        return;
    }
    let m = moment(w);
    let views = &mut w.game.views;
    let keep = views.partition_point(|&(t, _)| t < tick);
    views.truncate(keep);
    views.push((tick, m));
}

/// The moment recorded for `tick`, or the last before it.
fn at(log: &ViewLog, tick: u64) -> Option<&ViewMoment> {
    let i = log.partition_point(|&(t, _)| t <= tick);
    i.checked_sub(1).map(|i| &log[i].1)
}

/// With the replay camera Original, shows the recorded view during a replay that still
/// follows it, and while the level runs back. Runs after the viewer's own
/// camera controls: a change since the view was last set is theirs, and
/// ends the replay's view.
pub(crate) fn play(w: &mut World) {
    if !w.settings.original_replays {
        return;
    }
    let backwards = w.backwards();
    let Some(tick) = w.game.sim.as_ref().map(|s| s.tick) else {
        return;
    };
    let replaying = w.game.replay.as_ref().is_some_and(|r| r.views.is_some());
    if !replaying && !backwards {
        (w.game.view_shown, w.game.view_gliding) = (None, false);
        return;
    }
    // A glide of the replay's own runs its course (the level waits for it).
    if w.game.view_gliding {
        if w.view.glide.is_some() {
            return;
        }
        w.game.view_gliding = false;
        w.game.view_shown = Some(moment(w));
    }
    let log = match &w.game.replay {
        Some(crate::Replay {
            views: Some(views), ..
        }) if !backwards => views,
        _ => &w.game.views,
    };
    let (Some(&now), next) = (at(log, tick), at(log, tick + 1).copied()) else {
        return;
    };
    if replaying
        && !backwards
        && let Some(last) = w.game.view_shown
        && moved(w, &last, &now)
    {
        // The viewer takes over, as in the original.
        w.game.replay = None;
        w.game.view_shown = None;
        return;
    }
    // Between this tick's view and the next one's, as the lemmings are
    // drawn between their places.
    let f = if backwards {
        0.0
    } else {
        w.game
            .tick_progress(w.settings.enhanced(), w.e.time.fixed.overstep_fraction())
    };
    let shown = match next {
        Some(n) if f > 0.0 && n.preset == now.preset && n.lemming_cam == now.lemming_cam => {
            ViewMoment {
                pos: now.pos.lerp(n.pos, f),
                yaw: lerp_angle(now.yaw, n.yaw, f),
                roll: now.roll + (n.roll - now.roll) * f,
                head: now.head + (n.head - now.head) * f,
                nod: now.nod + (n.nod - now.nod) * f,
                ..now
            }
        }
        _ => now,
    };
    // Another camera, or into or out of the lemming view: a glide, as
    // the player's was (the original and Original mode cut).
    let switched = w
        .game
        .view_shown
        .is_some_and(|s| s.preset != shown.preset || s.lemming_cam != shown.lemming_cam);
    let glide = switched && w.view.smooth && !backwards;
    show(w, &shown, glide);
    w.game.view_gliding = glide;
    w.game.view_shown = Some(shown);
}

/// Whether the viewer changed the view since it was shown as `last`; a
/// change the replay itself makes at this tick (`now`: a ride ending as
/// its lemming goes home, say) is not theirs.
fn moved(w: &World, last: &ViewMoment, now: &ViewMoment) -> bool {
    let v = &w.view;
    // Riding along, the ride itself moves the camera.
    let riding = matches!(last.lemming_cam, LemmingCam::Following { .. });
    let camera = !riding && (v.pos.distance(last.pos) > 1e-4 || (v.yaw - last.yaw).abs() > 1e-4);
    let switching = now.preset != last.preset || now.lemming_cam != last.lemming_cam;
    let neither = |a: bool, b: bool| a && b;
    ((camera || v.glide.is_some()) && !switching)
        || (v.head - last.head).abs() > 1e-4
        || neither(w.preset.0 != last.preset, w.preset.0 != now.preset)
        || neither(
            w.lemming_cam != last.lemming_cam,
            w.lemming_cam != now.lemming_cam,
        )
        || neither(
            w.highlight.on != last.highlight_on,
            w.highlight.on != now.highlight_on,
        )
        || neither(
            w.highlight.lemming != last.highlight,
            w.highlight.lemming != now.highlight,
        )
        || neither(w.selected.0 != last.skill, w.selected.0 != now.skill)
}

/// Shows moment `m`; with `glide`, the camera glides there.
fn show(w: &mut World, m: &ViewMoment, glide: bool) {
    let v = &mut w.view;
    if glide {
        v.glide_to(m.pos, m.yaw);
    } else {
        v.glide = None;
        (v.pos, v.yaw) = (m.pos, m.yaw);
    }
    (v.roll, v.head, v.nod) = (m.roll, m.head, m.nod);
    if w.preset.0 != m.preset {
        w.preset.set(m.preset);
    }
    w.info = m.info;
    w.lemming_cam = m.lemming_cam;
    w.highlight.on = m.highlight_on;
    w.highlight.lemming = m.highlight;
    if w.selected.0 != m.skill {
        w.selected.set(m.skill);
    }
}

/// `a` turned `f` of the short way towards `b` (radians).
fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    let d = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + d * f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_turn_the_short_way() {
        let tau = std::f32::consts::TAU;
        assert!((lerp_angle(0.1, tau - 0.1, 0.5) - 0.0).abs() < 1e-5);
        assert!((lerp_angle(1.0, 2.0, 0.25) - 1.25).abs() < 1e-5);
    }

    #[test]
    fn a_moment_is_found_at_or_before_its_tick() {
        let m = |x: f32| ViewMoment {
            pos: Vec3::new(x, 0.0, 0.0),
            yaw: 0.0,
            roll: 0.0,
            head: 0.0,
            nod: 0.0,
            preset: 0,
            info: None,
            lemming_cam: LemmingCam::Off,
            highlight_on: false,
            highlight: None,
            skill: None,
        };
        let log: ViewLog = vec![(3, m(3.0)), (5, m(5.0))];
        assert!(at(&log, 2).is_none());
        assert_eq!(at(&log, 4).map(|m| m.pos.x), Some(3.0));
        assert_eq!(at(&log, 9).map(|m| m.pos.x), Some(5.0));
    }
}
