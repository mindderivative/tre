//! 0.5.10 (#253): `Window.build` makes a whole detached subtree from one nested description and
//! `Window.apply` changes properties on many nodes at once. Both check everything first: a bad
//! description or entry raises `ValueError` naming where, and nothing has been made or changed.

use std::collections::HashSet;
use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use crate::node::Node;
use crate::node_handles;
use crate::window::PyWindow;

/// The deepest nesting `build` accepts.
const MAX_DEPTH: usize = 256;

/// One node of a description, read and checked but not yet prepared.
struct SpecNode<'py> {
    kind: String,
    props: Bound<'py, PyDict>,
    id: Option<String>,
    parent: Option<usize>,
    path: String,
}

fn bad(path: &str, message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("build: {path}: {message}"))
}

/// Walks the description iteratively (no recursion on Python's stack), in pre-order.
fn read_spec<'py>(spec: &Bound<'py, PyAny>) -> PyResult<Vec<SpecNode<'py>>> {
    let mut out: Vec<SpecNode<'py>> = Vec::new();
    let mut ids: HashSet<String> = HashSet::new();
    // (spec, parent index, path, depth); a stack, children pushed in reverse so they come out in order.
    let mut stack: Vec<(Bound<'py, PyAny>, Option<usize>, String, usize)> =
        vec![(spec.clone(), None, "spec".to_string(), 0)];
    while let Some((item, parent, path, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(bad(
                &path,
                format!("nested more than {MAX_DEPTH} levels deep"),
            ));
        }
        let dict = item
            .cast::<PyDict>()
            .map_err(|_| bad(&path, "a node description must be a dict"))?;
        for key in dict.keys().iter() {
            let key: String = key.extract().map_err(|_| bad(&path, "keys must be str"))?;
            if !matches!(key.as_str(), "kind" | "props" | "children" | "id") {
                return Err(bad(
                    &path,
                    format!("unknown key {key:?} -- a node has kind, props, children and id"),
                ));
            }
        }
        let kind: String = dict
            .get_item("kind")?
            .ok_or_else(|| bad(&path, "needs `kind`"))?
            .extract()
            .map_err(|_| bad(&path, "`kind` must be a str"))?;
        let props = match dict.get_item("props")? {
            None => PyDict::new(spec.py()),
            Some(p) if p.is_none() => PyDict::new(spec.py()),
            Some(p) => p
                .cast::<PyDict>()
                .map_err(|_| bad(&format!("{path}.props"), "must be a dict"))?
                .clone(),
        };
        let id = match dict.get_item("id")? {
            None => None,
            Some(v) if v.is_none() => None,
            Some(v) => {
                let id: String = v
                    .extract()
                    .map_err(|_| bad(&format!("{path}.id"), "must be a non-empty str"))?;
                if id.is_empty() {
                    return Err(bad(&format!("{path}.id"), "must be a non-empty str"));
                }
                if !ids.insert(id.clone()) {
                    return Err(bad(&format!("{path}.id"), format!("{id:?} is used twice")));
                }
                Some(id)
            }
        };
        let index = out.len();
        out.push(SpecNode {
            kind,
            props,
            id,
            parent,
            path: path.clone(),
        });
        if let Some(children) = dict.get_item("children")?
            && !children.is_none()
        {
            let list: Vec<Bound<'py, PyAny>> = if let Ok(list) = children.cast::<PyList>() {
                list.iter().collect()
            } else if let Ok(tuple) = children.cast::<PyTuple>() {
                tuple.iter().collect()
            } else {
                return Err(bad(&format!("{path}.children"), "must be a list"));
            };
            for (i, child) in list.into_iter().enumerate().rev() {
                stack.push((
                    child,
                    Some(index),
                    format!("{path}.children[{i}]"),
                    depth + 1,
                ));
            }
        }
    }
    Ok(out)
}

