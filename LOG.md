# LOG — Branch `0.3.5`: Milestone 98

- The user: "Start M98." Tesserae's M97 Step 6 run found its production code
  uses none of M98's names.
- The user kept `Window.set_theme` until M99 ("Keep it until M99"), so its
  theme types move into `engine-py`, and `serde_yaml_ng` and `pythonize`
  stay until M99.

## Done

1. Step 1 (`2e86884`): `engine-spec`, `View`, `Component`, the binding
   evaluator, the recording functions, and `from_view`/`show_view` deleted;
   `set_theme`'s theme types moved to `engine-py/src/theme_spec.rs`.
2. Step 2 (`f5547de`): the Python reactivity layer, the declarative tests,
   examples, YAML, showcase panel, migration tool, and docs pages removed;
   `threadsafe_reload.py` rewritten.
3. Step 3: the full chain green -- cargo 471, pytest 993 (984 headless),
   76 examples, showcase, mkdocs strict, 174/174 names documented.

## Status

**Complete.** M99 waits on Tesserae's M40–M42.
