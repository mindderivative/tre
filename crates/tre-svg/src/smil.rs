//! Real SMIL `<animate>`/`<animateTransform type="translate">` parsing
//! (Phase 13 Step 13.7, Q4). `usvg` -- the parser this crate's own
//! `parse_svg` uses for everything else -- does not process SMIL
//! animation elements at all: it is a static-resolution parser by
//! design (the same real library `resvg` itself uses for static
//! rendering), confirmed by reading its own source before writing this
//! module rather than assumed. Real SMIL support therefore needs a
//! separate, direct XML pass over the raw document, via `roxmltree`
//! (already a transitive dependency of `usvg` itself, now pinned here
//! directly).
//!
//! **Real, disclosed v1 scope** -- deliberately not full SMIL spec
//! compliance, which is a large surface (motion-path animation, complex
//! `begin`/sync timing, `<animateColor>`, additive/accumulative
//! animation): this covers the common real case, `<animate>` (a single
//! scalar attribute, e.g. `opacity`) and `<animateTransform
//! type="translate">` (a 2D point), each with either `values="a;b;c"`
//! (a real keyframe list) or `from`/`to` (treated as a real 2-keyframe
//! list), plus `dur` (accepting `"Ns"`, `"Nms"`, or a bare number
//! treated as seconds). `begin`, `repeatCount`, `calcMode`,
//! `animateTransform type="scale"/"rotate"`, and `<animateMotion>` are
//! all real, disclosed gaps -- not attempted here.
//!
//! A caller drives the extracted keyframes through `tre-animation`'s
//! own real `Timeline`/`Tween` machinery (Phase 13 Steps 13.2/13.3) --
//! this module only extracts the real data SMIL authors already wrote
//! into the document; it does not itself sequence or sample anything.

/// One real `<animate>` directive: a single scalar attribute animated
/// across `keyframes` (at least 2 values) over `duration_seconds`.
#[derive(Debug, Clone, PartialEq)]
pub struct SmilAnimate {
    pub attribute_name: String,
    pub keyframes: Vec<f32>,
    pub duration_seconds: f32,
}

/// One real `<animateTransform type="translate">` directive: a 2D point
/// animated across `keyframes` (at least 2 points) over
/// `duration_seconds`.
#[derive(Debug, Clone, PartialEq)]
pub struct SmilAnimateTranslate {
    pub keyframes: Vec<[f32; 2]>,
    pub duration_seconds: f32,
}

/// Every real SMIL animation directive found in one document, in
/// document order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedSmil {
    pub animates: Vec<SmilAnimate>,
    pub animate_translates: Vec<SmilAnimateTranslate>,
}

