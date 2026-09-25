//! M97 Phase 2 Step 3: the engine-md3 handover, as data. Everything MD3
//! that leaves `tre` in 0.3.5 -- colour science reference output, the
//! type/shape/elevation scales, the icons, the named motion curves, the
//! loading indicator's shapes -- generated from the code itself into
//! `docs/design/md3-handover.json`, so the framework rebuilding it has
//! exact values to check against rather than a hand-copied list.
//!
//! The file is golden: this test fails when it's stale. Regenerate with
//! `UPDATE_HANDOVER=1 cargo test -p engine-md3 --test handover`, then
//! `python tools/md3_handover.py` for the rest of the file and its page.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use engine_core::{Animated, LoadingIndicatorState, MotionCurve};
use engine_md3::{DynamicTheme, elevation_named, icons, named, type_style_named};
use material_colors::color::Argb;
use material_colors::theme::ThemeBuilder;
use peniko::Color;
use peniko::kurbo::{BezPath, Ellipse, PathEl, Point, RoundedRect, Shape};
use serde_json::{Map, Value, json};

/// The reference seeds: MD3's baseline purple, then a red, a teal, and
/// an amber, so a port is checked across hues, not one colour.
const SEEDS: [(u8, u8, u8); 4] = [
    (0x67, 0x50, 0xA4),
    (0xB3, 0x26, 0x1E),
    (0x00, 0x6A, 0x6A),
    (0xFF, 0xB3, 0x00),
];

/// Every role `ColorScheme::role` answers, in its declaration order.
#[rustfmt::skip]
const ROLES: [&str; 49] = [
    "primary", "on_primary", "primary_container", "on_primary_container",
    "inverse_primary", "primary_fixed", "primary_fixed_dim", "on_primary_fixed",
    "on_primary_fixed_variant", "secondary", "on_secondary", "secondary_container",
    "on_secondary_container", "secondary_fixed", "secondary_fixed_dim",
    "on_secondary_fixed", "on_secondary_fixed_variant", "tertiary", "on_tertiary",
    "tertiary_container", "on_tertiary_container", "tertiary_fixed",
    "tertiary_fixed_dim", "on_tertiary_fixed", "on_tertiary_fixed_variant", "error",
    "on_error", "error_container", "on_error_container", "surface_dim", "surface",
    "surface_tint", "surface_bright", "surface_container_lowest",
    "surface_container_low", "surface_container", "surface_container_high",
    "surface_container_highest", "on_surface", "on_surface_variant", "outline",
    "outline_variant", "inverse_surface", "inverse_on_surface", "surface_variant",
    "background", "on_background", "shadow", "scrim",
];

const TONES: [i32; 27] = [
    0, 4, 5, 6, 10, 12, 17, 20, 22, 24, 25, 30, 35, 40, 50, 60, 70, 80, 87, 90, 92, 94, 95, 96, 98,
    99, 100,
];

#[rustfmt::skip]
const TYPE_ROLES: [&str; 15] = [
    "display_large", "display_medium", "display_small", "headline_large",
    "headline_medium", "headline_small", "title_large", "title_medium", "title_small",
    "body_large", "body_medium", "body_small", "label_large", "label_medium", "label_small",
];

/// The named curves and the cubic-bezier control points each one is --
/// `the_named_curves_are_their_control_points` proves every entry by
/// sampling the engine's curve against the plain bezier.
#[rustfmt::skip]
const CURVES: [(&str, MotionCurve, [f64; 4]); 5] = [
    ("standard", MotionCurve::Standard, [0.2, 0.0, 0.0, 1.0]),
    ("standard_decelerate", MotionCurve::StandardDecelerate, [0.0, 0.0, 0.0, 1.0]),
    ("standard_accelerate", MotionCurve::StandardAccelerate, [0.3, 0.0, 1.0, 1.0]),
    ("emphasized_decelerate", MotionCurve::EmphasizedDecelerate, [0.05, 0.7, 0.1, 1.0]),
    ("emphasized_accelerate", MotionCurve::EmphasizedAccelerate, [0.3, 0.0, 0.8, 0.15]),
];

/// `emphasized` isn't one bezier: two segments, joined at `(1/6, 0.4)`,
/// each given as its four absolute points.
const EMPHASIZED: [[(f64, f64); 4]; 2] = [
    [(0.0, 0.0), (0.05, 0.0), (0.133333, 0.06), (0.166666, 0.4)],
    [(0.166666, 0.4), (0.208333, 0.82), (0.25, 1.0), (1.0, 1.0)],
];

const LOADING_SHAPES: [&str; 4] = ["pentagon", "pill", "cookie", "oval"];
const LOADING_SIZE: f64 = 48.0;

fn hex(color: Color) -> String {
    let c = color.to_rgba8();
    if c.a == 0xFF {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
    }
}

