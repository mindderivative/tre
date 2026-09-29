//! 0.4.2 M11 (issue #23): CSS Grid's track lists and placements, parsed
//! from and formatted back to the text `node.set`/`node.get` use.
//!
//! The syntax is CSS's, in `tre`'s units: a plain number is pixels.
//!
//! - A track: `200` (pixels), `"25%"`, `"1fr"`, `"auto"`, `"min_content"`,
//!   `"max_content"`, `"minmax(a, b)"`, or `"fit_content(x)"` (hyphens, as
//!   in CSS, work too).
//! - A template: tracks separated by spaces, and `"repeat(n, tracks)"`, with
//!   `n` a count, `auto_fill`, or `auto_fit`.
//! - A placement: a line (`2`, `-1`), `"span 2"`, `"auto"`, or two of them
//!   as `"start / end"`.
//!
//! Named lines and template areas aren't supported yet. Everything here is
//! plain Rust, independent of Python: `node_layout` converts Python values
//! to the text these functions take.

use taffy::geometry::Line;
use taffy::style::{
    ExpandedMaxTrackSizingFunction, ExpandedMinTrackSizingFunction, GridPlacement,
    GridTemplateComponent, GridTemplateRepetition, LengthPercentage, MaxTrackSizingFunction,
    MinTrackSizingFunction, RepetitionCount, TrackSizingFunction,
};
use taffy::style_helpers::{fit_content, line, minmax, span};

/// One track's size, or the start of `minmax`/`fit_content`'s arguments.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Breadth {
    Px(f32),
    Percent(f32),
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
}

fn number(text: &str) -> Option<f32> {
    text.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn breadth(text: &str) -> Result<Breadth, String> {
    let text = text.trim();
    let keyword = text.replace('-', "_");
    Ok(match keyword.as_str() {
        "auto" => Breadth::Auto,
        "min_content" => Breadth::MinContent,
        "max_content" => Breadth::MaxContent,
        _ => {
            if let Some(v) = text.strip_suffix('%').and_then(number) {
                Breadth::Percent(v / 100.0)
            } else if let Some(v) = text.strip_suffix("fr").and_then(number) {
                Breadth::Fr(v)
            } else if let Some(v) = number(text) {
                Breadth::Px(v)
            } else {
                return Err(format!("{text:?} isn't a track size"));
            }
        }
    })
}

fn min_of(b: Breadth) -> Result<MinTrackSizingFunction, String> {
    Ok(match b {
        Breadth::Px(v) => MinTrackSizingFunction::length(v),
        Breadth::Percent(v) => MinTrackSizingFunction::percent(v),
        Breadth::Auto => MinTrackSizingFunction::auto(),
        Breadth::MinContent => MinTrackSizingFunction::min_content(),
        Breadth::MaxContent => MinTrackSizingFunction::max_content(),
        Breadth::Fr(_) => return Err("a minimum can't be in `fr`".into()),
    })
}

fn max_of(b: Breadth) -> MaxTrackSizingFunction {
    match b {
        Breadth::Px(v) => MaxTrackSizingFunction::length(v),
        Breadth::Percent(v) => MaxTrackSizingFunction::percent(v),
        Breadth::Fr(v) => MaxTrackSizingFunction::fr(v),
        Breadth::Auto => MaxTrackSizingFunction::auto(),
        Breadth::MinContent => MaxTrackSizingFunction::min_content(),
        Breadth::MaxContent => MaxTrackSizingFunction::max_content(),
    }
}

/// `text` as `name(args)`, its arguments split at top-level commas.
fn call<'t>(text: &'t str, names: &[&str]) -> Option<Vec<&'t str>> {
    let open = text.find('(')?;
    let name = text[..open].trim().replace('-', "_");
    if !names.contains(&name.as_str()) || !text.ends_with(')') {
        return None;
    }
    Some(split_top_level(&text[open + 1..text.len() - 1], ','))
}

