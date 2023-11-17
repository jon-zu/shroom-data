use std::collections::VecDeque;

use id_tree::{InsertBehavior, Node, Tree};

use crate::{file::WzIO, WzReader};

use super::{WzDirHeader, WzDirNode, WzImgHeader};

#[derive(Debug)]
pub struct WzTree {
    tree: Tree<WzDirNode>,
}

impl WzTree {
    pub fn from_reader<R: WzIO>(r: &mut WzReader<R>, name: Option<&str>) -> anyhow::Result<Self> {
        let mut tree = Tree::new();

        let off = r.root_offset();

        let root_id = tree.insert(
            Node::new(WzDirNode::Dir(WzDirHeader::root(
                name.unwrap_or("Root"),
                1,
                off,
            ))),
            InsertBehavior::AsRoot,
        )?;
        let root = r.read_root_dir()?;
        let mut q = VecDeque::new();
        q.push_back((root_id, root));

        while let Some((parent_id, dir)) = q.pop_front() {
            for val in dir.entries.0.iter() {
                let new_node = tree
                    .insert(
                        Node::new(val.clone()),
                        InsertBehavior::UnderNode(&parent_id),
                    )
                    .unwrap();

                if let WzDirNode::Dir(dir) = val {
                    q.push_back((new_node, r.read_dir_node(dir)?));
                }
            }
        }

        Ok(Self { tree })
    }

    pub fn get_tree(&self) -> &Tree<WzDirNode> {
        &self.tree
    }

    pub fn get_by_path(&self, path: &str) -> Option<&WzDirNode> {
        let mut cur = self.tree.root_node_id()?;
        for part in path.split('/') {
            let sub = self
                .tree
                .children_ids(cur)
                .unwrap()
                .find(|x| self.tree.get(x).unwrap().data().name() == Some(part));

            if let Some(sub) = sub {
                cur = sub;
            } else {
                return None;
            }
        }

        Some(self.tree.get(cur).unwrap().data())
    }

    pub fn get_img_by_path(&self, path: &str) -> Option<&WzImgHeader> {
        self.get_by_path(path).and_then(|x| match x {
            WzDirNode::Img(img) => Some(img),
            _ => None,
        })
    }
}
