"""tre v2 -- Python-facing declarative/imperative GUI framework.

§14 step 6: this package currently re-exports only what `engine-py`'s
`#[pymodule]` (`tre._core`) exposes -- `App`/`Node`, minimal node
creation and one property setter (`Node.animate`). The rest of the
public Python surface (a real `ViewModel`/`Bindable`/`bind()`, §8's own
MVVM sketch) lands at step 12, once `BindingResolver` exists.
"""

from tre._core import App, Node

__all__ = ["App", "Node"]
