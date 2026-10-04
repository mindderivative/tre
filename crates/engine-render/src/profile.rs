//! 0.5.4 (#135): where a frame's scene-building time went, by node.
//!
//! Switched on, the paint walk times each node it reaches (its own work: the
//! walk's decision, its layers and its drawing, not its children's, which are
//! timed as their own nodes) and a [`FrameProfile`] adds the times up by node
//! kind and keeps the slowest few nodes. It costs two clock reads a node, so
//! it is off unless asked for. What it times is the CPU work of encoding the
//! scene; the GPU's time is per frame (`GpuTimer`), not per node.

use std::time::Duration;

use engine_core::NodeId;

/// How many of the slowest nodes a profile keeps.
pub const SLOWEST: usize = 10;

/// What nodes of one kind cost in a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KindCost {
    /// Nodes of the kind the walk reached.
    pub reached: usize,
    /// Of those, how many drew themselves (the rest were outside the damage).
    pub drawn: usize,
    pub time: Duration,
}

/// One node's own time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeCost {
    pub id: NodeId,
    pub kind: &'static str,
    pub drawn: bool,
    pub time: Duration,
}

/// A frame's scene-building time, attributed.
#[derive(Clone, Debug, Default)]
pub struct FrameProfile {
    /// Per kind, in the order first met.
    pub by_kind: Vec<(&'static str, KindCost)>,
    /// The slowest nodes, slowest first.
    pub slowest: Vec<NodeCost>,
    /// Nodes the walk reached, and the time spent in all of them.
    pub reached: usize,
    pub time: Duration,
}

impl FrameProfile {
    /// Adds one node's time.
    pub fn add(&mut self, id: NodeId, kind: &'static str, drawn: bool, time: Duration) {
        self.reached += 1;
        self.time += time;
        let entry = match self.by_kind.iter().position(|(name, _)| *name == kind) {
            Some(i) => &mut self.by_kind[i].1,
            None => {
                self.by_kind.push((kind, KindCost::default()));
                &mut self.by_kind.last_mut().expect("just pushed").1
            }
        };
        entry.reached += 1;
        entry.drawn += usize::from(drawn);
        entry.time += time;
        let cost = NodeCost {
            id,
            kind,
            drawn,
            time,
        };
        let at = self.slowest.partition_point(|n| n.time >= time);
        if at < SLOWEST {
            self.slowest.insert(at, cost);
            self.slowest.truncate(SLOWEST);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{NodeKind, PaintProperties, Tree};
    use peniko::Color;
    use taffy::prelude::Style;

    fn ids(n: usize) -> Vec<NodeId> {
        let mut tree = Tree::new();
        (0..n)
            .map(|_| {
                tree.insert(
                    NodeKind::Rect,
                    Style::default(),
                    PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
                )
            })
            .collect()
    }

    #[test]
    fn times_add_up_by_kind_and_in_total() {
        let ids = ids(3);
        let mut p = FrameProfile::default();
        let ms = Duration::from_millis;
        p.add(ids[0], "box", true, ms(2));
        p.add(ids[1], "text", true, ms(5));
        p.add(ids[2], "box", false, ms(1));
        assert_eq!((p.reached, p.time), (3, ms(8)));
        let boxes = p.by_kind.iter().find(|(k, _)| *k == "box").unwrap().1;
        assert_eq!((boxes.reached, boxes.drawn, boxes.time), (2, 1, ms(3)));
        assert_eq!(p.by_kind.len(), 2);
    }

    #[test]
    fn only_the_slowest_few_are_kept_slowest_first() {
        let ids = ids(SLOWEST + 5);
        let mut p = FrameProfile::default();
        for (i, id) in ids.iter().enumerate() {
            p.add(*id, "box", true, Duration::from_micros(i as u64 + 1));
        }
        assert_eq!(p.slowest.len(), SLOWEST);
        let times: Vec<u128> = p.slowest.iter().map(|n| n.time.as_micros()).collect();
        let mut sorted = times.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(times, sorted);
        assert_eq!(times[0], (SLOWEST + 5) as u128, "the slowest of all");
        assert_eq!(p.slowest[0].id, ids[SLOWEST + 4]);
    }
}
