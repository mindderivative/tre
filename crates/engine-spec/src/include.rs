//! §16.6's real `include:` composition -- splitting a `view.yaml`
//! across multiple files, spliced together before any of §16.1's own
//! `deny_unknown_fields` validation ever runs, exactly per its own
//! text: "expanded during loading, before validation... the included
//! file's `WidgetSpec` tree splices in at that point as ordinary
//! children, indistinguishable from inline ones once loaded."
//!
//! Deliberately operates on the raw `serde_yaml_ng::Value` tree, not
//! on `WidgetSpec` itself -- a bare `{include: path}` mapping can't
//! deserialize into `WidgetSpec` at all (missing its required `id`/
//! `kind`, an unrecognized key under `deny_unknown_fields`), and
//! widening `WidgetSpec.children`'s own element type to accommodate it
//! would ripple through every place `build.rs`/`reconcile.rs` already
//! walks that field. Expanding first, on the untyped tree, then
//! deserializing the fully-resolved result keeps `WidgetSpec` itself
//! completely unchanged.

use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};

use crate::build::SpecError;
use crate::spec::WidgetSpec;

/// Real, stated (ARCHITECTURE.md §16.6's own risk register-style
/// reasoning): not manufactured ahead of a real need, but a genuinely
/// unbounded include chain -- accidental or adversarial -- needs a
/// hard stop somewhere.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Parses `yaml`, expands every real `include:` entry against
/// `base_dir` (recursively, with path confinement/cycle detection/a
/// depth limit), and deserializes the fully-expanded tree into a
/// `WidgetSpec` -- the real, include-aware sibling of `crate::spec::
/// parse_view`, used wherever a view is loaded from an actual file on
/// disk (an `include:` path is only ever meaningful relative to one).
///
/// `base_dir: None` is real and valid -- `yaml` with no `include:`
/// entry at all parses exactly as `parse_view` would; an `include:`
/// actually encountered with no `base_dir` given fails clearly
/// (`SpecError::IncludeNoBaseDir`), not silently.
pub fn parse_view_with_includes(
    yaml: &str,
    base_dir: Option<&Path>,
) -> Result<WidgetSpec, SpecError> {
    let raw: Value = serde_yaml_ng::from_str(yaml)?;
    let mut visited = Vec::new();
    let expanded = expand_includes(raw, base_dir, &mut visited)?;
    serde_yaml_ng::from_value(expanded).map_err(SpecError::Parse)
}

fn expand_includes(
    value: Value,
    base_dir: Option<&Path>,
    visited: &mut Vec<PathBuf>,
) -> Result<Value, SpecError> {
    if visited.len() >= MAX_INCLUDE_DEPTH {
        return Err(SpecError::IncludeDepthExceeded {
            limit: MAX_INCLUDE_DEPTH,
        });
    }
    match value {
        Value::Mapping(map) => expand_mapping(map, base_dir, visited),
        Value::Sequence(seq) => {
            let mut expanded = Vec::with_capacity(seq.len());
            for item in seq {
                expanded.push(expand_includes(item, base_dir, visited)?);
            }
            Ok(Value::Sequence(expanded))
        }
        other => Ok(other),
    }
}

