//! What the core does for the arrows' focus (`nsh_core::focus`, the TV's
//! rules): it finds the views focus props name and hands the platforms
//! their nodes, and it scrolls a list to the row an arrow went to before
//! that row was mounted, then focuses it.
//!
//! - **Named views**: `nextFocusUp` / `Down` / `Left` / `Right` and a
//!   focus group's destinations name views by ref ([`FocusTarget`]); a
//!   Rust ref names whatever element carries it, which may be rendered
//!   after the view naming it (a group's own children). The views that
//!   name others are kept in [`Core::focus_refs`] and their targets looked
//!   up again in every commit round: a change goes out as the view's
//!   `MountItem::ViewProps` (`ViewProps::next_focus`,
//!   `ViewProps::focus_group`). Nothing runs while no view names one.
//! - **List rows past those mounted**: a platform whose arrow found no stop
//!   to go to inside the list around the focus asks
//!   ([`Core::focus_past`]); the core finds the row the focus is in, the
//!   next one along the list ([`crate::focus::next_row`]), scrolls the list
//!   to it by index (`Command::ScrollToIndex`, the `PinTarget::Index` path)
//!   and, once that row's cell shows it, sends the cell `Command::Focus`:
//!   the platform focuses its first stop. The row is forgotten when the
//!   focus moves elsewhere first.

use super::{Core, Key, NO_KEY, dirty};
use crate::{
    Command, NodeId,
    element::{FocusGroupTargets, FocusTarget},
    focus::{Direction, GroupConfig, NextFocus, PendingRow, Row, next_row},
    geometry::{Percentage, Points},
    text::TextEngine,
};

/// A view whose focus props name other views.
#[derive(Debug)]
pub(super) struct FocusRefs {
    key: Key,
    id: NodeId,
    /// `nextFocusUp`, `Down`, `Left`, `Right`.
    next: [Option<FocusTarget>; 4],
    /// A focus group's trap and destinations.
    group: Option<FocusGroupTargets>,
}

impl FocusRefs {
    fn is_empty(&self) -> bool {
        self.next.iter().all(Option::is_none) && self.group.is_none()
    }
}

fn slot(direction: Direction) -> usize {
    match direction {
        Direction::Up => 0,
        Direction::Down => 1,
        Direction::Left => 2,
        Direction::Right => 3,
    }
}

impl<M: TextEngine> Core<M> {
    /// The view `key`'s route toward `direction` (`nextFocus*`).
    pub(super) fn set_next_focus(
        &mut self,
        key: Key,
        direction: Direction,
        target: Option<FocusTarget>,
    ) {
        let target = target.filter(|t| *t != FocusTarget::None);
        if let Some(refs) = self.focus_refs_of(key, target.is_some()) {
            refs.next[slot(direction)] = target;
        }
    }

    /// The view `key` is a focus group (`nsh/base`'s `FocusGroup`), or no
    /// longer one.
    pub(super) fn set_focus_group(&mut self, key: Key, group: Option<FocusGroupTargets>) {
        if let Some(refs) = self.focus_refs_of(key, group.is_some()) {
            refs.group = group;
        }
    }

    /// `key`'s entry, made when `make`.
    fn focus_refs_of(&mut self, key: Key, make: bool) -> Option<&mut FocusRefs> {
        let id = self.nodes[key as usize].id;
        let at = match self
            .focus_refs
            .iter()
            .position(|r| r.key == key && r.id == id)
        {
            Some(at) => at,
            None if make => {
                self.focus_refs.push(FocusRefs {
                    key,
                    id,
                    next: Default::default(),
                    group: None,
                });
                self.focus_refs.len() - 1
            }
            None => return None,
        };
        Some(&mut self.focus_refs[at])
    }

