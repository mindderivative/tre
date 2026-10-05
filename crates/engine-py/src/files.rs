//! 0.5.4 (#114): files dragged from the OS onto a window.
//!
//! The platform reports a drag as one event per file, and gives them no
//! position of their own, so the pointer's last stands in. They are queued
//! as they arrive and delivered together by `flush` -- once a frame live, at
//! once under `simulate` -- so a drop of three files is one `file_drop` with
//! three `paths`, not three events. Each goes to the node under the pointer
//! (bubbling) and to the window's own listener.

use std::path::PathBuf;

use engine_core::{InputEvent, NodeId};
use peniko::kurbo::Point;
use pyo3::prelude::*;

use crate::dispatch::WindowIo;
use crate::event::NodeContext;
use crate::listeners::{self, EventType, WindowEventType};

enum Queued {
    Hover(Vec<PathBuf>, Point),
    Cancel,
    Drop(Vec<PathBuf>, Point),
}

/// A window's file drag state.
#[derive(Default)]
pub(crate) struct FileDrops {
    queue: Vec<Queued>,
    /// The node that last heard `file_hover`, to tell when it is cancelled.
    hover_target: Option<NodeId>,
}

impl FileDrops {
    /// Whether anything is waiting for the next `flush`.
    pub(crate) fn pending(&self) -> bool {
        !self.queue.is_empty()
    }
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Queues a file event; nothing is delivered until `flush`.
pub(crate) fn queue(io: &WindowIo<'_>, event: &InputEvent) {
    let mut files = io.window.files.borrow_mut();
    match event {
        InputEvent::FileHovered { path, position } => match files.queue.last_mut() {
            Some(Queued::Hover(paths, at)) => {
                paths.push(path.clone());
                *at = *position;
            }
            _ => files
                .queue
                .push(Queued::Hover(vec![path.clone()], *position)),
        },
        InputEvent::FileHoverCancelled => files.queue.push(Queued::Cancel),
        InputEvent::FileDropped { path, position } => match files.queue.last_mut() {
            Some(Queued::Drop(paths, at)) => {
                paths.push(path.clone());
                *at = *position;
            }
            _ => files
                .queue
                .push(Queued::Drop(vec![path.clone()], *position)),
        },
        _ => {}
    }
}

/// Delivers what is queued, in order.
pub(crate) fn flush(ctx: &NodeContext<'_>, io: &WindowIo<'_>, root: NodeId, py: Python<'_>) {
    let queued = std::mem::take(&mut io.window.files.borrow_mut().queue);
    for item in queued {
        match item {
            Queued::Hover(paths, at) => {
                let target = ctx.tree.borrow().hit_test_input(root, at).unwrap_or(root);
                io.window.files.borrow_mut().hover_target = Some(target);
                let list = names(&paths);
                let (first, all) = (list.first().cloned(), list);
                listeners::deliver_window(io.listeners, py, WindowEventType::FileHover, |e| {
                    e.window_x = Some(at.x);
                    e.window_y = Some(at.y);
                    e.path = first.clone();
                    e.paths = Some(all.clone());
                });
                let (first, all) = (first.clone(), all.clone());
                listeners::deliver(ctx, py, EventType::FileHover, target, Some(at), |e| {
                    e.path = first;
                    e.paths = Some(all);
                });
            }
            Queued::Cancel => {
                let target = io.window.files.borrow_mut().hover_target.take();
                listeners::deliver_window(
                    io.listeners,
                    py,
                    WindowEventType::FileHoverCancel,
                    |_| {},
                );
                if let Some(target) = target {
                    listeners::deliver(ctx, py, EventType::FileHoverCancel, target, None, |_| {});
                }
            }
            Queued::Drop(paths, at) => {
                io.window.files.borrow_mut().hover_target = None;
                let target = ctx.tree.borrow().hit_test_input(root, at).unwrap_or(root);
                let list = names(&paths);
                let (first, all) = (list.first().cloned(), list);
                listeners::deliver_window(io.listeners, py, WindowEventType::FileDrop, |e| {
                    e.window_x = Some(at.x);
                    e.window_y = Some(at.y);
                    e.path = first.clone();
                    e.paths = Some(all.clone());
                });
                listeners::deliver(ctx, py, EventType::FileDrop, target, Some(at), |e| {
                    e.path = first;
                    e.paths = Some(all);
                });
            }
        }
    }
}
