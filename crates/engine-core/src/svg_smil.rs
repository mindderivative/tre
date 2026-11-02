//! 0.5.10 (#245): SMIL animation in an SVG document, sampled at a time.
//!
//! `usvg` ignores `<animate>`, `<set>` and `<animateTransform>`. So the
//! document is read here once into a `Timeline`, and for a time `t` the
//! timeline writes the values the animations have at `t` into a copy of the
//! document's text (replacing an attribute's value, or adding the attribute),
//! which `usvg` then parses as the static document it is.
//!
//! The subset is what animated icon sets use: the three elements above; the
//! attributes `attributeName`, `dur`, `begin` (a clock offset: `1.5s`, `200ms`,
//! or bare seconds), `to`, `values`, `keyTimes`, `repeatCount`, `fill`
//! (`freeze` or `remove`) and `type` (`rotate`, `translate`, `scale`); and the
//! animated attributes `stroke-dashoffset`, `opacity`, `fill-opacity`, `d`,
//! `r`, `cx`, `cy`, `x`, `y`, `stroke-width` and `transform`. An animation
//! with only `to` starts from the attribute the parent element has (or the
//! property's initial value). Several animations of one attribute: the later
//! one in the document wins. A `d` whose values do not share one command
//! sequence holds its first value. Anything else (`animateMotion`, sync-base
//! `begin`, `calcMode`, `additive`, ...) is ignored, never an error.

use roxmltree::Document;
use std::collections::HashMap;
use std::ops::Range;

