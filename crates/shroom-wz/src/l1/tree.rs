use std::collections::VecDeque;

use id_tree::Tree;

use crate::{l0::WzImgHeader, val::WzValue};

pub struct WzValueNode<'a> {
    pub name: &'a str,
    pub value: &'a WzValue,
}

#[ouroboros::self_referencing]
pub struct WzValueTree {
    pub img_hdr: crate::l0::WzImgHeader,
    pub root: WzValue,
    #[borrows(root)]
    #[covariant]
    pub tree: id_tree::Tree<WzValueNode<'this>>,
}

impl WzValueTree {
    pub fn build_from_img(img_hdr: WzImgHeader, root: WzValue) -> Self {
        Self::new(img_hdr, root, |root| {
            let mut tree = Tree::new();
            let node = tree
                .insert(
                    id_tree::Node::new(WzValueNode {
                        name: "root",
                        value: root,
                    }),
                    id_tree::InsertBehavior::AsRoot,
                )
                .unwrap();

            let mut q = VecDeque::new();
            q.push_back((root, node));

            while let Some((val, node)) = q.pop_front() {
                let obj = match val {
                    WzValue::Object(obj) => obj,
                    WzValue::Canvas(canvas) => {
                        if let Some(WzValue::Object(obj)) = canvas.sub.as_deref() {
                            obj
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                for (k, v) in obj.0.iter() {
                    let child = tree
                        .insert(
                            id_tree::Node::new(WzValueNode { name: k, value: v }),
                            id_tree::InsertBehavior::UnderNode(&node),
                        )
                        .unwrap();
                    q.push_back((v, child));
                }
            }

            tree
        })
    }
}