fn expand_mapping(
    map: Mapping,
    base_dir: Option<&Path>,
    visited: &mut Vec<PathBuf>,
) -> Result<Value, SpecError> {
    let Some(include_value) = map.get("include") else {
        // An ordinary widget mapping -- recurse into every value (this
        // is what reaches a `children:` list's own entries) without
        // otherwise touching the mapping's own shape.
        let mut rebuilt = Mapping::new();
        for (key, value) in map {
            rebuilt.insert(key, expand_includes(value, base_dir, visited)?);
        }
        return Ok(Value::Mapping(rebuilt));
    };

    let include_path = include_value
        .as_str()
        .ok_or_else(|| SpecError::IncludeValueNotString {
            value: format!("{include_value:?}"),
        })?;

    if map.len() != 1 {
        let extra_keys: Vec<String> = map
            .keys()
            .filter_map(|k| k.as_str())
            .filter(|&k| k != "include")
            .map(str::to_string)
            .collect();
        return Err(SpecError::IncludeNotSoleKey {
            path: include_path.to_string(),
            extra_keys,
        });
    }

    let Some(base_dir) = base_dir else {
        return Err(SpecError::IncludeNoBaseDir {
            path: include_path.to_string(),
        });
    };

    let resolved = resolve_confined(base_dir, include_path)?;
    if visited.contains(&resolved) {
        return Err(SpecError::IncludeCycle { path: resolved });
    }

    let included_yaml =
        std::fs::read_to_string(&resolved).map_err(|source| SpecError::IncludeReadFailed {
            path: resolved.clone(),
            source,
        })?;
    let included_value: Value = serde_yaml_ng::from_str(&included_yaml)?;

    let included_base_dir = resolved.parent().map(Path::to_path_buf);

    visited.push(resolved);
    let expanded = expand_includes(included_value, included_base_dir.as_deref(), visited);
    visited.pop();
    expanded
}