/// `text` split at `sep` outside parentheses (a space splits at runs of
/// whitespace), each part trimmed, empty ones dropped.
fn split_top_level(text: &str, sep: char) -> Vec<&str> {
    let (mut parts, mut depth, mut start) = (Vec::new(), 0usize, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 && (c == sep || (sep == ' ' && c.is_whitespace())) => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// One track: a size, `minmax(min, max)`, or `fit_content(limit)`.
pub(crate) fn parse_track(text: &str) -> Result<TrackSizingFunction, String> {
    let text = text.trim();
    if let Some(args) = call(text, &["minmax"]) {
        let [min, max] = args[..] else {
            return Err(format!("{text:?}: minmax takes two sizes"));
        };
        return Ok(minmax(min_of(breadth(min)?)?, max_of(breadth(max)?)));
    }
    if let Some(args) = call(text, &["fit_content"]) {
        let [limit] = args[..] else {
            return Err(format!("{text:?}: fit_content takes one size"));
        };
        let limit = match breadth(limit)? {
            Breadth::Px(v) => LengthPercentage::length(v),
            Breadth::Percent(v) => LengthPercentage::percent(v),
            _ => {
                return Err(format!(
                    "{text:?}: fit_content takes pixels or a percentage"
                ));
            }
        };
        return Ok(fit_content(limit));
    }
    let b = breadth(text)?;
    Ok(match b {
        // `1fr` is CSS's `minmax(auto, 1fr)`.
        Breadth::Fr(_) => minmax(MinTrackSizingFunction::auto(), max_of(b)),
        _ => minmax(min_of(b)?, max_of(b)),
    })
}

/// A template's tracks: sizes and `repeat(...)` groups.
pub(crate) fn parse_template(text: &str) -> Result<Vec<GridTemplateComponent<String>>, String> {
    split_top_level(text, ' ')
        .into_iter()
        .map(|item| {
            if let Some(args) = call(item, &["repeat"]) {
                let Some((count, tracks)) = args.split_first() else {
                    return Err(format!("{item:?}: repeat needs a count and tracks"));
                };
                let count = match count.replace('-', "_").as_str() {
                    "auto_fill" => RepetitionCount::AutoFill,
                    "auto_fit" => RepetitionCount::AutoFit,
                    n => RepetitionCount::Count(
                        n.parse::<u16>().ok().filter(|n| *n > 0).ok_or_else(|| {
                            format!("{item:?}: a repeat count must be at least 1")
                        })?,
                    ),
                };
                let tracks: Vec<TrackSizingFunction> = tracks
                    .iter()
                    .flat_map(|t| split_top_level(t, ' '))
                    .map(parse_track)
                    .collect::<Result<_, _>>()?;
                if tracks.is_empty() {
                    return Err(format!("{item:?}: repeat needs at least one track"));
                }
                Ok(GridTemplateComponent::Repeat(GridTemplateRepetition {
                    count,
                    tracks,
                    line_names: Vec::new(),
                }))
            } else {
                parse_track(item).map(GridTemplateComponent::Single)
            }
        })
        .collect()
}

/// Implicit tracks (`grid_auto_rows`/`columns`): sizes, no `repeat`.
pub(crate) fn parse_auto_tracks(text: &str) -> Result<Vec<TrackSizingFunction>, String> {
    split_top_level(text, ' ')
        .into_iter()
        .map(parse_track)
        .collect()
}

fn placement(text: &str) -> Result<GridPlacement<String>, String> {
    let text = text.trim();
    if text == "auto" {
        return Ok(GridPlacement::Auto);
    }
    if let Some(n) = text.strip_prefix("span") {
        return n
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|n| *n > 0)
            .map(span)
            .ok_or_else(|| format!("{text:?}: a span must be at least 1"));
    }
    text.parse::<i16>()
        .ok()
        .filter(|n| *n != 0)
        .map(line)
        .ok_or_else(|| format!("{text:?} isn't a line (a non-zero number), a span, or auto"))
}

/// A child's placement on one axis: `start`, or `start / end`.
pub(crate) fn parse_placement(text: &str) -> Result<Line<GridPlacement<String>>, String> {
    let parts: Vec<&str> = text.split('/').collect();
    match parts[..] {
        [start] => Ok(Line {
            start: placement(start)?,
            end: GridPlacement::Auto,
        }),
        [start, end] => Ok(Line {
            start: placement(start)?,
            end: placement(end)?,
        }),
        _ => Err(format!("{text:?}: at most one `/`")),
    }
}

/// A number as `tre` writes one: whole numbers without a decimal point.
fn num(v: f32) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn min_text(m: MinTrackSizingFunction) -> String {
    match m.expand() {
        ExpandedMinTrackSizingFunction::Length(v) => num(v),
        ExpandedMinTrackSizingFunction::Percent(v) => format!("{}%", num(v * 100.0)),
        ExpandedMinTrackSizingFunction::Auto => "auto".into(),
        ExpandedMinTrackSizingFunction::MinContent => "min_content".into(),
        ExpandedMinTrackSizingFunction::MaxContent => "max_content".into(),
        // `tre` never sets a calc() track.
        #[allow(unreachable_patterns)]
        _ => "auto".into(),
    }
}

fn max_text(m: MaxTrackSizingFunction) -> String {
    match m.expand() {
        ExpandedMaxTrackSizingFunction::Length(v) => num(v),
        ExpandedMaxTrackSizingFunction::Percent(v) => format!("{}%", num(v * 100.0)),
        ExpandedMaxTrackSizingFunction::Fr(v) => format!("{}fr", num(v)),
        ExpandedMaxTrackSizingFunction::Auto => "auto".into(),
        ExpandedMaxTrackSizingFunction::MinContent => "min_content".into(),
        ExpandedMaxTrackSizingFunction::MaxContent => "max_content".into(),
        ExpandedMaxTrackSizingFunction::FitContentPx(v) => format!("fit_content({})", num(v)),
        ExpandedMaxTrackSizingFunction::FitContentPercent(v) => {
            format!("fit_content({}%)", num(v * 100.0))
        }
        #[allow(unreachable_patterns)]
        _ => "auto".into(),
    }
}

/// One track as `parse_track` takes it, in its shortest form.
pub(crate) fn track_text(t: &TrackSizingFunction) -> String {
    let (min, max) = (min_text(t.min), max_text(t.max));
    let auto_min = matches!(t.min.expand(), ExpandedMinTrackSizingFunction::Auto);
    if auto_min
        && matches!(
            t.max.expand(),
            ExpandedMaxTrackSizingFunction::Fr(_)
                | ExpandedMaxTrackSizingFunction::FitContentPx(_)
                | ExpandedMaxTrackSizingFunction::FitContentPercent(_)
        )
    {
        return max;
    }
    if min == max {
        min
    } else {
        format!("minmax({min}, {max})")
    }
}

pub(crate) fn template_text(tracks: &[GridTemplateComponent<String>]) -> String {
    tracks
        .iter()
        .map(|c| match c {
            GridTemplateComponent::Single(t) => track_text(t),
            GridTemplateComponent::Repeat(r) => {
                let count = match r.count {
                    RepetitionCount::AutoFill => "auto_fill".to_string(),
                    RepetitionCount::AutoFit => "auto_fit".to_string(),
                    RepetitionCount::Count(n) => n.to_string(),
                };
                let inner: Vec<String> = r.tracks.iter().map(track_text).collect();
                format!("repeat({count}, {})", inner.join(" "))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn auto_tracks_text(tracks: &[TrackSizingFunction]) -> String {
    tracks.iter().map(track_text).collect::<Vec<_>>().join(" ")
}

fn placement_text(p: &GridPlacement<String>) -> String {
    match p {
        GridPlacement::Auto => "auto".into(),
        GridPlacement::Line(l) => l.as_i16().to_string(),
        GridPlacement::Span(n) => format!("span {n}"),
        // `tre` never sets a named line.
        GridPlacement::NamedLine(name, _) | GridPlacement::NamedSpan(name, _) => name.clone(),
    }
}

/// A placement as `parse_placement` takes it: `start` alone when the end is
/// automatic.
pub(crate) fn placement_text_line(p: &Line<GridPlacement<String>>) -> String {
    match p.end {
        GridPlacement::Auto => placement_text(&p.start),
        _ => format!("{} / {}", placement_text(&p.start), placement_text(&p.end)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str) -> String {
        template_text(&parse_template(text).expect(text))
    }

    #[test]
    fn templates_read_back_as_written() {
        for text in [
            "200 1fr auto",
            "repeat(3, 1fr)",
            "minmax(120, 1fr) 25%",
            "min_content max_content fit_content(300)",
            "repeat(auto_fill, minmax(100, 1fr))",
            "1.5fr 40",
        ] {
            assert_eq!(round_trip(text), text);
        }
    }

    #[test]
    fn css_spellings_are_accepted() {
        assert_eq!(
            round_trip("min-content  fit-content(50%)"),
            "min_content fit_content(50%)"
        );
        assert_eq!(round_trip("repeat(auto-fit, 1fr)"), "repeat(auto_fit, 1fr)");
    }

    #[test]
    fn bad_tracks_say_why() {
        for (text, why) in [
            ("1em", "isn't a track size"),
            ("minmax(1fr, 2fr)", "can't be in `fr`"),
            ("repeat(0, 1fr)", "at least 1"),
            ("-10", "isn't a track size"),
            ("minmax(10)", "two sizes"),
        ] {
            let err = parse_template(text).unwrap_err();
            assert!(err.contains(why), "{text}: {err}");
        }
    }

    #[test]
    fn placements_read_back_as_written() {
        for text in ["2", "-1", "span 2", "1 / 3", "2 / span 3", "auto"] {
            let p = parse_placement(text).expect(text);
            assert_eq!(placement_text_line(&p), text);
        }
        assert!(parse_placement("0").is_err());
        assert!(parse_placement("span 0").is_err());
        assert!(parse_placement("1 / 2 / 3").is_err());
    }
}
