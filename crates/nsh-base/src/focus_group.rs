//! `FocusGroup`: a region of a screen the arrow keys treat as one, as a
//! TV's remote moves through rows and menus: the focus enters it at its
//! `destinations` (a row's current item rather than the item nearest the
//! focus), and a group that traps keeps the arrows inside (a side menu
//! open over the page). The rules are `nsh_core::focus::directional`'s;
//! the group is a view whose props carry them (`Interactive::focus_group`),
//! which the platforms that move the focus with the arrows read (GPUI, TVs).

use nsh_core::{element::FocusTarget, prelude::*};

/// The props of [`FocusGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FocusGroupProps {
    /// The views the arrows enter the group at, in order (TS: their refs;
    /// Rust: [`FocusTarget`]s of the elements' `ElementRef`s): the first
    /// that is, or holds, a stop of the keyboard focus in reach takes it.
    /// Without one in reach, the group's `preferredFocus` stop takes it,
    /// else the stop nearest the focus. They are found in every commit, so
    /// they may be the group's own children.
    #[prop(default)]
    pub destinations: Prop<Vec<FocusTarget>>,
    /// The arrows never take the focus out of the group (default `false`):
    /// an arrow with no stop that way inside it goes nowhere. A view's own
    /// `nextFocus*` route still may.
    #[prop(default)]
    pub trap: Prop<bool>,
    /// The group's content: its stops and whatever the app lays out
    /// among them.
    pub children: Prop<Children>,
}

/// A region the arrow keys treat as one (Android TV's focus groups, the
/// web's spatial navigation containers): the arrows enter it at the first
/// of its `destinations` in reach, else at its `preferredFocus` stop,
/// else at the stop nearest the focus, and with `trap` they do not leave
/// it. Moving inside it is the arrows' usual nearest stop. It has no look
/// and lays out nothing of its own: a view the app styles (`class`,
/// `style`) around its children. Groups nest: a trap holds the groups
/// inside it.
///
/// Its rules apply where the arrows move the keyboard focus between views
/// (the GPUI backend, TVs); elsewhere it is a plain view.
#[component]
pub fn FocusGroup(props: FocusGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .focus_group(props.trap, props.destinations)
        .child(props.children)
}
