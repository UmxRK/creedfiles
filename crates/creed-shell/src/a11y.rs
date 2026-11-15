//! Screen-reader support: each surface publishes its hit list as an AccessKit tree, exposed
//! over AT-SPI by accesskit_unix. When no assistive technology is running the adapter stays
//! inactive and costs (almost) nothing.

use crate::app::Msg;
use crate::ui::{Hit, Id, Kind, Role};
use accesskit::{Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId, Rect, Role as AkRole, Toggled, TreeId, TreeInfo, TreeUpdate};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::sync::{Arc, Mutex};

const ROOT: NodeId = NodeId(0);

struct Activation {
    latest: Arc<Mutex<Option<TreeUpdate>>>,
}
impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.latest.lock().ok()?.clone()
    }
}

struct Actions {
    uid: u64,
    tx: Mutex<Sender<Msg>>,
}
impl ActionHandler for Actions {
    fn do_action(&mut self, req: ActionRequest) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(Msg::A11y(self.uid, req.target_node.0, req.action));
        }
    }
}

struct Deactivation;
impl DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}

pub struct SurfaceA11y {
    adapter: accesskit_unix::Adapter,
    latest: Arc<Mutex<Option<TreeUpdate>>>,
    kind: Kind,
}

fn role(r: Role) -> AkRole {
    match r {
        Role::Button => AkRole::Button,
        Role::Toggle => AkRole::Switch,
        Role::Slider => AkRole::Slider,
        Role::Item => AkRole::ListItem,
        Role::Input => AkRole::SearchInput,
        Role::Label => AkRole::Label,
        Role::Group | Role::Backdrop => AkRole::GenericContainer,
    }
}

pub fn build(kind: Kind, hits: &[Hit], focus: Option<Id>, size: (f64, f64)) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(hits.len() + 1);
    let mut root = Node::new(AkRole::Window);
    root.set_label(format!("Creed {}", kind.name()));
    root.set_bounds(Rect { x0: 0.0, y0: 0.0, x1: size.0, y1: size.1 });
    let mut children = vec![];
    for h in hits.iter().filter(|h| !matches!(h.role, Role::Backdrop | Role::Group) && !h.label.is_empty()) {
        // Hit ids are hashes; 0 is reserved for the root.
        let id = NodeId(h.id.max(1));
        if children.contains(&id) {
            continue;
        }
        let mut n = Node::new(role(h.role));
        n.set_label(h.label.clone());
        n.set_bounds(Rect { x0: h.rect.x, y0: h.rect.y, x1: h.rect.x + h.rect.w, y1: h.rect.y + h.rect.h });
        n.add_action(Action::Focus);
        match h.role {
            Role::Slider => {
                n.set_numeric_value(h.value.unwrap_or(0.0) * 100.0);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.add_action(Action::Increment);
                n.add_action(Action::Decrement);
            }
            Role::Toggle => {
                n.set_toggled(if h.value.unwrap_or(0.0) > 0.5 { Toggled::True } else { Toggled::False });
                n.add_action(Action::Click);
            }
            Role::Label => {}
            _ => n.add_action(Action::Click),
        }
        children.push(id);
        nodes.push((id, n));
    }
    root.set_children(children);
    nodes.insert(0, (ROOT, root));
    let focus_id = focus.map(|f| NodeId(f.max(1))).filter(|f| nodes.iter().any(|(id, _)| id == f)).unwrap_or(ROOT);
    TreeUpdate { nodes, tree: Some(TreeInfo::new(ROOT)), tree_id: TreeId::ROOT, focus: focus_id }
}

impl SurfaceA11y {
    pub fn new(uid: u64, kind: Kind, tx: Sender<Msg>) -> SurfaceA11y {
        let latest = Arc::new(Mutex::new(None));
        let adapter = accesskit_unix::Adapter::new(Activation { latest: latest.clone() }, Actions { uid, tx: Mutex::new(tx) }, Deactivation);
        SurfaceA11y { adapter, latest, kind }
    }

    pub fn update(&mut self, hits: &[Hit], focus: Option<Id>, size: (f64, f64)) {
        let upd = build(self.kind, hits, focus, size);
        if let Ok(mut l) = self.latest.lock() {
            *l = Some(upd.clone());
        }
        self.adapter.update_if_active(|| upd);
    }

    pub fn focus(&mut self, on: bool) {
        self.adapter.update_window_focus_state(on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Rect as R;
    use crate::ui::Action as A;

    #[test]
    fn tree_shape() {
        let hits = vec![
            Hit { id: 5, rect: R::new(0.0, 0.0, 10.0, 10.0), action: A::None, role: Role::Backdrop, label: String::new(), focusable: false, value: None, scroll: None, secondary: None },
            Hit { id: 7, rect: R::new(1.0, 2.0, 3.0, 4.0), action: A::ToggleDnd, role: Role::Toggle, label: "Do Not Disturb".into(), focusable: true, value: Some(1.0), scroll: None, secondary: None },
            Hit { id: 9, rect: R::new(1.0, 2.0, 3.0, 4.0), action: A::None, role: Role::Slider, label: "Volume".into(), focusable: true, value: Some(0.5), scroll: None, secondary: None },
        ];
        let t = build(Kind::Control, &hits, Some(9), (100.0, 100.0));
        assert_eq!(t.nodes.len(), 3);
        assert_eq!(t.focus, NodeId(9));
        assert_eq!(t.nodes[0].1.children().len(), 2);
        assert_eq!(t.nodes[1].1.label(), Some("Do Not Disturb"));
        assert_eq!(t.nodes[2].1.numeric_value(), Some(50.0));
    }
}
