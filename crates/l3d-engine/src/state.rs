//! States, as Bevy had them: a [`State`] resource, a [`NextState`] to ask
//! for a change, and the [`OnEnter`] and [`OnExit`] schedules run when one
//! happens. A change asked for is made between [`PreUpdate`] and the fixed
//! steps, and always runs both schedules, even for the state it is already
//! in.
//!
//! [`PreUpdate`]: crate::app::PreUpdate

use std::fmt::Debug;
use std::hash::Hash;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;

/// What a state can be.
pub trait States: Clone + PartialEq + Eq + Hash + Debug + Send + Sync + 'static {}

impl<T: Clone + PartialEq + Eq + Hash + Debug + Send + Sync + 'static> States for T {}

/// The current state.
#[derive(Resource, Debug)]
pub struct State<S: States>(S);

impl<S: States> State<S> {
    pub fn get(&self) -> &S {
        &self.0
    }
}

impl<S: States> std::ops::Deref for State<S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.0
    }
}

impl<S: States> PartialEq<S> for State<S> {
    fn eq(&self, other: &S) -> bool {
        self.0 == *other
    }
}

/// A change of state asked for, made later in the frame.
#[derive(Resource, Debug, Default)]
pub enum NextState<S: States> {
    #[default]
    Unchanged,
    Pending(S),
}

impl<S: States> NextState<S> {
    pub fn set(&mut self, state: S) {
        *self = NextState::Pending(state);
    }

    pub fn reset(&mut self) {
        *self = NextState::Unchanged;
    }
}

/// Runs when the state becomes this value.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnEnter<S: States>(pub S);

/// Runs when the state stops being this value.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnExit<S: States>(pub S);

/// A run condition: whether the state is `state`.
pub fn in_state<S: States>(state: S) -> impl FnMut(Option<Res<State<S>>>) -> bool + Clone {
    move |current: Option<Res<State<S>>>| current.is_some_and(|c| c.0 == state)
}

/// The initial state, entered at the first state transition.
#[derive(Resource)]
struct Entering<S: States>(S);

pub(crate) fn insert<S: States>(world: &mut World, initial: S) {
    world.insert_resource(State(initial.clone()));
    world.insert_resource(NextState::<S>::Unchanged);
    world.insert_resource(Entering(initial));
}

/// Makes the change asked for, if any: runs [`OnExit`] for the old state
/// and [`OnEnter`] for the new.
pub(crate) fn apply<S: States>(world: &mut World) {
    if let Some(Entering(initial)) = world.remove_resource::<Entering<S>>() {
        let _ = world.try_run_schedule(OnEnter(initial));
        return;
    }
    let next = std::mem::take(&mut *world.resource_mut::<NextState<S>>());
    let NextState::Pending(entered) = next else {
        return;
    };
    let exited = std::mem::replace(&mut world.resource_mut::<State<S>>().0, entered.clone());
    let _ = world.try_run_schedule(OnExit(exited));
    let _ = world.try_run_schedule(OnEnter(entered));
}