fn argb_hex(argb: Argb) -> String {
    format!("#{:02X}{:02X}{:02X}", argb.red, argb.green, argb.blue)
}

fn scheme_json(scheme: &engine_md3::ColorScheme) -> Value {
    ROLES
        .iter()
        .map(|role| (role.to_string(), json!(hex(scheme.role(role).unwrap()))))
        .collect::<Map<_, _>>()
        .into()
}

fn color_section() -> Value {
    let mut seeds = Map::new();
    for (r, g, b) in SEEDS {
        let seed = Color::from_rgba8(r, g, b, 0xFF);
        let theme = DynamicTheme::from_seed(seed);
        // `DynamicTheme` keeps only the two schemes; its palettes come
        // from the same builder call it makes.
        let palettes = ThemeBuilder::with_source(Argb::new(0xFF, r, g, b))
            .build()
            .palettes;
        let named = [
            ("primary", palettes.primary),
            ("secondary", palettes.secondary),
            ("tertiary", palettes.tertiary),
            ("neutral", palettes.neutral),
            ("neutral_variant", palettes.neutral_variant),
            ("error", palettes.error),
        ];
        let palettes: Map<_, _> = named
            .iter()
            .map(|(name, palette)| {
                let tones: Map<_, _> = TONES
                    .iter()
                    .map(|&t| (t.to_string(), json!(argb_hex(palette.tone(t)))))
                    .collect();
                (name.to_string(), Value::from(tones))
            })
            .collect();
        seeds.insert(
            hex(seed),
            json!({
                "light": scheme_json(&theme.light),
                "dark": scheme_json(&theme.dark),
                "palettes": palettes,
            }),
        );
    }
    json!({
        "library": "material-colors 0.4.2 (Rust port of material-color-utilities)",
        "variant": "tonal_spot",
        "contrast": 0.0,
        "roles": ROLES.as_slice(),
        "tones": TONES,
        "seeds": seeds,
    })
}

fn typography_section() -> Value {
    TYPE_ROLES
        .iter()
        .map(|role| {
            let style = type_style_named(role).unwrap();
            // `f32` fields, taken to `f64` at their written precision so
            // the JSON reads back exactly.
            let exact = |v: f32| (f64::from(v) * 1000.0).round() / 1000.0;
            let entry = json!({
                "font_family": style.font_family,
                "font_weight": exact(style.font_weight),
                "font_size": exact(style.font_size),
                "line_height": exact(style.line_height),
            });
            (role.to_string(), entry)
        })
        .collect::<Map<_, _>>()
        .into()
}

fn scale(names: &[&str], lookup: fn(&str) -> Option<f64>) -> Value {
    names
        .iter()
        .map(|name| (name.to_string(), json!(lookup(name).unwrap())))
        .collect::<Map<_, _>>()
        .into()
}

/// `curve`'s value at time `t` of a unit animation, via the engine's own
/// `Animated` -- the one public way to evaluate a named curve.
fn at(curve: MotionCurve, t: f64) -> f64 {
    let start = Instant::now();
    let mut value = Animated::new(0.0);
    value.animate_to(1.0, Duration::from_secs(1000), curve, start);
    value.tick(start + Duration::from_secs_f64(t * 1000.0), &mut Vec::new());
    value.current
}

fn sample(curve: MotionCurve) -> Vec<f64> {
    (0..=10)
        .map(|i| (at(curve, f64::from(i) / 10.0) * 10_000.0).round() / 10_000.0)
        .collect()
}

fn motion_section() -> Value {
    let mut curves: Map<_, _> = CURVES
        .iter()
        .map(|(name, curve, points)| {
            let entry = json!({"cubic_bezier": points, "samples": sample(*curve)});
            (name.to_string(), entry)
        })
        .collect();
    curves.insert(
        "emphasized".into(),
        json!({"segments": EMPHASIZED, "samples": sample(MotionCurve::Emphasized)}),
    );
    let times: Vec<f64> = (0..=10).map(|i| f64::from(i) / 10.0).collect();
    json!({"sample_times": times, "curves": curves})
}

fn corners(path: &BezPath) -> Vec<Point> {
    path.elements()
        .iter()
        .filter_map(|el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
            PathEl::ClosePath => None,
            other => panic!("a loading shape is a polygon, not {other:?}"),
        })
        .collect()
}

fn svg(path: &BezPath) -> String {
    // Two decimals, and never `-0.00`.
    let n = |v: f64| format!("{:.2}", v).replace("-0.00", "0.00");
    let point = |p: &Point| format!("{},{}", n(p.x), n(p.y));
    path.elements()
        .iter()
        .map(|el| match el {
            PathEl::MoveTo(p) => format!("M{}", point(p)),
            PathEl::LineTo(p) => format!("L{}", point(p)),
            PathEl::QuadTo(a, p) => format!("Q{} {}", point(a), point(p)),
            PathEl::CurveTo(a, b, p) => format!("C{} {} {}", point(a), point(b), point(p)),
            PathEl::ClosePath => "Z".to_string(),
        })
        .collect()
}

