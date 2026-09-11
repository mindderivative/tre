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
/// # Errors
/// Returns [`crate::SvgError::MalformedXml`] if `svg_source` isn't
/// well-formed XML at all.
pub fn parse_smil(svg_source: &str) -> Result<ParsedSmil, crate::SvgError> {
    let doc = roxmltree::Document::parse(svg_source)
        .map_err(|e| crate::SvgError::MalformedXml(e.to_string()))?;

    let mut result = ParsedSmil::default();
    for node in doc.descendants() {
        match node.tag_name().name() {
            "animate" => {
                if let Some(animate) = parse_animate(&node) {
                    result.animates.push(animate);
                }
            }
            "animateTransform" => {
                if let Some(translate) = parse_animate_translate(&node) {
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

    #[test]
    fn parses_a_simple_animate_with_from_to() {
        let svg = r#"<svg><rect><animate attributeName="opacity" from="0" to="1" dur="2s"/></rect></svg>"#;
        let parsed = parse_smil(svg).unwrap();
        assert_eq!(parsed.animates.len(), 1);
        let a = &parsed.animates[0];
        assert_eq!(a.attribute_name, "opacity");
        assert_eq!(a.keyframes, vec![0.0, 1.0]);
        assert!((a.duration_seconds - 2.0).abs() <= 1e-5);
    }

    #[test]
    fn parses_an_animate_with_a_real_values_keyframe_list() {
        let svg = r#"<svg><rect><animate attributeName="x" values="0;50;10;100" dur="500ms"/></rect></svg>"#;
        let parsed = parse_smil(svg).unwrap();
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
        let parsed = parse_smil(svg).unwrap();
        assert!((parsed.animates[0].duration_seconds - 3.0).abs() <= 1e-5);
    }

    #[test]
    fn parses_an_animate_transform_translate() {
        let svg = r#"<svg><rect><animateTransform attributeName="transform" type="translate" from="0 0" to="100 50" dur="1s"/></rect></svg>"#;
        let parsed = parse_smil(svg).unwrap();
        assert_eq!(parsed.animate_translates.len(), 1);
        let t = &parsed.animate_translates[0];
        assert_eq!(t.keyframes, vec![[0.0, 0.0], [100.0, 50.0]]);
    }

    #[test]
    fn ignores_animate_transform_with_a_type_other_than_translate() {
        let svg = r#"<svg><rect><animateTransform attributeName="transform" type="rotate" from="0" to="360" dur="1s"/></rect></svg>"#;
        let parsed = parse_smil(svg).unwrap();
        assert!(
            parsed.animate_translates.is_empty(),
            "type=\"rotate\" is a real, disclosed v1 gap"
        );
    }

    #[test]
    fn skips_an_animate_element_missing_a_required_attribute_rather_than_erroring() {
        let svg = r#"<svg><rect><animate attributeName="opacity" from="0"/></rect></svg>"#; // no `to`, no `dur`
        let parsed = parse_smil(svg).unwrap();
        assert!(parsed.animates.is_empty());
    }

    #[test]
    fn malformed_xml_is_rejected_as_a_real_error() {
        let broken = "<svg><rect><animate";
        assert!(matches!(
            parse_smil(broken),
            Err(crate::SvgError::MalformedXml(_))
        ));
    }

    #[test]
    fn a_real_document_with_no_animation_elements_returns_empty_results() {
        let svg = r#"<svg><rect x="0" y="0" width="10" height="10"/></svg>"#;
        let parsed = parse_smil(svg).unwrap();
        assert!(parsed.animates.is_empty());
        assert!(parsed.animate_translates.is_empty());
    }
}