    /// Looks the named views up again: the views whose targets' nodes
    /// changed get their `ViewProps` sent. Entries of released views, and
    /// those that name nothing any more (cleared), are dropped.
    pub(super) fn sync_focus_refs(&mut self) {
        let mut refs = std::mem::take(&mut self.focus_refs);
        refs.retain(|r| {
            self.nodes
                .get(r.key as usize)
                .is_some_and(|node| node.id == r.id)
        });
        for r in &refs {
            // A node named before it is made is found once it is.
            let found = |target: &FocusTarget| target.node().filter(|&id| self.key(id).is_ok());
            let mut next = NextFocus::default();
            for direction in [
                Direction::Up,
                Direction::Down,
                Direction::Left,
                Direction::Right,
            ] {
                next.set(direction, r.next[slot(direction)].as_ref().and_then(found));
            }
            let group = r.group.as_ref().map(|g| GroupConfig {
                trap: g.trap,
                destinations: g.destinations.iter().filter_map(found).collect(),
            });
            let node = &mut self.nodes[r.key as usize];
            let unchanged = match &node.props {
                Some(props) => props.next_focus == next && props.focus_group == group,
                None => next.is_empty() && group.is_none(),
            };
            if unchanged {
                continue;
            }
            let props = node.props.get_or_insert_default();
            props.next_focus = next;
            props.focus_group = group;
            self.mark(r.key, dirty::VIEW_PROPS);
        }
        refs.retain(|r| !r.is_empty());
        // Entries made while these were looked up (none: nothing renders
        // here) would come after them.
        refs.append(&mut self.focus_refs);
        self.focus_refs = refs;
    }

    /// An arrow toward `direction` found no stop to go to inside the list
    /// around `from` (the stop with the keyboard focus), which can scroll
    /// on: the list scrolls to the next row along its axis by index
    /// (`Command::ScrollToIndex`), and that row's cell takes the focus once
    /// it is mounted (`Command::Focus`; the platform focuses its first
    /// stop). The innermost list around `from` that has a next row that
    /// way does; false when none has (a scroll view that is no list, the
    /// list's end), and nothing happens.
    pub fn focus_past(&mut self, from: NodeId, direction: Direction) -> bool {
        let Ok(mut key) = self.key(from) else {
            return false;
        };
        loop {
            let parent = self.nodes[key as usize].parent;
            if parent == NO_KEY {
                return false;
            }
            let list_node = &self.nodes[parent as usize];
            if let Some(list) = list_node.list.as_ref()
                && let Some(index) = list
                    .slots
                    .iter()
                    .find(|s| s.cell == key)
                    .and_then(|s| s.ready())
            {
                let row = Row {
                    list: list_node.id,
                    index,
                    count: list.count,
                    columns: list.columns,
                    horizontal: list.horizontal,
                };
                // An inverted list's next item is above (before) it, and
                // so is a right-to-left one's to the left.
                let along = direction.horizontal() == list.horizontal;
                let toward = if along && (list.inverted || list.mirrored()) {
                    direction.opposite()
                } else {
                    direction
                };
                if let Some(to) = next_row(&row, toward) {
                    // It comes in at the edge the arrow goes toward.
                    let end = if list.inverted {
                        !toward.forward()
                    } else {
                        toward.forward()
                    };
                    self.commands.push((
                        row.list,
                        Command::ScrollToIndex {
                            index: to,
                            animated: true,
                            view_position: Percentage(if end { 1.0 } else { 0.0 }),
                            view_offset: Points::ZERO,
                        },
                    ));
                    let pending = PendingRow {
                        list: row.list,
                        index: to,
                        from,
                    };
                    self.pending_row = Some((pending, self.focus.map(|(_, id)| id)));
                    return true;
                }
            }
            key = parent;
        }
    }

    /// The row an arrow went to ([`Core::focus_past`]) is mounted: its
    /// cell takes the focus. Forgotten when the focus moved elsewhere
    /// first, or the list or the row is gone.
    pub(super) fn land_pending_row(&mut self) {
        let Some((pending, seen)) = self.pending_row else {
            return;
        };
        // The platform reported another focus since (the stop the arrow
        // left need not report its own).
        let now = self.focus.map(|(_, id)| id);
        let moved = now != seen && now != Some(pending.from);
        let list = self
            .key(pending.list)
            .ok()
            .and_then(|key| self.nodes[key as usize].list.as_ref());
        let Some(list) = list.filter(|l| !moved && pending.index < l.count) else {
            self.pending_row = None;
            return;
        };
        let Some(cell) = list
            .slots
            .iter()
            .find(|s| s.ready() == Some(pending.index))
            .map(|s| s.cell)
            .filter(|&cell| self.nodes[cell as usize].created)
        else {
            return;
        };
        let id = self.nodes[cell as usize].id;
        self.commands.push((id, Command::Focus));
        self.pending_row = None;
    }
}