/// The pill and the oval as `shape_morph::loading_indicator_shapes`
/// defines them, curves and all. A morph shape keeps only each path
/// element's end point, so the engine draws these two as the polygons
/// through their curves' ends -- diamonds -- which `shapes` records.
fn intended_shapes() -> Map<String, Value> {
    let (w, h) = (LOADING_SIZE, LOADING_SIZE);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = cx.min(cy);
    let pill = RoundedRect::new(0.0, 0.0, w, h, h / 2.0).to_path(0.1);
    let oval = Ellipse::new((cx, cy), (r, r * 0.5), 0.0).to_path(0.1);
    let mut out = Map::new();
    out.insert("pill".into(), json!(svg(&pill)));
    out.insert("oval".into(), json!(svg(&oval)));
    out
}

fn loading_section() -> Value {
    let state = LoadingIndicatorState::new(LOADING_SIZE, LOADING_SIZE);
    let shapes: Map<_, _> = LOADING_SHAPES
        .iter()
        .zip(&state.shapes)
        .map(|(name, shape)| (name.to_string(), json!(svg(&shape.to_path()))))
        .collect();
    json!({
        "view_box": [0.0, 0.0, LOADING_SIZE, LOADING_SIZE],
        "order": LOADING_SHAPES,
        "step_ms": 650,
        "easing": "linear",
        "shapes": shapes,
        "intended": intended_shapes(),
    })
}

fn handover() -> Map<String, Value> {
    let icons: Map<_, _> = icons::names()
        .map(|name| (name.to_string(), json!(icons::path_for(name).unwrap())))
        .collect();
    let shapes = [
        "none",
        "extra_small",
        "small",
        "medium",
        "large",
        "extra_large",
    ];
    let levels = [
        "level_0", "level_1", "level_2", "level_3", "level_4", "level_5",
    ];
    let mut out = Map::new();
    out.insert("color".into(), color_section());
    out.insert("typography".into(), typography_section());
    out.insert("shape".into(), scale(&shapes, named));
    out.insert("elevation".into(), scale(&levels, elevation_named));
    out.insert(
        "icons".into(),
        json!({"view_box": [0.0, -960.0, 960.0, 960.0], "paths": icons}),
    );
    out.insert("motion".into(), motion_section());
    out.insert("loading_indicator".into(), loading_section());
    out
}

fn handover_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/design/md3-handover.json")
}

#[test]
fn the_committed_handover_matches_the_engine() {
    let path = handover_path();
    let mut file: Map<String, Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let generated = handover();
    if std::env::var_os("UPDATE_HANDOVER").is_some() {
        file.extend(generated);
        let text = serde_json::to_string_pretty(&file).unwrap() + "\n";
        std::fs::write(&path, text).unwrap();
        return;
    }
    for (key, value) in generated {
        assert!(
            file.get(&key) == Some(&value),
            "docs/design/md3-handover.json's {key:?} is stale -- run \
             `UPDATE_HANDOVER=1 cargo test -p engine-md3 --test handover`"
        );
    }
}

/// Proves `CURVES` and `EMPHASIZED`: each named curve samples the same
/// as the plain bezier its entry states.
#[test]
fn the_named_curves_are_their_control_points() {
    for (name, curve, [x1, y1, x2, y2]) in CURVES {
        assert_eq!(
            sample(curve),
            sample(MotionCurve::Bezier(x1, y1, x2, y2)),
            "{name}"
        );
    }
    // Each emphasized segment, rescaled to a unit bezier, matches within
    // its own span.
    for [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] in EMPHASIZED {
        let (w, h) = (x3 - x0, y3 - y0);
        let unit = MotionCurve::Bezier((x1 - x0) / w, (y1 - y0) / h, (x2 - x0) / w, (y2 - y0) / h);
        for i in 1..10 {
            let t = x0 + w * f64::from(i) / 10.0;
            let expected = y0 + h * at(unit, (t - x0) / w);
            assert!(
                (at(MotionCurve::Emphasized, t) - expected).abs() < 1e-3,
                "t={t}"
            );
        }
    }
}

/// The loading shapes are published at 48 x 48; they scale linearly,
/// so one size serves every size.
#[test]
fn the_loading_shapes_scale_linearly() {
    let small = LoadingIndicatorState::new(LOADING_SIZE, LOADING_SIZE);
    let large = LoadingIndicatorState::new(LOADING_SIZE * 2.0, LOADING_SIZE * 2.0);
    for (a, b) in small.shapes.iter().zip(&large.shapes) {
        let (a, b) = (corners(&a.to_path()), corners(&b.to_path()));
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert!((a.x * 2.0 - b.x).abs() < 1e-6 && (a.y * 2.0 - b.y).abs() < 1e-6);
        }
    }
}