/// The animations of one document.
#[derive(Debug)]
pub struct Timeline {
    /// The document as text (inflated when it was an SVGZ).
    text: String,
    anims: Vec<Anim>,
    duration: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Cmd(char),
    Num(f64),
    /// An arc's large-arc or sweep flag: one digit, never interpolated.
    Flag(bool),
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Num(f64),
    Path(Vec<Tok>),
    List(Vec<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Number,
    Path,
    Rotate,
    Translate,
    Scale,
}

#[derive(Debug)]
struct Anim {
    /// Where the target element's tag name ends: an attribute it does not
    /// have yet is inserted there.
    insert_at: usize,
    /// The existing attribute's value, inside its quotes.
    existing: Option<Range<usize>>,
    name: &'static str,
    kind: Kind,
    begin: f64,
    /// `None`: active from `begin` on, for good (a `<set>` without `dur`).
    dur: Option<f64>,
    repeat: f64,
    freeze: bool,
    values: Vec<Value>,
    key_times: Vec<f64>,
}

/// Animatable attributes, and what they are when the element has none.
const NUMBERS: [(&str, f64); 9] = [
    ("stroke-dashoffset", 0.0),
    ("opacity", 1.0),
    ("fill-opacity", 1.0),
    ("r", 0.0),
    ("cx", 0.0),
    ("cy", 0.0),
    ("x", 0.0),
    ("y", 0.0),
    ("stroke-width", 1.0),
];

impl Timeline {
    /// The timeline of `data` (an SVG or SVGZ document), or `None` when it
    /// has no animation this module plays.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let inflated;
        let data = if data.starts_with(&[0x1f, 0x8b]) {
            inflated = crate::svg::inflate_capped(data).ok()?;
            &inflated[..]
        } else {
            data
        };
        let text = std::str::from_utf8(data).ok()?;
        if !text.contains("animate") && !text.contains("set") {
            return None;
        }
        // usvg reads a document with a DOCTYPE (Illustrator, Inkscape), so this does too.
        let options = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };
        let doc = Document::parse_with_options(text, options).ok()?;
        let mut anims = Vec::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            let tag = node.tag_name().name();
            if !matches!(tag, "animate" | "set" | "animateTransform") {
                continue;
            }
            let Some(parent) = node.parent_element() else {
                continue;
            };
            if let Some(anim) = Anim::read(text, &node, &parent, tag) {
                anims.push(anim);
            }
        }
        if anims.is_empty() {
            return None;
        }
        let mut duration = Some(0.0f64);
        for anim in &anims {
            let end = match (anim.dur, anim.repeat.is_infinite()) {
                (None, _) => Some(anim.begin),
                (Some(_), true) => None,
                (Some(d), false) => Some(anim.begin + d * anim.repeat),
            };
            duration = match (duration, end.filter(|e| e.is_finite())) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
        }
        Some(Self {
            text: text.to_string(),
            anims,
            duration,
        })
    }

    /// The seconds after which nothing changes, or `None` when an animation
    /// repeats for ever.
    pub fn duration(&self) -> Option<f64> {
        self.duration
    }

    /// The document as text, before any animation.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The document as it looks at `t` seconds, or `None` when that is the
    /// document unchanged.
    pub fn apply(&self, t: f64) -> Option<String> {
        // (insert_at, attribute) -> (existing range, text): the later wins.
        let mut written: HashMap<(usize, &'static str), (&Anim, String)> = HashMap::new();
        for anim in &self.anims {
            if let Some(value) = anim.at(t) {
                written.insert((anim.insert_at, anim.name), (anim, anim.format(&value)));
            }
        }
        if written.is_empty() {
            return None;
        }
        // Edits from the back, so earlier offsets stay valid. An insertion's
        // position is its element's; the attribute name orders those that share
        // one.
        let mut edits: Vec<(usize, usize, String, &'static str)> = written
            .into_iter()
            .map(|((at, name), (anim, text))| match &anim.existing {
                Some(range) => (range.start, range.end, text, name),
                None => (at, at, format!(" {name}=\"{text}\""), name),
            })
            .collect();
        edits.sort_by(|a, b| (b.0, b.3).cmp(&(a.0, a.3)));
        let mut out = self.text.clone();
        for (start, end, text, _) in edits {
            out.replace_range(start..end, &text);
        }
        Some(out)
    }
}

impl Anim {
    fn read(
        text: &str,
        node: &roxmltree::Node<'_, '_>,
        parent: &roxmltree::Node<'_, '_>,
        tag: &str,
    ) -> Option<Self> {
        let attr = |n: &str| node.attribute(n);
        let name = attr("attributeName")?;
        let is_set = tag == "set";
        let kind = if tag == "animateTransform" {
            if name != "transform" {
                return None;
            }
            match attr("type").unwrap_or("translate") {
                "rotate" => Kind::Rotate,
                "translate" => Kind::Translate,
                "scale" => Kind::Scale,
                _ => return None,
            }
        } else if name == "d" {
            Kind::Path
        } else if NUMBERS.iter().any(|(n, _)| *n == name) {
            Kind::Number
        } else {
            return None;
        };
        let (name, initial): (&'static str, f64) = match kind {
            Kind::Rotate | Kind::Translate | Kind::Scale => ("transform", 0.0),
            Kind::Path => ("d", 0.0),
            Kind::Number => *NUMBERS.iter().find(|(n, _)| *n == name)?,
        };
        // A negative `begin` starts the animation part-way through, which is how icon sets
        // phase their loops.
        let begin = match attr("begin") {
            Some(b) => clock(b.trim().strip_prefix('-').unwrap_or(b))
                .map(|n| if b.trim().starts_with('-') { -n } else { n })?,
            None => 0.0,
        };
        let dur = match attr("dur") {
            Some(d) => Some(clock(d).filter(|d| *d > 0.0)?),
            None if is_set => None,
            None => return None,
        };
        let repeat = match attr("repeatCount") {
            None => 1.0,
            Some("indefinite") => f64::INFINITY,
            Some(n) => n.trim().parse::<f64>().ok().filter(|n| *n > 0.0)?,
        };
        let freeze = attr("fill") == Some("freeze");
        let base_attr = parent.attributes().find(|a| a.name() == name);
        let base = base_attr.as_ref().map(|a| a.value());
        let existing = base_attr.as_ref().map(|a| a.range_value());
        let mut insert_at = parent.range().start + 1;
        insert_at += text[insert_at..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(0);

        let parse = |s: &str| parse_value(kind, s);
        let mut values: Vec<Value> = if is_set {
            vec![parse(attr("to")?)?]
        } else if let Some(list) = attr("values") {
            // SMIL allows one `;` after the last value.
            list.trim()
                .trim_end_matches(';')
                .split(';')
                .map(|v| parse(v.trim()))
                .collect::<Option<Vec<_>>>()?
        } else {
            let to = parse(attr("to")?)?;
            // A transform animation replaces the attribute, so it starts from
            // the identity, not from the parent's own transform list.
            let from = match (kind, base) {
                (_, _) if attr("from").is_some() => parse(attr("from")?)?,
                (Kind::Rotate | Kind::Translate | Kind::Scale, _) => identity(kind, &to),
                (_, Some(b)) => parse(b)?,
                (Kind::Number, None) => Value::Num(initial),
                _ => return None,
            };
            vec![from, to]
        };
        if values.is_empty() {
            return None;
        }
        normalise(kind, &mut values)?;
        let mut key_times = Vec::new();
        if let Some(kt) = attr("keyTimes") {
            let parsed: Option<Vec<f64>> = kt
                .trim()
                .trim_end_matches(';')
                .split(';')
                .map(|v| v.trim().parse().ok())
                .collect();
            if let Some(parsed) = parsed
                && parsed.len() == values.len()
                && parsed.first() == Some(&0.0)
                && parsed.last() == Some(&1.0)
                && parsed.windows(2).all(|w| w[0] <= w[1])
            {
                key_times = parsed;
            }
        }
        Some(Self {
            insert_at,
            existing,
            name,
            kind,
            begin,
            dur,
            repeat,
            freeze,
            values,
            key_times,
        })
    }

    /// The value at `t`, or `None` when the animation does not apply then.
    fn at(&self, t: f64) -> Option<Value> {
        let local = t - self.begin;
        if local < 0.0 {
            return None;
        }
        let progress = match self.dur {
            None => 1.0,
            Some(d) => {
                let active = d * self.repeat;
                if local >= active {
                    if !self.freeze {
                        return None;
                    }
                    // The repeat count itself, not `active / d`, which is off by a rounding
                    // error for a duration like 0.1 s (3 x 0.1 / 0.1 is 3.0000000000000004).
                    let fraction = self.repeat.fract();
                    if fraction == 0.0 { 1.0 } else { fraction }
                } else {
                    (local % d) / d
                }
            }
        };
        Some(self.sample(progress))
    }

    fn sample(&self, p: f64) -> Value {
        let n = self.values.len();
        if n == 1 {
            return self.values[0].clone();
        }
        let time = |i: usize| -> f64 {
            if self.key_times.is_empty() {
                i as f64 / (n - 1) as f64
            } else {
                self.key_times[i]
            }
        };
        if p >= 1.0 {
            return self.values[n - 1].clone();
        }
        let i = (0..n - 1).rev().find(|&i| time(i) <= p).unwrap_or(0);
        let (t0, t1) = (time(i), time(i + 1));
        let f = if t1 > t0 { (p - t0) / (t1 - t0) } else { 1.0 };
        lerp(&self.values[i], &self.values[i + 1], f)
    }

    fn format(&self, value: &Value) -> String {
        match value {
            Value::Num(n) => n.to_string(),
            Value::Path(tokens) => {
                let mut out = String::new();
                for tok in tokens {
                    match tok {
                        Tok::Flag(f) => {
                            out.push(' ');
                            out.push(if *f { '1' } else { '0' });
                        }
                        Tok::Cmd(c) => {
                            if !out.is_empty() {
                                out.push(' ');
                            }
                            out.push(*c);
                        }
                        Tok::Num(n) => {
                            out.push(' ');
                            out.push_str(&n.to_string());
                        }
                    }
                }
                out
            }
            Value::List(list) => {
                let args: Vec<String> = list.iter().map(f64::to_string).collect();
                let word = match self.kind {
                    Kind::Rotate => "rotate",
                    Kind::Translate => "translate",
                    _ => "scale",
                };
                format!("{word}({})", args.join(" "))
            }
        }
    }
}

fn lerp(a: &Value, b: &Value, f: f64) -> Value {
    let mix = |x: f64, y: f64| x + (y - x) * f;
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(mix(*x, *y)),
        (Value::List(x), Value::List(y)) if x.len() == y.len() => {
            Value::List(x.iter().zip(y).map(|(x, y)| mix(*x, *y)).collect())
        }
        (Value::Path(x), Value::Path(y)) if x.len() == y.len() => Value::Path(
            x.iter()
                .zip(y)
                .map(|(x, y)| match (x, y) {
                    (Tok::Num(x), Tok::Num(y)) => Tok::Num(mix(*x, *y)),
                    _ => x.clone(),
                })
                .collect(),
        ),
        _ => a.clone(),
    }
}

/// A clock value: `1.5s`, `200ms`, or bare seconds. Not a sync-base, a list
/// or `indefinite`.
fn clock(text: &str) -> Option<f64> {
    let t = text.trim();
    let (number, scale) = if let Some(n) = t.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = t.strip_suffix('s') {
        (n, 1.0)
    } else {
        (t, 1.0)
    };
    let n: f64 = number.trim().parse().ok()?;
    (n.is_finite() && n >= 0.0).then_some(n * scale)
}

fn parse_value(kind: Kind, text: &str) -> Option<Value> {
    match kind {
        Kind::Number => {
            let t = text.trim();
            let t = t.strip_suffix("px").unwrap_or(t);
            let n: f64 = t.trim().parse().ok()?;
            n.is_finite().then_some(Value::Num(n))
        }
        Kind::Path => tokens(text).map(Value::Path),
        _ => {
            let list: Option<Vec<f64>> = text
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().ok())
                .collect();
            let list = list?;
            (!list.is_empty()).then_some(Value::List(list))
        }
    }
}

