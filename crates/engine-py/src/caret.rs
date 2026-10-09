//! 0.5.6 (#162, #165): what the focused text input needs from the window's
//! loop each frame -- the caret's blink, and where the IME should put its
//! candidate window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{NodeKind, Tree};

use crate::timers::SharedAlarm;

/// Moves the focused field's caret through its blink at `now` and wakes the
/// loop for the next change, so an idle field wakes at each edge and not every
/// frame. A field whose blink is off keeps a solid caret. Touches the tree only
/// when the phase changes, so a frame with nothing new is not a redraw.
pub(crate) fn update_blink(tree: &Rc<RefCell<Tree>>, now: Instant, alarm: &SharedAlarm) {
    let Some(field) = tree.borrow().focused() else {
        return;
    };
    let (period, epoch, on) = {
        let tree = tree.borrow();
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            return;
        };
        if state.caret_blink_ms == 0 {
            if state.caret_on {
                return;
            }
            (0, None, false)
        } else {
            (state.caret_blink_ms, state.caret_epoch, state.caret_on)
        }
    };
    if period == 0 {
        // Blinking was switched off while the caret was in its off phase.
        if let Some(NodeKind::TextField(state)) =
            tree.borrow_mut().get_mut(field).map(|n| &mut n.kind)
        {
            state.caret_on = true;
        }
        return;
    }
    let epoch = match epoch {
        Some(epoch) => epoch,
        None => {
            if let Some(NodeKind::TextField(state)) =
                tree.borrow_mut().get_mut(field).map(|n| &mut n.kind)
            {
                state.caret_epoch = Some(now);
            }
            now
        }
    };
    let length = Duration::from_millis(u64::from(period));
    let phase = now.saturating_duration_since(epoch).as_millis() / u128::from(period);
    let should_be_on = phase.is_multiple_of(2);
    if should_be_on != on
        && let Some(NodeKind::TextField(state)) =
            tree.borrow_mut().get_mut(field).map(|n| &mut n.kind)
    {
        state.caret_on = should_be_on;
    }
    if let Some(alarm) = alarm.borrow().as_ref() {
        let next = epoch + length * u32::try_from(phase + 1).unwrap_or(u32::MAX);
        alarm.arm(next);
    }
}

/// What the OS needs to know about the focused text input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImeState {
    /// The caret's box in window coordinates `(x, y, width, height)`, where the
    /// IME puts its candidate window.
    pub(crate) area: (f64, f64, f64, f64),
    /// A password field: the IME should not learn or suggest from it.
    pub(crate) password: bool,
}

/// What `ime_state` last worked out from: shaping a field is the costly part,
/// so it is redone only when something that places the caret changed.
pub(crate) type ImeKey = (engine_core::NodeId, usize, usize, usize, [u64; 5]);

/// The focused text input's `ImeState`, or `None` when no text input has focus.
/// `cache` remembers the last answer and what it was worked out from.
pub(crate) fn ime_state(
    tree: &Rc<RefCell<Tree>>,
    cache: &mut Option<(ImeKey, Option<ImeState>)>,
) -> Option<ImeState> {
    let (field, password, key) = {
        let tree = tree.borrow();
        let Some(field) = tree.focused() else {
            *cache = None;
            return None;
        };
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            *cache = None;
            return None;
        };
        let size = tree.layout(field).size;
        let (x, y) = tree.absolute_position(field);
        let key: ImeKey = (
            field,
            state.cursor,
            state.content.len(),
            state.preedit.as_ref().map_or(0, String::len),
            [
                x.to_bits(),
                y.to_bits(),
                state.horizontal_scroll_offset.current.to_bits()
                    ^ state.scroll_offset.current.to_bits().rotate_left(17),
                u64::from(size.width.to_bits()) << 32 | u64::from(size.height.to_bits()),
                u64::from(state.font_size.to_bits()) << 32
                    | u64::from(state.caret_width.to_bits()) ^ u64::from(state.obscured),
            ],
        );
        (field, state.obscured, key)
    };
    if let Some((last, answer)) = cache
        && *last == key
    {
        return *answer;
    }
    let answer = crate::text_interaction::field_caret_rect(tree, field).map(|rect| {
        let (x, y) = tree.borrow().absolute_position(field);
        ImeState {
            area: (
                x + rect.x0,
                y + rect.y0,
                rect.width().max(1.0),
                rect.height(),
            ),
            password,
        }
    });
    *cache = Some((key, answer));
    answer
}
