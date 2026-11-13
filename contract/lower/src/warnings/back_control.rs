//! `lower-navigation-back-id`: a navigation root whose literal
//! `navigationBack` names an id no element in the view has, beside a control
//! that presses an action of that name (`navigationBack="back"` with
//! `button press=back` and no `id="back"`, the action read as what Back
//! runs). The platform's Back (Android's Back, a pushed screen's edge swipe, a
//! sheet's swipe down, the agent's `back`) then cannot press that control
//! through the name: it falls to the root's `navigate`, else the router's
//! Back, so what the control's action does as a screen closes (end a session,
//! stop a sound) may not run on Back.
//!
//! Naming a Back control is optional, and a name that matches nothing is not
//! a mistake by itself, so the warning is for the likely mix-up of an action
//! with a control's id. Only what is certain is named, as `lower-geometry-id`
//! does: the ids are every `id` the expanded view writes, and a computed `id`
//! anywhere, or a use left unexpanded, may be the one, so with one there is
//! no warning. The namesake comparison uses expanded action names, so it can
//! miss an action named `back` locally inside a child component.

use super::geometry::Ids;
use super::literal;
use crate::LowerError;
use contract_syntax::{Expr, Node};
use contract_types::Checked;

/// The navigation root's `navigationBack`, when it names an id no element has
/// and a control presses the action of that name.
pub(super) fn unnamed(checked: &Checked<'_>, out: &mut Vec<LowerError>) {
    let root = &checked.expanded.root;
    // The navigation root is the view's first top-level node, an element with
    // a `navigationKey`, as lowering takes it (`NavPlace::is_root`).
    let Some(Node::Element { attrs, .. }) = root.view.first() else {
        return;
    };
    if !attrs.iter().any(|a| a.name == "navigationKey") {
        return;
    }
    let Some(written) = attrs
        .iter()
        .rev()
        .find(|a| a.name == "navigationBack" && a.written)
    else {
        return;
    };
    let Some(id) = literal(attrs, "navigationBack").filter(|id| !id.is_empty()) else {
        return;
    };
    let Some(view) = Ids::of(&root.view) else {
        return;
    };
    if view.ids.contains(id) || !presses(&root.view, id) {
        return;
    }
    let repair = if view.test_ids.contains(id) {
        format!("`testId=\"{id}\"` names an element for tests and the agent, not for Back: if Back should press that control, give it `id=\"{id}\"` as well")
    } else if let Some(near) = contract_syntax::suggestion(id, view.ids.iter().map(String::as_str))
    {
        format!("Did you mean `navigationBack=\"{near}\"`? Otherwise, if Back should press the control with `press={id}`, give it `id=\"{id}\"`")
    } else {
        format!("If Back should press the control with `press={id}`, give it `id=\"{id}\"`")
    };
    out.push(LowerError {
        id: "lower-navigation-back-id",
        message: format!(
            "`navigationBack=\"{id}\"` names a control's `id`, not an action, and nothing in the view has `id=\"{id}\"`. Back therefore cannot press the control with `press={id}` through this name; it falls back to the root's `navigate` handler, or to the router's Back when there is no handler. {repair}"
        ),
        span: written.span,
    });
}

/// Whether a control in the view presses the action `name` (`press=name` or
/// `press=name(…)`).
fn presses(view: &[Node], name: &str) -> bool {
    let mut stack: Vec<&Node> = view.iter().collect();
    while let Some(node) = stack.pop() {
        if let Node::Element { attrs, .. } = node {
            if attrs.iter().any(|a| {
                a.name == "press"
                    && matches!(&a.value, Expr::Ident(n, _) | Expr::Call(n, _, _) if n == name)
            }) {
                return true;
            }
        }
        stack.extend(node.children());
    }
    false
}
