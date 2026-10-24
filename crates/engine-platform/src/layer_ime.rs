//! 0.5.7 (#199): `zwp_text_input_v3` events for a layer surface, applied the way the protocol
//! says: nothing takes effect until `done`.
//!
//! The input method sends `preedit_string`, `commit_string` and `delete_surrounding_text` as a
//! batch and closes it with `done`. A batch means: remove the preedit that was showing, delete
//! the surrounding text, insert the committed text, then show the new preedit (none, if the
//! batch has no `preedit_string`). This turns a batch into the `InputEvent`s the `winit` path
//! already produces: `ImePreedit` and `TextInput`.

use engine_core::InputEvent;

/// What one `done` asks the app to do, in order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ImeOut {
    /// The composition preview changed; empty text clears it. The range is the byte offsets
    /// of the cursor inside the preview, if the input method gave one.
    Preedit(String, Option<(usize, usize)>),
    /// Text the input method committed.
    Commit(String),
}

impl ImeOut {
    pub(crate) fn into_input(self) -> InputEvent {
        match self {
            ImeOut::Preedit(text, range) => InputEvent::ImePreedit(text, range),
            ImeOut::Commit(text) => InputEvent::TextInput(text),
        }
    }
}

/// The events received since the last `done`, and whether a preview is showing.
#[derive(Default)]
pub(crate) struct ImeSession {
    preedit: Option<(String, i32, i32)>,
    commit: Option<String>,
    showing: bool,
}

impl ImeSession {
    pub(crate) fn preedit_string(&mut self, text: Option<String>, begin: i32, end: i32) {
        self.preedit = text.map(|t| (t, begin, end));
    }

    pub(crate) fn commit_string(&mut self, text: Option<String>) {
        self.commit = text;
    }

    /// Applies the batch: what to tell the app.
    pub(crate) fn done(&mut self) -> Vec<ImeOut> {
        let preedit = self.preedit.take().filter(|(text, _, _)| !text.is_empty());
        let commit = self.commit.take().filter(|text| !text.is_empty());
        let mut out = Vec::new();
        if let Some(text) = commit {
            // The committed text replaces the preview that was showing.
            if self.showing {
                out.push(ImeOut::Preedit(String::new(), None));
                self.showing = false;
            }
            out.push(ImeOut::Commit(text));
        }
        match preedit {
            Some((text, begin, end)) => {
                // A negative offset means the input method hides the cursor.
                let range = (begin >= 0 && end >= 0).then_some((begin as usize, end as usize));
                out.push(ImeOut::Preedit(text, range));
                self.showing = true;
            }
            None if self.showing => {
                out.push(ImeOut::Preedit(String::new(), None));
                self.showing = false;
            }
            None => {}
        }
        out
    }

    /// The text input lost its focus (or was disabled): half-received events are dropped and a
    /// showing preview is cleared.
    pub(crate) fn reset(&mut self) -> Vec<ImeOut> {
        self.preedit = None;
        self.commit = None;
        if std::mem::take(&mut self.showing) {
            vec![ImeOut::Preedit(String::new(), None)]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre(text: &str, begin: i32, end: i32) -> ImeOut {
        ImeOut::Preedit(text.into(), Some((begin as usize, end as usize)))
    }

    #[test]
    fn nothing_happens_until_done() {
        let mut s = ImeSession::default();
        s.preedit_string(Some("か".into()), 0, 3);
        s.commit_string(Some("漢".into()));
        // (no done yet: nothing to read)
        assert_eq!(s.done(), vec![ImeOut::Commit("漢".into()), pre("か", 0, 3)]);
    }

    #[test]
    fn a_preview_is_shown_replaced_and_cleared() {
        let mut s = ImeSession::default();
        s.preedit_string(Some("k".into()), 1, 1);
        assert_eq!(s.done(), vec![pre("k", 1, 1)]);
        s.preedit_string(Some("ka".into()), 2, 2);
        assert_eq!(s.done(), vec![pre("ka", 2, 2)]);
        // A batch with no preedit_string clears the preview.
        assert_eq!(s.done(), vec![ImeOut::Preedit(String::new(), None)]);
        // And clearing nothing says nothing.
        assert_eq!(s.done(), vec![]);
    }

    #[test]
    fn a_commit_clears_the_preview_it_replaces_first() {
        let mut s = ImeSession::default();
        s.preedit_string(Some("kanji".into()), 5, 5);
        s.done();
        s.commit_string(Some("漢字".into()));
        assert_eq!(
            s.done(),
            vec![
                ImeOut::Preedit(String::new(), None),
                ImeOut::Commit("漢字".into())
            ]
        );
    }

    #[test]
    fn a_hidden_cursor_is_no_range_and_an_empty_preedit_is_a_clear() {
        let mut s = ImeSession::default();
        s.preedit_string(Some("あ".into()), -1, -1);
        assert_eq!(s.done(), vec![ImeOut::Preedit("あ".into(), None)]);
        s.preedit_string(Some(String::new()), 0, 0);
        assert_eq!(s.done(), vec![ImeOut::Preedit(String::new(), None)]);
    }

    #[test]
    fn losing_focus_clears_a_showing_preview_and_drops_half_a_batch() {
        let mut s = ImeSession::default();
        s.preedit_string(Some("x".into()), 1, 1);
        s.done();
        s.commit_string(Some("half".into()));
        assert_eq!(s.reset(), vec![ImeOut::Preedit(String::new(), None)]);
        assert_eq!(s.done(), vec![]);
        assert_eq!(s.reset(), vec![]);
    }

    #[test]
    fn the_events_become_the_engines_inputs() {
        assert_eq!(
            ImeOut::Commit("a".into()).into_input(),
            InputEvent::TextInput("a".into())
        );
        assert_eq!(
            pre("b", 0, 1).into_input(),
            InputEvent::ImePreedit("b".into(), Some((0, 1)))
        );
    }
}
