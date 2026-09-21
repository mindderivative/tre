# PLAN — M44: Widen Live-Bindable Properties (`background` Color Bindings)

## Goal
Widen `tre`'s live-bindable (`{{ }}`) properties beyond `opacity`/
`corner_radius`/`checked`/`text` -- specifically, make `background` a
real bindable property (hex/CSS-named string or an `(r,g,b,a)` tuple),
by fixing the real dispatch bug behind the gap. Richer reactivity
(`Computed`/`Effect`/`batch()`/`untrack()`) was scoped in the same
approved plan but deliberately not implemented -- see the approved plan
(`/home/phil/.claude/plans/reflective-sleeping-falcon.md`) and
`BUILD_TRACKER.md`'s own M44 section for the full item-2 scoping.

## Steps
1. Investigated `Node::animate()` (`node.rs`) directly: already supports
   `background`/`transform`/`shape` as composite values, and every
   numeric property beyond opacity/corner_radius, imperatively.
2. Found the real gap: `apply_binding_value` (`view.rs`) dispatched by
   the resolved `Value`'s own runtime type, not by `property` -- `Bool`
   always went to `set_checked`, `Str` always to `set_text`, and any
   non-primitive value (`Value::Handle`) was rejected outright.
3. Rewrote `apply_binding_value` to dispatch property-name-first:
   `checked`/`text` stay special-cased but gated by `property` now
   (a real type-mismatch error if the wrong `Value` shape shows up);
   everything else forwards to `animate()`. A `Value::Str` resolved for
   `property == "background"` is parsed via new `parse_background_color`
   (pure, GIL-free, uses `peniko::color::parse_color`, the same real
   parser `engine_spec::build::resolve_color` uses for static colors).
   A `Value::Handle` is recovered via `PyViewModelResolver::to_pyobject`
   (widened `private` -> `pub(crate)`) and forwarded to `animate()`.
4. Both real call sites (`BindingCallback::__call__`,
   `attach_bindings_and_handlers`) updated to pass `&resolver` through.
5. New Rust unit tests (`view.rs`'s own `#[cfg(test)]` module, +4):
   `parse_background_color` exact-value coverage for hex, hex+alpha,
   CSS-named, and its error message on bad input.
6. New pytest tests (`tests/test_view_binding.py`, +6): hex-string and
   RGBA-tuple `background` bindings apply/re-evaluate without raising;
   an invalid color string raises with the bad value named; a `Bool`
   bound to `background` is rejected directly; `checked`/`text` give
   clear, specific type-mismatch errors.
7. New live example `examples/bindable_background.py` + `.yaml`: two
   swatches (hex-string Signal, RGBA-tuple Signal) cycling color via a
   real dispatched click, then a genuine 60-frame `App.run()`.
8. `.pyi`/`BUILD_TRACKER.md` updated (no `.pyi` change needed -- nothing
   touched here is Python-visible API).

## Status
Complete, single phase. Full verification chain green: `cargo check`/
`clippy -D warnings`/`fmt`, `cargo test --workspace --release`
(`engine-py` 15, up from 11, +4), `maturin develop --release`, `pytest
tests/` (606 passed, up from 600, +6, 1 skipped unchanged), all 81
examples (+1), showcase demo. **M44 -- Widening Live-Bindable
Properties -- is now fully complete.** This closes the milestone -- per
the standing "push only after a full milestone closes" convention, a
`git push` is now appropriate.