/// Parses `svg_source` for real `<animate>`/`<animateTransform
/// type="translate">` elements -- see this module's own doc comment for
/// the real, disclosed v1 scope. An element missing a required
/// attribute (`attributeName`/`dur`/(`values` or `from`+`to`)), or one
/// whose numeric attributes don't parse as real numbers, is silently
/// skipped rather than rejecting the whole document -- matching a real
/// SVG author's own expectation that one malformed animation directive
/// among many shouldn't break every other one.
///
/// Hardened against untrusted input the same way [`crate::parse_svg`] is
/// (Security review finding: this parser previously took an unbounded
/// `&str` straight into `roxmltree` with no size or output-count cap at
/// all): `max_bytes` is checked before `roxmltree::Document::parse` ever
/// sees the data, and `max_keyframes` bounds the total keyframe count
/// extracted across the *whole document* (summed across every real
/// `<animate>`/`<animateTransform>` element found), checked
/// incrementally as each element is parsed rather than only after fully
/// resolving a pathological document first. Unlike `parse_svg`'s own
/// `max_points`, no curve-flattening-style expansion happens here (one
/// SMIL keyframe is exactly one already-present numeric token in the
/// source), so `max_bytes` alone already bounds worst-case keyframe
/// count too -- `max_keyframes` exists to give a caller a real, separate,
/// independently-tunable ceiling on the output size, matching the
/// established two-budget shape rather than relying on the byte cap
/// alone. Bounds this crate's own logic only: the initial
/// `roxmltree::Document::parse` call itself has no further hardening
/// beyond the `max_bytes` gate before it runs, the same "rely on the
/// underlying parser's own hardening once past our own size gate"
/// posture `parse_svg` already takes for `usvg`.
///
/// # Errors
/// Returns [`crate::SvgError::TooLarge`] if `svg_source.len()` exceeds
/// `max_bytes`. Returns [`crate::SvgError::MalformedXml`] if
/// `svg_source` isn't well-formed XML at all. Returns
/// [`crate::SvgError::TooManyKeyframes`] if the total extracted keyframe
/// count exceeds `max_keyframes`.
pub fn parse_smil(
    svg_source: &str,
    max_bytes: usize,
    max_keyframes: usize,
) -> Result<ParsedSmil, crate::SvgError> {
    if svg_source.len() > max_bytes {
        return Err(crate::SvgError::TooLarge {
            size: svg_source.len(),
            max: max_bytes,
        });
    }

    let doc = roxmltree::Document::parse(svg_source)
        .map_err(|e| crate::SvgError::MalformedXml(e.to_string()))?;

    let mut result = ParsedSmil::default();
    let mut keyframe_budget = 0usize;
    for node in doc.descendants() {
        match node.tag_name().name() {
            "animate" => {
                if let Some(animate) = parse_animate(&node) {
                    keyframe_budget += animate.keyframes.len();
                    if keyframe_budget > max_keyframes {
                        return Err(crate::SvgError::TooManyKeyframes {
                            count: keyframe_budget,
                            max: max_keyframes,
                        });
                    }
                    result.animates.push(animate);
                }
            }
            "animateTransform" => {
                if let Some(translate) = parse_animate_translate(&node) {
                    keyframe_budget += translate.keyframes.len();
                    if keyframe_budget > max_keyframes {
                        return Err(crate::SvgError::TooManyKeyframes {
                            count: keyframe_budget,
                            max: max_keyframes,
                        });
                    }
                    result.animate_translates.push(translate);
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

/// Parses a `dur` attribute value: `"2s"`, `"500ms"`, or a bare `"2"`
/// (treated as seconds, a common real-world shorthand even though the
/// SMIL spec technically requires an explicit unit).
fn parse_dur(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    if let Some(ms) = trimmed.strip_suffix("ms") {
        ms.trim().parse::<f32>().ok().map(|v| v / 1000.0)
    } else if let Some(secs) = trimmed.strip_suffix('s') {
        secs.trim().parse::<f32>().ok()
    } else {
        trimmed.parse::<f32>().ok()
    }
}

fn parse_animate(node: &roxmltree::Node) -> Option<SmilAnimate> {
    let attribute_name = node.attribute("attributeName")?.to_string();
    let duration_seconds = parse_dur(node.attribute("dur")?)?;
    let keyframes = if let Some(values) = node.attribute("values") {
        values
            .split(';')
            .map(|v| v.trim().parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()?
    } else {
        let from = node.attribute("from")?.trim().parse::<f32>().ok()?;
        let to = node.attribute("to")?.trim().parse::<f32>().ok()?;
        vec![from, to]
    };
    if keyframes.len() < 2 {
        return None;
    }
    Some(SmilAnimate {
        attribute_name,
        keyframes,
        duration_seconds,
    })
}

/// Parses `"x y"` or `"x,y"` (both real, valid SMIL number-list
/// separators) into a real 2D point.
fn parse_point(raw: &str) -> Option<[f32; 2]> {
    let normalized = raw.replace(',', " ");
    let mut parts = normalized.split_whitespace();
    let x = parts.next()?.parse::<f32>().ok()?;
    let y = parts.next()?.parse::<f32>().ok()?;
    Some([x, y])
}

fn parse_animate_translate(node: &roxmltree::Node) -> Option<SmilAnimateTranslate> {
    if node.attribute("type") != Some("translate") {
        return None;
    }
    let duration_seconds = parse_dur(node.attribute("dur")?)?;
    let keyframes = if let Some(values) = node.attribute("values") {
        values
            .split(';')
            .map(|v| parse_point(v.trim()))
            .collect::<Option<Vec<[f32; 2]>>>()?
    } else {
        let from = parse_point(node.attribute("from")?)?;
        let to = parse_point(node.attribute("to")?)?;
        vec![from, to]
    };
    if keyframes.len() < 2 {
        return None;
    }
    Some(SmilAnimateTranslate {
        keyframes,
        duration_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generous test-only defaults -- matching `parse_svg`'s own test
    /// module literals (`1_000_000`/`10_000`), comfortably above
    /// anything these small, hand-authored test documents ever produce.
    const MAX_BYTES: usize = 1_000_000;
    const MAX_KEYFRAMES: usize = 10_000;

    #[test]
    fn parses_a_simple_animate_with_from_to() {
        let svg = r#"<svg><rect><animate attributeName="opacity" from="0" to="1" dur="2s"/></rect></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert_eq!(parsed.animates.len(), 1);
        let a = &parsed.animates[0];
        assert_eq!(a.attribute_name, "opacity");
        assert_eq!(a.keyframes, vec![0.0, 1.0]);
        assert!((a.duration_seconds - 2.0).abs() <= 1e-5);
    }

    #[test]
    fn parses_an_animate_with_a_real_values_keyframe_list() {
        let svg = r#"<svg><rect><animate attributeName="x" values="0;50;10;100" dur="500ms"/></rect></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert_eq!(parsed.animates.len(), 1);
        let a = &parsed.animates[0];
        assert_eq!(a.keyframes, vec![0.0, 50.0, 10.0, 100.0]);
        assert!(
            (a.duration_seconds - 0.5).abs() <= 1e-5,
            "500ms must parse to 0.5s"
        );
    }

    #[test]
    fn parses_a_bare_numeric_dur_as_seconds() {
        let svg = r#"<svg><rect><animate attributeName="x" from="0" to="1" dur="3"/></rect></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert!((parsed.animates[0].duration_seconds - 3.0).abs() <= 1e-5);
    }

    #[test]
    fn parses_an_animate_transform_translate() {
        let svg = r#"<svg><rect><animateTransform attributeName="transform" type="translate" from="0 0" to="100 50" dur="1s"/></rect></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert_eq!(parsed.animate_translates.len(), 1);
        let t = &parsed.animate_translates[0];
        assert_eq!(t.keyframes, vec![[0.0, 0.0], [100.0, 50.0]]);
    }

    #[test]
    fn ignores_animate_transform_with_a_type_other_than_translate() {
        let svg = r#"<svg><rect><animateTransform attributeName="transform" type="rotate" from="0" to="360" dur="1s"/></rect></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert!(
            parsed.animate_translates.is_empty(),
            "type=\"rotate\" is a real, disclosed v1 gap"
        );
    }

    #[test]
    fn skips_an_animate_element_missing_a_required_attribute_rather_than_erroring() {
        let svg = r#"<svg><rect><animate attributeName="opacity" from="0"/></rect></svg>"#; // no `to`, no `dur`
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert!(parsed.animates.is_empty());
    }

    #[test]
    fn malformed_xml_is_rejected_as_a_real_error() {
        let broken = "<svg><rect><animate";
        assert!(matches!(
            parse_smil(broken, MAX_BYTES, MAX_KEYFRAMES),
            Err(crate::SvgError::MalformedXml(_))
        ));
    }

    #[test]
    fn a_real_document_with_no_animation_elements_returns_empty_results() {
        let svg = r#"<svg><rect x="0" y="0" width="10" height="10"/></svg>"#;
        let parsed = parse_smil(svg, MAX_BYTES, MAX_KEYFRAMES).unwrap();
        assert!(parsed.animates.is_empty());
        assert!(parsed.animate_translates.is_empty());
    }

    #[test]
    fn parse_smil_rejects_oversized_input_before_touching_roxmltree() {
        let svg = r#"<svg><rect><animate attributeName="opacity" from="0" to="1" dur="2s"/></rect></svg>"#;
        let result = parse_smil(svg, 4, MAX_KEYFRAMES);
        assert!(matches!(result, Err(crate::SvgError::TooLarge { .. })));
    }

    #[test]
    fn parse_smil_rejects_a_document_with_too_many_keyframes() {
        // A single `values` list with far more than 4 keyframes, well
        // under `MAX_BYTES` -- this must be caught by this crate's own
        // keyframe-count ceiling, not by running out of memory first.
        let values = (0..2000)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(";");
        let svg = format!(
            r#"<svg><rect><animate attributeName="x" values="{values}" dur="1s"/></rect></svg>"#
        );
        let result = parse_smil(&svg, MAX_BYTES, 100);
        assert!(matches!(
            result,
            Err(crate::SvgError::TooManyKeyframes { .. })
        ));
    }

    #[test]
    fn parse_smil_sums_the_keyframe_budget_across_multiple_elements() {
        // Neither element alone exceeds the budget, but their combined
        // total does -- the budget must be threaded across the whole
        // document, not reset per element.
        let svg = r#"<svg><rect>
            <animate attributeName="x" values="0;1;2;3;4;5" dur="1s"/>
            <animate attributeName="y" values="0;1;2;3;4;5" dur="1s"/>
        </rect></svg>"#;
        let result = parse_smil(svg, MAX_BYTES, 8);
        assert!(matches!(
            result,
            Err(crate::SvgError::TooManyKeyframes { .. })
        ));
    }
}

/// This crate's own "fuzz-test the parser with malformed and adversarial
/// documents, asserting bounded time and memory regardless of input"
/// discipline (`lib.rs`'s own `mod proptests` doc comment), extended to
/// `parse_smil` -- previously the one parser in this crate with no
/// proptest coverage at all (the Security review's own finding).
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::time::{Duration, Instant};

    /// Same generous, coarse bound as `lib.rs`'s own `BOUNDED_TIME` --
    /// this exists to catch a real hang/blowup, not to enforce a tight
    /// performance budget.
    const BOUNDED_TIME: Duration = Duration::from_secs(2);
    const MAX_BYTES: usize = 1_000_000;
    const MAX_KEYFRAMES: usize = 10_000;

    proptest! {
        /// Pure random bytes, no SVG/XML structure at all -- exercises
        /// `roxmltree`'s own malformed-XML rejection path, surfaced as
        /// `Err(SvgError::MalformedXml(..))`, never a panic or a hang.
        #[test]
        fn parse_smil_never_panics_or_hangs_on_arbitrary_bytes(
            bytes in proptest::collection::vec(any::<u8>(), 0..4096)
        ) {
            // Real SMIL parsing takes `&str`, not `&[u8]` -- a caller
            // (`tre-python`'s own `parse_smil` wrapper) already rejects
            // non-UTF-8 input before ever reaching this function, so
            // `from_utf8_lossy` here (rather than skipping non-UTF-8
            // cases) still exercises this function's own real logic
            // against adversarial-looking text instead of silently
            // discarding most of the generated corpus.
            let text = String::from_utf8_lossy(&bytes);
            let start = Instant::now();
            let _ = parse_smil(&text, MAX_BYTES, MAX_KEYFRAMES);
            prop_assert!(
                start.elapsed() < BOUNDED_TIME,
                "parse_smil took {:?} on {} bytes of pure random input -- expected bounded, not \
                 unbounded, worst-case time regardless of how malformed the input is",
                start.elapsed(),
                bytes.len()
            );
        }

        /// A real SMIL/XML skeleton (so `roxmltree` actually engages,
        /// not just its own top-level parse-failure path), but with a
        /// randomly-generated `values` attribute -- extreme, huge, or
        /// malformed-looking numeric lists are exactly the "adversarial
        /// document" shape this crate's own `parse_svg` proptests
        /// already stress its own point budget with, applied here to
        /// this parser's own keyframe budget instead.
        #[test]
        fn parse_smil_never_panics_or_hangs_on_random_values_attribute(
            values in "[0-9.;\\- ]{0,2000}"
        ) {
            let svg = format!(
                r#"<svg><rect><animate attributeName="x" values="{values}" dur="1s"/></rect></svg>"#
            );
            let start = Instant::now();
            let _ = parse_smil(&svg, MAX_BYTES, MAX_KEYFRAMES);
            prop_assert!(
                start.elapsed() < BOUNDED_TIME,
                "parse_smil took {:?} on a random values attribute of length {} -- expected \
                 bounded, not unbounded, worst-case time",
                start.elapsed(),
                values.len()
            );
        }
    }
}
