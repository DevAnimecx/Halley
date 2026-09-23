//! Ordered tab collection with active-tab tracking and a bounded
//! closed-tab stack for reopen.
//!
//! Invariants (enforced by construction; asserted in tests):
//! * `tabs` is never empty after the first insert of the session —
//!   closing the last tab is the manager's caller's job to replace
//!   (see [`crate::Browser`], which re-opens `new_tab_url`).
//! * `active` always points at a tab that exists in `tabs` after
//!   successful operations (or is `None` only before the first tab).
//! * `closed` is bounded by `max_closed_tabs` (config).

use std::collections::VecDeque;

use crate::tab::{Tab, TabId};

/// A closed tab remembered for reopen (URL + last title only — the
/// engine page is gone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTab {
    /// Original tab id (informational; reopen allocates a new id).
    pub id: TabId,
    /// URL to restore.
    pub url: String,
    /// Last known title (display only until reload).
    pub title: String,
}

/// Ordered tabs + active selection + reopen stack.
#[derive(Debug)]
pub struct TabManager {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    closed: VecDeque<ClosedTab>,
    next_id: u64,
    max_closed: usize,
}

impl TabManager {
    /// Create an empty manager; `max_closed` is the reopen-stack bound.
    pub fn new(max_closed: usize) -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            closed: VecDeque::new(),
            next_id: 1,
            max_closed,
        }
    }

    /// Tabs in display order.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Currently active tab id, if any.
    pub fn active_id(&self) -> Option<TabId> {
        self.active
    }

    /// Currently active tab, if any.
    pub fn active(&self) -> Option<&Tab> {
        self.active.and_then(|id| self.get(id))
    }

    /// Currently active tab, mutable (nav-flag refresh from the engine).
    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active?;
        self.get_mut(id)
    }

    /// Number of open tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether there are no open tabs (transient before first insert).
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Look up a tab by id.
    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id() == id)
    }

    /// Mutable lookup by id.
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id() == id)
    }

    /// Mutable lookup by engine page id (event routing).
    pub fn iter_mut_by_page(&mut self, page: halley_engine::PageId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.page() == page)
    }

    /// Index of `id` in display order.
    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id() == id)
    }

    /// Allocate a fresh [`TabId`] (never reuses prior ids in this manager).
    pub fn alloc_id(&mut self) -> TabId {
        let id = TabId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    /// Peek at the most recently closed tab without restoring it.
    pub fn last_closed(&self) -> Option<&ClosedTab> {
        self.closed.front()
    }

    /// Pop the most recently closed tab for reopen (LIFO).
    pub fn pop_closed(&mut self) -> Option<ClosedTab> {
        self.closed.pop_front()
    }

    /// Closed-stack length (tests / diagnostics).
    pub fn closed_len(&self) -> usize {
        self.closed.len()
    }

    /// Insert `tab` at `index` (clamped) and make it active.
    ///
    /// Returns the tab id. `index > len` appends.
    pub fn insert_at(&mut self, index: usize, tab: Tab) -> TabId {
        let id = tab.id();
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab);
        self.active = Some(id);
        id
    }

    /// Append `tab` as the last strip entry and activate it.
    pub fn push_active(&mut self, tab: Tab) -> TabId {
        let id = tab.id();
        self.tabs.push(tab);
        self.active = Some(id);
        id
    }

    /// Activate `id` if it exists. Returns whether the active tab changed.
    pub fn activate(&mut self, id: TabId) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        if self.active == Some(id) {
            return false;
        }
        self.active = Some(id);
        true
    }

    /// Activate the next tab (wraps). Returns the newly active id, or
    /// `None` if there are no tabs.
    pub fn activate_next(&mut self) -> Option<TabId> {
        self.activate_relative(1)
    }

    /// Activate the previous tab (wraps). Returns the newly active id.
    pub fn activate_prev(&mut self) -> Option<TabId> {
        self.activate_relative(-1)
    }

    fn activate_relative(&mut self, delta: isize) -> Option<TabId> {
        if self.tabs.is_empty() {
            return None;
        }
        let len = self.tabs.len() as isize;
        let current = self
            .active
            .and_then(|id| self.index_of(id))
            .map(|i| i as isize)
            .unwrap_or(0);
        let next = ((current + delta).rem_euclid(len)) as usize;
        let id = self.tabs[next].id();
        self.active = Some(id);
        Some(id)
    }

    /// Remove `id` from the strip, remembering it for reopen (bounded).
    ///
    /// Picks a neighbor for `active` when the active tab is removed.
    /// Returns the removed tab's id if it existed.
    pub fn close(&mut self, id: TabId) -> Option<TabId> {
        let index = self.index_of(id)?;
        let tab = self.tabs.remove(index);
        if self.max_closed > 0 {
            self.closed.push_front(ClosedTab {
                id: tab.id(),
                url: tab.url().to_string(),
                title: tab.title().to_string(),
            });
            while self.closed.len() > self.max_closed {
                self.closed.pop_back();
            }
        }

        if self.active == Some(id) {
            if self.tabs.is_empty() {
                self.active = None;
            } else {
                let next_index = index.min(self.tabs.len() - 1);
                self.active = Some(self.tabs[next_index].id());
            }
        }
        Some(id)
    }

    /// Remove every tab except `keep` (must exist). Returns removed ids
    /// in strip order. Active becomes `keep`.
    pub fn close_others(&mut self, keep: TabId) -> Vec<TabId> {
        if self.get(keep).is_none() {
            return Vec::new();
        }
        let mut removed = Vec::new();
        // Close from the end so indices stay valid.
        let keep_index = self.index_of(keep).unwrap_or(0);
        for i in (0..self.tabs.len()).rev() {
            if i == keep_index {
                continue;
            }
            let id = self.tabs[i].id();
            if let Some(removed_id) = self.close(id) {
                removed.push(removed_id);
            }
        }
        removed.reverse();
        self.active = Some(keep);
        removed
    }

    /// Close every tab whose strip position is strictly after `from`'s
    /// position. Returns removed ids in original strip order.
    pub fn close_to_right(&mut self, from: TabId) -> Vec<TabId> {
        let Some(start) = self.index_of(from) else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        while self.tabs.len() > start + 1 {
            let id = self.tabs[self.tabs.len() - 1].id();
            if let Some(removed_id) = self.close(id) {
                removed.push(removed_id);
            }
        }
        removed.reverse();
        // Active tab may have been in the closed range; restore a valid one.
        if self.active.is_none() || self.get(self.active.unwrap()).is_none() {
            if let Some(id) = self.tabs.get(start).map(Tab::id) {
                self.active = Some(id);
            }
        }
        removed
    }

    /// Move `id` to `to_index` (clamped). Returns the final index.
    pub fn move_tab(&mut self, id: TabId, to_index: usize) -> Option<usize> {
        let from = self.index_of(id)?;
        let to = to_index.min(self.tabs.len() - 1);
        if from == to {
            return Some(to);
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        Some(to)
    }

    /// Stable invariant checks used by tests and debug asserts.
    pub fn assert_invariants(&self) {
        // Unique ids.
        for (i, a) in self.tabs.iter().enumerate() {
            for b in &self.tabs[i + 1..] {
                assert_ne!(a.id(), b.id(), "duplicate tab ids");
            }
        }
        // Active points at a live tab (when any tab exists).
        if let Some(active) = self.active {
            assert!(
                self.get(active).is_some(),
                "active id {active:?} not in tabs"
            );
        } else {
            assert!(self.tabs.is_empty(), "no active tab but tabs exist");
        }
        // Closed stack bounded.
        assert!(self.closed.len() <= self.max_closed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab::Tab;

    fn tab(manager: &mut TabManager, url: &str) -> TabId {
        let id = manager.alloc_id();
        let t = Tab::new(id, id.raw(), url);
        manager.push_active(t);
        id
    }

    fn manager() -> TabManager {
        TabManager::new(10)
    }

    #[test]
    fn empty_manager_has_no_active_tab() {
        let m = manager();
        assert!(m.is_empty());
        assert_eq!(m.active_id(), None);
        m.assert_invariants();
    }

    #[test]
    fn push_activates_and_order_is_insertion() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        assert_eq!(m.active_id(), Some(b));
        assert_eq!(m.tabs().iter().map(Tab::id).collect::<Vec<_>>(), vec![a, b]);
        m.assert_invariants();
    }

    #[test]
    fn activate_unknown_id_is_rejected() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        assert!(!m.activate(TabId::from_raw(a.raw() + 100)));
        assert_eq!(m.active_id(), Some(a));
    }

    #[test]
    fn next_prev_wrap_around() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        assert_eq!(m.activate_next(), Some(a));
        assert_eq!(m.activate_next(), Some(b));
        assert_eq!(m.activate_next(), Some(c));
        assert_eq!(m.activate_next(), Some(a)); // wrap
        assert_eq!(m.activate_prev(), Some(c)); // wrap back
        m.assert_invariants();
    }

    #[test]
    fn close_moves_active_to_neighbor_and_remembers_closed() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        m.activate(b);
        assert_eq!(m.close(b), Some(b));
        // Active was b at index 1 → neighbor c at index 1 after removal.
        assert_eq!(m.active_id(), Some(c));
        assert_eq!(m.last_closed().map(|c| c.id), Some(b));
        assert_eq!(m.tabs().iter().map(Tab::id).collect::<Vec<_>>(), vec![a, c]);
        m.assert_invariants();
    }

    #[test]
    fn close_last_tab_clears_active_and_stack_bounds() {
        let mut m = TabManager::new(2);
        let a = tab(&mut m, "a");
        assert_eq!(m.close(a), Some(a));
        assert!(m.is_empty());
        assert_eq!(m.active_id(), None);
        assert_eq!(m.closed_len(), 1);

        let _b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        let d = tab(&mut m, "d");
        assert_eq!(m.close(c), Some(c));
        assert_eq!(m.close(d), Some(d));
        assert_eq!(m.closed_len(), 2); // capped at max_closed (b was first closed, then a,c,d → keep last 2)
        m.assert_invariants();
    }

    #[test]
    fn close_others_keeps_only_keep() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        m.activate(b);
        let removed = m.close_others(c);
        assert_eq!(removed, vec![a, b]);
        assert_eq!(m.tabs().iter().map(Tab::id).collect::<Vec<_>>(), vec![c]);
        assert_eq!(m.active_id(), Some(c));
        m.assert_invariants();
    }

    #[test]
    fn close_to_right_closes_suffix_only() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        m.activate(a);
        let removed = m.close_to_right(a);
        assert_eq!(removed, vec![b, c]);
        assert_eq!(m.tabs().iter().map(Tab::id).collect::<Vec<_>>(), vec![a]);
        m.assert_invariants();
    }

    #[test]
    fn move_tab_clamps_and_preserves_identity() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        let c = tab(&mut m, "c");
        assert_eq!(m.move_tab(c, 0), Some(0));
        assert_eq!(
            m.tabs().iter().map(Tab::id).collect::<Vec<_>>(),
            vec![c, a, b]
        );
        assert_eq!(m.move_tab(c, 99), Some(2));
        assert_eq!(
            m.tabs().iter().map(Tab::id).collect::<Vec<_>>(),
            vec![a, b, c]
        );
        m.assert_invariants();
    }

    #[test]
    fn pop_closed_is_lifo() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        let b = tab(&mut m, "b");
        m.close(a);
        m.close(b);
        // b closed last → reopened first.
        assert_eq!(m.pop_closed().map(|c| c.id), Some(b));
        assert_eq!(m.pop_closed().map(|c| c.id), Some(a));
        assert_eq!(m.pop_closed(), None);
    }

    #[test]
    fn ids_are_never_reused_after_close() {
        let mut m = manager();
        let a = tab(&mut m, "a");
        m.close(a);
        let b = tab(&mut m, "b");
        assert_ne!(a, b);
        assert!(b.raw() > a.raw());
    }
}
