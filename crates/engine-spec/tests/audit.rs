//! M97 Phase 2 Step 4: the binding and cascade audit
//! (`docs/design/binding-cascade-audit.md`). Each test pins one listed
//! behavior of `binding.rs` or `cascade.rs` -- mostly where it differs
//! from what its doc comments, or Python, would lead a reader to expect
//! -- so the audit stays true, and a framework porting either one can
//! see exactly what it's matching.

use std::collections::HashMap;

use engine_spec::{
    BinOp, BindingResolver, ExpressionError, ResolveError, Stylesheet, Value, evaluate,
    parse_binding, parse_stylesheet, parse_view, resolve_style_layered,
};

/// Identifiers from a map; nothing else resolves.
struct Vars(HashMap<&'static str, Value>);

impl BindingResolver for Vars {
    fn ident(&self, name: &str) -> Result<Value, ResolveError> {
        self.0
            .get(name)
            .cloned()
            .ok_or_else(|| ResolveError(format!("unknown {name}")))
    }
    fn attr(&self, _: &Value, name: &str) -> Result<Value, ResolveError> {
        Err(ResolveError(format!("no attr {name}")))
    }
    fn index(&self, _: &Value, _: &Value) -> Result<Value, ResolveError> {
        Err(ResolveError("no index".into()))
    }
    fn call(&self, _: &Value, method: &str) -> Result<Value, ResolveError> {
        Err(ResolveError(format!("no method {method}")))
    }
    fn binary_op(&self, _: BinOp, _: &Value, _: &Value) -> Result<Value, ResolveError> {
        Err(ResolveError("no handles".into()))
    }
    fn truthy(&self, _: &Value) -> Result<bool, ResolveError> {
        Err(ResolveError("no handles".into()))
    }
}

fn eval(raw: &str) -> Result<Value, String> {
    let expr = parse_binding(raw).map_err(|e| format!("parse: {e}"))?;
    let vars = Vars(HashMap::from([
        ("n", Value::Int(3)),
        ("x", Value::Float(0.5)),
    ]));
    evaluate(&expr, &vars).map_err(|e| format!("eval: {e}"))
}

fn fails(raw: &str, stage: &str) {
    let result = eval(raw);
    assert!(
        matches!(&result, Err(e) if e.starts_with(stage)),
        "{raw}: expected a {stage} error, got {result:?}"
    );
}

// --- binding.rs ------------------------------------------------------------

#[test]
fn b1_only_addition_mixes_int_and_float() {
    assert_eq!(eval("{{ n + x }}"), Ok(Value::Float(3.5)));
    for raw in [
        "{{ n - x }}",
        "{{ n * x }}",
        "{{ n / x }}",
        "{{ n < x }}",
        "{{ x >= n }}",
    ] {
        fails(raw, "eval");
    }
}

#[test]
fn b2_equality_is_by_variant_so_int_never_equals_float_or_bool() {
    assert_eq!(eval("{{ 1 == 1.0 }}"), Ok(Value::Bool(false)));
    assert_eq!(eval("{{ True == 1 }}"), Ok(Value::Bool(false)));
    assert_eq!(eval("{{ 1 != 1.0 }}"), Ok(Value::Bool(true)));
}

#[test]
fn b3_division_by_zero_errors_for_ints_and_is_infinite_for_floats() {
    fails("{{ 1 / 0 }}", "eval");
    assert_eq!(eval("{{ 1.0 / 0.0 }}"), Ok(Value::Float(f64::INFINITY)));
    assert_eq!(eval("{{ 3 / 2 }}"), Ok(Value::Float(1.5)), "true division");
}

#[test]
fn b4_integers_are_64_bit() {
    fails("{{ 9223372036854775808 }}", "parse");
    // Overflow isn't checked by the evaluator: a debug build panics, a
    // release build (the published wheels) wraps.
    let sum = std::panic::catch_unwind(|| eval("{{ 9223372036854775807 + 1 }}"));
    assert!(sum.is_err() || sum.unwrap() == Ok(Value::Int(i64::MIN)));
}

#[test]
fn b5_strings_only_concatenate_and_compare_for_equality() {
    assert_eq!(eval("{{ 'a' + 'b' }}"), Ok(Value::Str("ab".into())));
    assert_eq!(eval("{{ 'a' == 'a' }}"), Ok(Value::Bool(true)));
    for raw in ["{{ 'a' < 'b' }}", "{{ 'a' * 2 }}", "{{ 'n: ' + n }}"] {
        fails(raw, "eval");
    }
}

#[test]
fn b6_booleans_are_not_numbers() {
    fails("{{ True + 1 }}", "eval");
    fails("{{ True < 2 }}", "eval");
}

#[test]
fn b7_missing_syntax_is_a_parse_error() {
    for raw in [
        "{{ -1 }}",            // no unary minus: write 0 - 1
        "{{ 1 < n < 5 }}",     // comparisons don't chain
        "{{ n % 2 }}",         // no %, //, **
        "{{ n ** 2 }}",        //
        "{{ .5 }}",            // a float needs a leading digit
        "{{ 1.2.3 }}",         //
        "{{ n if x else 0 }}", // no conditional expression
        "{{ [1, 2] }}",        // no list, tuple, or dict literals
        "{{ f(1) }}",          // no calls with arguments, or bare calls
        "{{ 'it\\'s' }}",      // no escapes inside strings
    ] {
        fails(raw, "parse");
    }
}

#[test]
fn b8_none_is_an_identifier_and_and_or_return_an_operand() {
    // Only True and False are literals; None is looked up like a name.
    fails("{{ None }}", "eval");
    assert_eq!(eval("{{ 0 or x }}"), Ok(Value::Float(0.5)));
    assert_eq!(eval("{{ n and 'yes' }}"), Ok(Value::Str("yes".into())));
    assert_eq!(eval("{{ not n }}"), Ok(Value::Bool(false)));
}

#[test]
fn b9_a_binding_is_the_whole_value_not_an_interpolation() {
    for raw in ["Count: {{ n }}", "{{ n }} items"] {
        let result = parse_binding(raw);
        assert!(
            matches!(result, Err(ExpressionError::NotABinding(_))),
            "{raw}"
        );
    }
    // Two bindings in one string read as one expression, `n }} of {{ n`,
    // which doesn't parse.
    let result = parse_binding("{{ n }} of {{ n }}");
    assert!(matches!(result, Err(ExpressionError::Parse(_))));
}

// --- cascade.rs ------------------------------------------------------------

fn opacity(widget: &str, sheet: &str) -> Option<f32> {
    let widget = parse_view(widget).unwrap();
    let sheet: Stylesheet = parse_stylesheet(sheet).unwrap();
    resolve_style_layered(&widget, None, None, Some(&sheet)).opacity
}

#[test]
fn c1_a_rule_naming_kind_and_classes_applies_at_both_tiers_separately() {
    let sheet = "styles:\n  - {kind: Rect, classes: [dim], style: {opacity: 0.5}}\n";
    // Its kind tier ignores its classes: a Rect without `dim` matches.
    assert_eq!(opacity("id: a\nkind: Rect\n", sheet), Some(0.5));
    // Its class tier ignores its kind: a Container with `dim` matches.
    assert_eq!(
        opacity("id: b\nkind: Container\nclasses: [dim]\n", sheet),
        Some(0.5)
    );
}

#[test]
fn c2_a_rule_naming_an_id_and_a_kind_styles_every_widget_of_that_kind() {
    let sheet = "styles:\n  - {id: hero, kind: Rect, style: {opacity: 0.5}}\n";
    assert_eq!(opacity("id: other\nkind: Rect\n", sheet), Some(0.5));
}

#[test]
fn c3_within_a_tier_the_later_rule_wins() {
    let sheet = "styles:\n  - {kind: Rect, style: {opacity: 0.2}}\n  \
                 - {kind: Rect, style: {opacity: 0.4}}\n";
    assert_eq!(opacity("id: a\nkind: Rect\n", sheet), Some(0.4));
}

#[test]
fn c4_a_repeated_class_counts_toward_specificity() {
    // Both match `[a, b]`; `[a, a, a]` names one distinct class but
    // counts three, so it outranks `[a, b]` despite coming first.
    let sheet = "styles:\n  - {classes: [a, a, a], style: {opacity: 0.2}}\n  \
                 - {classes: [a, b], style: {opacity: 0.4}}\n";
    assert_eq!(
        opacity("id: w\nkind: Rect\nclasses: [a, b]\n", sheet),
        Some(0.2)
    );
}

#[test]
fn c5_each_layer_cascades_alone_so_a_theme_id_rule_loses_to_an_app_baseline() {
    let widget = parse_view("id: hero\nkind: Rect\n").unwrap();
    let theme = parse_stylesheet("styles:\n  - {id: hero, style: {opacity: 0.2}}\n").unwrap();
    let app = parse_stylesheet("styles:\n  - {style: {opacity: 0.9}}\n").unwrap();
    let style = resolve_style_layered(&widget, Some(&theme), None, Some(&app));
    assert_eq!(style.opacity, Some(0.9));
}