/// Real path confinement (ARCHITECTURE.md §16.6's own stated
/// requirement): `include_path` must resolve to a real file *inside*
/// `base_dir` -- an absolute path or a `../` escape is rejected.
/// `canonicalize` (not a string prefix check) resolves symlinks too,
/// so a symlink pointing outside `base_dir` can't evade this either --
/// it also requires the target to actually exist, acceptable since the
/// file has to be read regardless.
fn resolve_confined(base_dir: &Path, include_path: &str) -> Result<PathBuf, SpecError> {
    if Path::new(include_path).is_absolute() {
        return Err(SpecError::IncludePathEscapesBase {
            path: include_path.to_string(),
        });
    }
    let joined = base_dir.join(include_path);
    let canon_base = base_dir
        .canonicalize()
        .map_err(|source| SpecError::IncludeReadFailed {
            path: base_dir.to_path_buf(),
            source,
        })?;
    let canon_joined = joined
        .canonicalize()
        .map_err(|source| SpecError::IncludeReadFailed {
            path: joined.clone(),
            source,
        })?;
    if !canon_joined.starts_with(&canon_base) {
        return Err(SpecError::IncludePathEscapesBase {
            path: include_path.to_string(),
        });
    }
    Ok(canon_joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, real, uniquely-named temp directory per test -- real
    /// files on a real filesystem, the same discipline `crate::watch`'s
    /// own tests already use, not a mocked/in-memory filesystem.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "engine_spec_include_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create test subdir");
        }
        std::fs::write(&path, content).expect("write test file");
        path
    }

    #[test]
    fn a_real_two_file_include_splices_in_as_an_ordinary_child() {
        let dir = temp_dir("two_file");
        write(
            &dir,
            "dialog.yaml",
            r#"
id: confirm
kind: Container
style: {width: 100, height: 40}
"#,
        );
        let main = r#"
id: root
kind: Container
style: {width: 200, height: 200}
children:
  - include: dialog.yaml
"#;
        let spec = parse_view_with_includes(main, Some(&dir)).expect("a real include must parse");
        assert_eq!(spec.children.len(), 1);
        assert_eq!(spec.children[0].id, "confirm");
        assert!(matches!(
            spec.children[0].kind,
            crate::NodeKindSpec::Container
        ));
    }

    #[test]
    fn nested_includes_resolve_relative_to_their_own_file() {
        let dir = temp_dir("nested");
        write(
            &dir,
            "parts/inner.yaml",
            r#"
id: inner
kind: Container
style: {width: 10, height: 10}
"#,
        );
        write(
            &dir,
            "parts/outer.yaml",
            r#"
id: outer
kind: Container
style: {width: 50, height: 50}
children:
  - include: inner.yaml
"#,
        );
        let main = r#"
id: root
kind: Container
style: {width: 200, height: 200}
children:
  - include: parts/outer.yaml
"#;
        let spec =
            parse_view_with_includes(main, Some(&dir)).expect("nested includes must resolve");
        assert_eq!(spec.children[0].id, "outer");
        assert_eq!(
            spec.children[0].children[0].id, "inner",
            "inner.yaml must resolve relative to parts/, not the top-level base_dir"
        );
    }

    #[test]
    fn an_include_with_no_base_dir_fails_clearly() {
        let main = r#"
id: root
kind: Container
children:
  - include: whatever.yaml
"#;
        let err = parse_view_with_includes(main, None)
            .expect_err("an include with no base_dir must fail, not silently no-op");
        assert!(matches!(err, SpecError::IncludeNoBaseDir { .. }));
    }

    #[test]
    fn an_absolute_include_path_is_rejected() {
        let dir = temp_dir("absolute");
        let main = "id: root\nkind: Container\nchildren:\n  - include: /etc/passwd\n";
        let err = parse_view_with_includes(main, Some(&dir))
            .expect_err("an absolute include path must be rejected");
        assert!(matches!(err, SpecError::IncludePathEscapesBase { .. }));
    }

    #[test]
    fn a_relative_escape_outside_the_base_dir_is_rejected() {
        let dir = temp_dir("escape_outer");
        let inner = dir.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        write(&dir, "secret.yaml", "id: secret\nkind: Container\n");
        let main = "id: root\nkind: Container\nchildren:\n  - include: ../secret.yaml\n";
        let err = parse_view_with_includes(main, Some(&inner))
            .expect_err("a real ../ escape outside base_dir must be rejected");
        assert!(matches!(err, SpecError::IncludePathEscapesBase { .. }));
    }

    #[test]
    fn a_real_self_include_cycle_is_rejected() {
        let dir = temp_dir("cycle");
        write(
            &dir,
            "a.yaml",
            "id: a\nkind: Container\nchildren:\n  - include: a.yaml\n",
        );
        let main = "id: root\nkind: Container\nchildren:\n  - include: a.yaml\n";
        let err = parse_view_with_includes(main, Some(&dir))
            .expect_err("a file including itself must be rejected as a real cycle");
        assert!(matches!(err, SpecError::IncludeCycle { .. }));
    }

    #[test]
    fn an_include_chain_past_the_depth_limit_is_rejected() {
        let dir = temp_dir("depth");
        // A chain of MAX_INCLUDE_DEPTH + 2 files, each including the next.
        for i in 0..(MAX_INCLUDE_DEPTH + 2) {
            let next = if i + 1 < MAX_INCLUDE_DEPTH + 2 {
                format!("children:\n  - include: level_{}.yaml\n", i + 1)
            } else {
                String::new()
            };
            write(
                &dir,
                &format!("level_{i}.yaml"),
                &format!("id: level_{i}\nkind: Container\n{next}"),
            );
        }
        let main = "id: root\nkind: Container\nchildren:\n  - include: level_0.yaml\n";
        let err = parse_view_with_includes(main, Some(&dir))
            .expect_err("a chain past the real depth limit must be rejected");
        assert!(matches!(err, SpecError::IncludeDepthExceeded { .. }));
    }

    #[test]
    fn include_alongside_another_key_in_the_same_mapping_is_rejected() {
        let dir = temp_dir("extra_key");
        write(&dir, "part.yaml", "id: part\nkind: Container\n");
        let main = r#"
id: root
kind: Container
children:
  - include: part.yaml
    id: not_allowed_here
"#;
        let err = parse_view_with_includes(main, Some(&dir))
            .expect_err("include: mixed with another key must fail, not silently ignore one");
        assert!(matches!(err, SpecError::IncludeNotSoleKey { .. }));
    }

    #[test]
    fn yaml_with_no_include_at_all_parses_exactly_as_plain_parse_view_would() {
        let yaml = r##"
id: root
kind: Container
style: {width: 100, height: 100}
children:
  - id: a
    kind: Rect
    style: {width: 10, height: 10, background: "#000000"}
"##;
        let via_includes =
            parse_view_with_includes(yaml, None).expect("plain yaml with no include must parse");
        let via_plain = crate::spec::parse_view(yaml).expect("plain parse_view must also parse");
        assert_eq!(via_includes.children.len(), via_plain.children.len());
        assert_eq!(via_includes.children[0].id, via_plain.children[0].id);
    }
}