#[pymethods]
impl PyWindow {
    /// Makes a whole detached subtree from one nested description and returns `(root, nodes)`:
    /// the root node and a dict of every node that has an `id`. A node is a dict with `kind`
    /// (required) and optionally `props` (what `create(kind, **props)` takes), `children` (a
    /// list of nodes) and `id` (a unique non-empty str). Everything is checked first, as
    /// `create` and `Node.set` check: a bad kind, property, key or id raises `ValueError`
    /// naming the path (`spec.children[2].props`) and nothing is made. Attach the root with
    /// `add_child`.
    fn build(&self, spec: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<(Py<Node>, Py<PyDict>)> {
        let nodes = read_spec(spec)?;
        // Phase 1: check every node and parse every property. Nothing is made.
        let mut prepared = Vec::with_capacity(nodes.len());
        for node in &nodes {
            let one = self
                .prepare_node(&node.kind, Some(&node.props), py)
                .map_err(|err| bad(&node.path, err.value(py)))?;
            prepared.push(one);
        }
        // Phase 2: make them. A terminal starts a shell, so terminals go last; the order the
        // nodes are attached in is the description's.
        let mut order: Vec<usize> = (0..prepared.len()).collect();
        order.sort_by_key(|&i| prepared[i].spawns());
        let mut slots: Vec<Option<crate::window_events::PreparedNode>> =
            prepared.into_iter().map(Some).collect();
        let mut made: Vec<Option<Py<Node>>> = (0..slots.len()).map(|_| None).collect();
        let mut failure = None;
        for i in order {
            let one = slots[i].take().expect("each node is made once");
            match self.make_node(one, py).and_then(|node| Py::new(py, node)) {
                Ok(node) => made[i] = Some(node),
                Err(err) => {
                    failure = Some((i, err));
                    break;
                }
            }
        }
        if let Some((i, err)) = failure {
            // A shell would not start (or a canvas's `draw` raised): free what was made.
            for node in made.iter().flatten() {
                let node = node.borrow(py);
                let freed = self.handles.tree.borrow_mut().destroy(node.id);
                node_handles::prune(&self.handles.handlers, &freed);
                self.handles.terminals.borrow_mut().remove(&node.id);
            }
            return Err(bad(&nodes[i].path, err.value(py)));
        }
        let made: Vec<Py<Node>> = made
            .into_iter()
            .map(|n| n.expect("every node is made"))
            .collect();
        {
            let mut tree = self.handles.tree.borrow_mut();
            for (i, node) in nodes.iter().enumerate() {
                if let Some(parent) = node.parent {
                    let (parent_id, child_id) = (made[parent].borrow(py).id, made[i].borrow(py).id);
                    tree.add_child(parent_id, child_id);
                }
            }
        }
        let by_id = PyDict::new(py);
        for (node, spec_node) in made.iter().zip(&nodes) {
            if let Some(id) = &spec_node.id {
                by_id.set_item(id, node.clone_ref(py))?;
            }
        }
        Ok((made[0].clone_ref(py), by_id.unbind()))
    }

    /// Changes properties on many nodes in one call: `changes` is a sequence of
    /// `(node, props)` pairs, each `props` what `node.set(**props)` takes. Every entry is checked
    /// first -- a node of another window, a destroyed one, a bad property -- and a bad one
    /// raises `ValueError` naming its index and changes nothing; then they apply in order (the
    /// same node twice: the later wins).
    fn apply(&self, changes: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let mut prepared = Vec::new();
        for (i, entry) in changes.try_iter()?.enumerate() {
            let entry = entry?;
            let at =
                |message: String| PyValueError::new_err(format!("apply: changes[{i}]: {message}"));
            let (node, props): (Py<Node>, Bound<'_, PyDict>) = entry
                .extract()
                .map_err(|_| at("must be a (node, props dict) pair".into()))?;
            let node = node.bind(py).borrow();
            if !Rc::ptr_eq(&node.tree, &self.handles.tree) {
                return Err(at("that node belongs to another window".into()));
            }
            let one = node
                .prepare_set(Some(&props))
                .map_err(|err| at(err.value(py).to_string()))?;
            prepared.push((entry.get_item(0)?.extract::<Py<Node>>()?, one));
        }
        for (node, one) in prepared {
            node.borrow(py).run_set(one, py)?;
        }
        Ok(())
    }
}
