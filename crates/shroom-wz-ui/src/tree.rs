use std::{borrow::Cow, collections::BTreeSet};

use dioxus::prelude::*;
use id_tree::{NodeId, Tree};

pub trait TreeData {
    fn get_label(&self) -> Cow<'_, str>;
    fn can_select(&self) -> bool;
    fn expanded_childs(&self) -> bool {
        false
    }
}

pub struct TreeNodeCtx {
    selected: Option<NodeId>,
    expanded: BTreeSet<NodeId>,
    expand_all: bool,
}

impl TreeNodeCtx {
    pub fn on_expand_toggle(&mut self, node_id: NodeId) {
        if self.expanded.contains(&node_id) {
            self.expanded.remove(&node_id);
        } else {
            self.expanded.insert(node_id);
        }
    }

    pub fn on_select(&mut self, node_id: NodeId) {
        self.selected = Some(node_id);
    }

    pub fn is_selected(&self, node_id: &NodeId) -> bool {
        self.selected.as_ref() == Some(node_id)
    }

    pub fn is_expanded(&self, node_id: &NodeId) -> bool {
        self.expand_all || self.expanded.contains(node_id)
    }
}

#[derive(Props)]
pub struct TreeNodeProps<'a, T> {
    tree: &'a Tree<T>,
    node_id: NodeId,
    level: usize,
    on_select: EventHandler<'a, NodeId>,
}

fn TreeNode<'a, T: TreeData>(cx: Scope<'a, TreeNodeProps<'a, T>>) -> Element<'a> {
    let TreeNodeProps {
        tree,
        node_id,
        level,
        on_select,
    } = cx.props;
    let ctx = use_shared_state::<TreeNodeCtx>(cx).unwrap();

    let node = tree.get(node_id).unwrap();
    let data = node.data();
    let is_leaf = node.children().is_empty();
    let expand_childs = data.expanded_childs();
    let expanded = expand_childs || ctx.read().is_expanded(node_id);

    let childs = tree
        .children_ids(node_id)
        .unwrap()
        .enumerate()
        .map(|(i, node)| {
            rsx!(TreeNode {
                key: "{i}"
                tree: tree,
                node_id: node.clone(),
                level: level + 1,
                on_select: move |node| on_select.call(node),
            })
        });

    let label = data.get_label();
    let active = if ctx.read().is_selected(node_id) {
        "active"
    } else {
        ""
    };

    let elem = if !is_leaf {
        rsx!(details{
            prevent_default: "onclick",
            class: active,
            open: expanded,
            onclick: move |_| {
                let mut ctx = ctx.write();
                if !is_leaf && !expand_childs {
                    ctx.on_expand_toggle(node_id.clone());
                }

                if data.can_select() {
                    ctx.on_select(node_id.clone());
                    on_select.call(node_id.clone());
                }
            },
            summary {
                class: "active",
                "{label}"
            }
            ul {
                childs
            }
        })
    } else {
        rsx!(a {
            class: active,
            onclick: move |_| {
                let mut ctx = ctx.write();
                if !is_leaf && !expand_childs  {
                    ctx.on_expand_toggle(node_id.clone());
                }

                if data.can_select() {
                    ctx.on_select(node_id.clone());
                    on_select.call(node_id.clone());
                }
            },
            "{label}"
        })
    };
    cx.render(rsx!(li { elem }))
}

#[derive(Props)]
pub struct TreeProps<'a, T> {
    data: &'a Tree<T>,
    on_select: EventHandler<'a, NodeId>,
}

pub fn TreeView<'a, T: TreeData>(cx: Scope<'a, TreeProps<'a, T>>) -> Element<'a> {
    let tree = &cx.props.data;
    let on_select = &cx.props.on_select;
    let root_id = tree.root_node_id().unwrap();

    use_shared_state_provider(cx, || TreeNodeCtx {
        selected: None,
        expanded: BTreeSet::new(),
        expand_all: false,
    });

    let _ctx = use_shared_state::<TreeNodeCtx>(cx).expect("Tree context");

    cx.render(rsx!(
    ul {
        class: "menu menu-xs bg-base-200 rounded-lg max-w-xs w-full",
        /*li {
            class: "list-group-item",
            button {
                class: "btn btn-primary m-1",
                onclick: move |_| {
                    ctx.write().expand_all = true;
                },
                "Expand All"
            }
            button {
                class: "btn btn-primary",
                onclick: move |_| {
                    ctx.write().expanded.clear();
                    ctx.write().expand_all = false;
                },
                "Collapse All"
            }
        },*/
        TreeNode {
            tree: tree,
            node_id: root_id.clone(),
            level: 0,
            on_select: move |node| on_select.call(node)
        }
    }))
}
