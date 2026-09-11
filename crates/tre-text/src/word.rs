//! Real word-boundary caret jumps (Phase 15 Step 15.4) -- pure logic
//! over Unicode Text Segmentation (UAX #29), via `unicode-segmentation`.
//! No font/shaping dependency at all, unlike `caret`/`wrap` -- word
//! boundaries are a property of the text itself, not its rendered
//! glyphs, so these are real, plain, directly-testable string
//! functions (no synthetic glyph data needed at all, unlike
//! `caret.rs`'s own tests).
//!
//! `unicode_word_indices` (not `split_word_bound_indices`) is used
//! deliberately: it already filters out pure whitespace/punctuation
//! segments, returning only real word-content runs -- exactly what a
//! Ctrl+arrow jump should land on/skip past, with no extra filtering
//! needed here.

use unicode_segmentation::UnicodeSegmentation;

/// The real byte offset of the END of the next real word (per UAX #29)
/// strictly after `from`, or `None` if there is no such word (`from`
/// is already at or past the last real word in `text`).
#[must_use]
pub fn next_word_end(text: &str, from: usize) -> Option<usize> {
    text.unicode_word_indices()
        .map(|(start, word)| start + word.len())
        .find(|&end| end > from)
}

/// The real byte offset of the START of the previous real word
/// strictly before `from`, or `None` if there is no such word.
#[must_use]
pub fn prev_word_start(text: &str, from: usize) -> Option<usize> {
    text.unicode_word_indices()
        .map(|(start, _)| start)
        .take_while(|&start| start < from)
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_word_end_skips_the_gap_between_words() {
        let text = "hello world";
        assert_eq!(
            next_word_end(text, 0),
            Some(5),
            "lands at the end of 'hello'"
        );
        assert_eq!(
            next_word_end(text, 5),
            Some(11),
            "from mid-gap, skips straight to the end of 'world', not its own start"
        );
        assert_eq!(
            next_word_end(text, 11),
            None,
            "no real word left once already at the end of the last one"
        );
    }

    #[test]
    fn prev_word_start_skips_the_gap_between_words() {
        let text = "hello world";
        assert_eq!(
            prev_word_start(text, 11),
            Some(6),
            "lands at the start of 'world'"
        );
        assert_eq!(
            prev_word_start(text, 6),
            Some(0),
            "from mid-gap (right at 'world's own start), skips back to 'hello's own start"
        );
        assert_eq!(
            prev_word_start(text, 0),
            None,
            "no real word left before the very start"
        );
    }

    #[test]
    fn a_real_accented_multi_byte_word_is_treated_as_one_real_word() {
        // "café" -- 'é' is a real 2-byte UTF-8 character; UAX #29 must
        // treat the whole word as one unit, not split at the accent.
        let text = "café résumé";
        let cafe_end = "café".len();
        assert_eq!(next_word_end(text, 0), Some(cafe_end));
        assert_eq!(prev_word_start(text, text.len()), Some(cafe_end + 1));
    }

    #[test]
    fn an_apostrophe_inside_a_word_does_not_split_it() {
        // Real UAX #29 rule: a mid-word apostrophe (as in a contraction)
        // does not itself create a word boundary.
        let text = "don't stop";
        assert_eq!(
            next_word_end(text, 0),
            Some("don't".len()),
            "\"don't\" must be one real word, not split at the apostrophe"
        );
    }

    #[test]
    fn punctuation_between_words_is_skipped_as_a_real_non_word_segment() {
        let text = "hello, world!";
        assert_eq!(
            next_word_end(text, 0),
            Some(5),
            "'hello' ends before the comma"
        );
        assert_eq!(
            next_word_end(text, 5),
            Some(12),
            "the next real word 'world' ends before the '!', skipping ', ' entirely"
        );
        assert_eq!(
            next_word_end(text, 12),
            None,
            "'!' alone is not a real word"
        );
    }
}