/// What a `to`-only transform animation starts from.
fn identity(kind: Kind, to: &Value) -> Value {
    let Value::List(to) = to else {
        return Value::List(vec![0.0]);
    };
    match kind {
        Kind::Rotate => {
            let mut list = vec![0.0];
            list.extend(to.iter().skip(1));
            Value::List(list)
        }
        Kind::Translate => Value::List(vec![0.0, 0.0]),
        _ => Value::List(vec![1.0, 1.0]),
    }
}

/// Brings the values of one animation to a common shape, or `None` when they
/// cannot have one. A path whose values do not share a command sequence is
/// cut down to its first value, which it holds.
fn normalise(kind: Kind, values: &mut Vec<Value>) -> Option<()> {
    match kind {
        Kind::Number => {}
        Kind::Path => {
            let same = |a: &Value, b: &Value| match (a, b) {
                (Value::Path(a), Value::Path(b)) => {
                    a.len() == b.len()
                        && a.iter().zip(b).all(|(x, y)| match (x, y) {
                            (Tok::Cmd(x), Tok::Cmd(y)) => x == y,
                            (Tok::Num(_), Tok::Num(_)) | (Tok::Flag(_), Tok::Flag(_)) => true,
                            _ => false,
                        })
                }
                _ => false,
            };
            if !values.iter().all(|v| same(&values[0], v)) {
                values.truncate(1);
            }
        }
        _ => {
            let width = match kind {
                Kind::Rotate => values
                    .iter()
                    .map(|v| if let Value::List(l) = v { l.len() } else { 0 })
                    .max()?,
                _ => 2,
            };
            let tail: Vec<f64> = match kind {
                Kind::Rotate => values
                    .iter()
                    .find_map(|v| match v {
                        Value::List(l) if l.len() == width => Some(l[1..].to_vec()),
                        _ => None,
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for value in values.iter_mut() {
                let Value::List(list) = value else {
                    return None;
                };
                match kind {
                    Kind::Rotate => {
                        if list.len() == 1 {
                            list.extend(&tail);
                        }
                        if list.len() != width {
                            return None;
                        }
                    }
                    Kind::Translate => {
                        if list.len() == 1 {
                            list.push(0.0);
                        }
                        list.truncate(2);
                    }
                    _ => {
                        if list.len() == 1 {
                            list.push(list[0]);
                        }
                        list.truncate(2);
                    }
                }
            }
        }
    }
    Some(())
}

fn tokens(d: &str) -> Option<Vec<Tok>> {
    let b = d.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    // The command in force, and which of its arguments is next: an arc's fourth and fifth
    // are flags, written as one digit each (`a1 1 0 011 1` is flags 0 and 1, then `1 1`).
    let mut arc = false;
    let mut arg = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() || c == b',' {
            i += 1;
        } else if c.is_ascii_alphabetic() {
            out.push(Tok::Cmd(c as char));
            arc = c == b'a' || c == b'A';
            arg = 0;
            i += 1;
        } else if arc && matches!(arg % 7, 3 | 4) {
            match c {
                b'0' | b'1' => out.push(Tok::Flag(c == b'1')),
                _ => return None,
            }
            arg += 1;
            i += 1;
        } else {
            let start = i;
            if b[i] == b'+' || b[i] == b'-' {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i < b.len() && b[i] == b'.' {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let n: f64 = d[start..i].parse().ok()?;
            if !n.is_finite() {
                return None;
            }
            out.push(Tok::Num(n));
            arg += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{body}</svg>"#)
    }

    fn at(timeline: &Timeline, t: f64) -> String {
        timeline
            .apply(t)
            .unwrap_or_else(|| timeline.text().to_string())
    }

    #[test]
    fn a_document_without_animation_has_no_timeline() {
        assert!(Timeline::parse(svg(r#"<path d="M0 0L1 1"/>"#).as_bytes()).is_none());
        // `animateMotion` and sync-base `begin` are not played.
        let motion = svg(r#"<path d="M0 0"><animateMotion dur="1s" path="M0 0L5 5"/></path>"#);
        assert!(Timeline::parse(motion.as_bytes()).is_none());
        let sync = svg(
            r#"<path opacity="1"><animate attributeName="opacity" to="0" dur="1s" begin="a.end"/></path>"#,
        );
        assert!(Timeline::parse(sync.as_bytes()).is_none());
    }

    #[test]
    fn a_to_animation_starts_from_the_parent_attribute() {
        let doc = svg(
            r#"<path d="M0 0" stroke-dashoffset="28"><animate attributeName="stroke-dashoffset" to="0" dur="2s" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert_eq!(tl.duration(), Some(2.0));
        assert!(at(&tl, 0.0).contains(r#"stroke-dashoffset="28""#));
        assert!(at(&tl, 1.0).contains(r#"stroke-dashoffset="14""#));
        assert!(at(&tl, 5.0).contains(r#"stroke-dashoffset="0""#));
    }

    #[test]
    fn remove_reverts_after_the_end_and_freeze_holds() {
        let make = |fill: &str| {
            svg(&format!(
                r#"<path d="M0 0" opacity="1"><animate attributeName="opacity" to="0" dur="1s" {fill}/></path>"#
            ))
        };
        let removed = Timeline::parse(make("").as_bytes()).unwrap();
        assert!(at(&removed, 3.0).contains(r#"opacity="1""#));
        let frozen = Timeline::parse(make(r#"fill="freeze""#).as_bytes()).unwrap();
        assert!(at(&frozen, 3.0).contains(r#"opacity="0""#));
    }

    #[test]
    fn a_set_applies_from_its_begin_and_adds_a_missing_attribute() {
        let doc =
            svg(r#"<path d="M0 0"><set attributeName="opacity" to="0.5" begin="0.7s"/></path>"#);
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert_eq!(tl.duration(), Some(0.7));
        assert!(tl.apply(0.5).is_none());
        let after = tl.apply(0.8).unwrap();
        assert!(
            after.contains(r#"<path opacity="0.5" d="M0 0">"#),
            "{after}"
        );
        assert!(Document::parse(&after).is_ok());
    }

    #[test]
    fn values_and_key_times_pick_the_segment() {
        let doc = svg(
            r#"<circle r="1"><animate attributeName="r" values="0;10;4" keyTimes="0;0.25;1" dur="4s" repeatCount="indefinite"/></circle>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert_eq!(tl.duration(), None);
        assert!(at(&tl, 0.5).contains(r#"r="5""#));
        assert!(at(&tl, 1.0).contains(r#"r="10""#));
        // Four seconds later it is the same again.
        assert_eq!(at(&tl, 0.5), at(&tl, 4.5));
        assert_ne!(at(&tl, 0.5), at(&tl, 2.5));
    }

    #[test]
    fn a_path_morphs_only_between_equal_command_sequences() {
        let same = svg(
            r#"<path d="M0 0 L10 10"><animate attributeName="d" values="M0 0L10 10;M0 0L20 30" dur="2s" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(same.as_bytes()).unwrap();
        assert!(
            at(&tl, 1.0).contains(r#"d="M 0 0 L 15 20""#),
            "{}",
            at(&tl, 1.0)
        );
        let different = svg(
            r#"<path d="M0 0 L10 10"><animate attributeName="d" values="M0 0L10 10;M0 0H20V30" dur="2s" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(different.as_bytes()).unwrap();
        assert!(at(&tl, 1.0).contains(r#"d="M 0 0 L 10 10""#));
        assert!(at(&tl, 5.0).contains(r#"d="M 0 0 L 10 10""#));
    }

    #[test]
    fn a_rotation_replaces_the_transform() {
        let doc = svg(
            r#"<g transform="translate(1 1)"><animateTransform attributeName="transform" type="rotate" to="360 12 12" dur="2s" repeatCount="indefinite"/></g>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert!(
            at(&tl, 1.0).contains(r#"transform="rotate(180 12 12)""#),
            "{}",
            at(&tl, 1.0)
        );
    }

    #[test]
    fn from_a_negative_begin_a_trailing_semicolon_and_a_doctype_are_read() {
        let doc = r#"<?xml version="1.0"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0" stroke-dashoffset="9"><animate attributeName="stroke-dashoffset" from="30" to="0" begin="-1s" dur="2s" fill="freeze"/><animate attributeName="opacity" values="0;1;" dur="2s" fill="freeze"/></path></svg>"#;
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        // One second into a two second animation that started a second ago: from 30 to 0.
        assert!(
            at(&tl, 0.0).contains(r#"stroke-dashoffset="15""#),
            "{}",
            at(&tl, 0.0)
        );
        assert!(at(&tl, 1.0).contains(r#"opacity="0.5""#));
    }

    #[test]
    fn a_frozen_animation_holds_its_last_value_whatever_the_float_error() {
        // 0.1 * 3 / 0.1 is 3.0000000000000004.
        let doc = svg(
            r#"<path d="M0 0" opacity="1"><animate attributeName="opacity" to="0" dur="0.1s" repeatCount="3" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert!(at(&tl, 5.0).contains(r#"opacity="0""#), "{}", at(&tl, 5.0));
    }

    #[test]
    fn arc_flags_are_read_by_position_and_never_interpolated() {
        let compact = tokens("M0 0a1 1 0 011 1").unwrap();
        assert_eq!(
            compact,
            vec![
                Tok::Cmd('M'),
                Tok::Num(0.0),
                Tok::Num(0.0),
                Tok::Cmd('a'),
                Tok::Num(1.0),
                Tok::Num(1.0),
                Tok::Num(0.0),
                Tok::Flag(false),
                Tok::Flag(true),
                Tok::Num(1.0),
                Tok::Num(1.0),
            ]
        );
        let doc = svg(
            r#"<path d="M0 0a1 1 0 011 1"><animate attributeName="d" values="M0 0a1 1 0 011 1;M0 0a1 1 0 101 3" dur="2s" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        // The flags of the segment's start stay, the numbers move.
        assert!(at(&tl, 1.0).contains("a 1 1 0 0 1 1 2"), "{}", at(&tl, 1.0));
    }

    #[test]
    fn later_animations_of_one_attribute_win_and_clocks_parse() {
        assert_eq!(clock("200ms"), Some(0.2));
        assert_eq!(clock("1.5s"), Some(1.5));
        assert_eq!(clock("2"), Some(2.0));
        assert_eq!(clock("a.end"), None);
        let doc = svg(
            r#"<path d="M0 0" opacity="1"><animate attributeName="opacity" to="0.2" dur="1s" fill="freeze"/><animate attributeName="opacity" to="0.8" dur="1s" fill="freeze"/></path>"#,
        );
        let tl = Timeline::parse(doc.as_bytes()).unwrap();
        assert!(at(&tl, 2.0).contains(r#"opacity="0.8""#));
    }
}
