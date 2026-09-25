//! M96: the time every animation, interaction state, and tick in this crate
//! reads -- the real clock, until `Window.advance(ms)` pins it for one tree's
//! headless tests. `engine-core` never reads the clock itself; every
//! timestamp it sees comes from here.
//!
//! Pinned instants are kept per tree, keyed by a weak reference to it, so
//! reading the time never borrows the tree -- nearly every caller reads it
//! while holding one -- and a pinned window never affects another. A weak
//! reference also keeps a dead tree's address from being reused while its
//! entry remains. `App.run()` unpins its windows, since a live window runs on
//! real time.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use engine_core::Tree;

type SharedTree = Rc<RefCell<Tree>>;

thread_local! {
    static PINNED: RefCell<Vec<(Weak<RefCell<Tree>>, Instant)>> = const { RefCell::new(Vec::new()) };
}

fn is(entry: &Weak<RefCell<Tree>>, tree: &SharedTree) -> bool {
    std::ptr::eq(entry.as_ptr(), Rc::as_ptr(tree))
}

/// `tree`'s current time: its pinned instant after `advance`, the real
/// clock otherwise.
pub(crate) fn now(tree: &SharedTree) -> Instant {
    PINNED
        .with(|pinned| {
            pinned
                .borrow()
                .iter()
                .find(|(entry, _)| is(entry, tree))
                .map(|(_, at)| *at)
        })
        .unwrap_or_else(Instant::now)
}

/// Moves `tree`'s time forward by exactly `by` and returns the new time.
/// The first call pins it at the real current time; from then on only
/// `advance` moves it.
pub(crate) fn advance(tree: &SharedTree, by: Duration) -> Instant {
    let next = now(tree) + by;
    PINNED.with(|pinned| {
        let mut pinned = pinned.borrow_mut();
        pinned.retain(|(entry, _)| entry.strong_count() > 0 && !is(entry, tree));
        pinned.push((Rc::downgrade(tree), next));
    });
    next
}

/// Returns `tree` to the real clock.
pub(crate) fn unpin(tree: &SharedTree) {
    PINNED.with(|pinned| pinned.borrow_mut().retain(|(entry, _)| !is(entry, tree)));
}
