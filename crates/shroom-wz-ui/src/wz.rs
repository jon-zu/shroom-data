use std::{borrow::Cow, cell::RefCell, collections::HashMap, io::Cursor, rc::Rc};

use dioxus::prelude::*;
use id_tree::{NodeId, Tree};
use image::RgbaImage;
use shroom_wz::{
    l0::{tree::WzTree, WzDirNode, WzImgHeader},
    l1::{canvas::WzCanvas, sound::WzSound, tree::WzValueNode, tree::WzValueTree},
    util::animation::Animation,
    val::WzValue,
    version::{WzRegion, WzVersion},
    WzConfig,
};

use crate::{
    audio_view::{AudioData, AudioView},
    image_view::AnimationView,
    tree::{TreeData, TreeView},
};

use crate::image_view::ImageView;

//pub type WzFile = re_wz::file::WzReaderMmap;
pub type WzFile = Cursor<Vec<u8>>;
pub type WzFileReader = shroom_wz::file::WzReader<WzFile>;

impl TreeData for WzDirNode {
    fn get_label(&self) -> Cow<'_, str> {
        match self {
            WzDirNode::Dir(dir) => format!("📁 {}", &dir.name).into(),
            WzDirNode::Nil(_) => "🚫 NIL".to_string().into(),
            WzDirNode::Link(link) => format!("🔗 {}", link.link.link_img.name).into(),
            WzDirNode::Img(img) => format!("💾 {}", &img.name).into(),
        }
    }

    fn can_select(&self) -> bool {
        matches!(self, WzDirNode::Img(_) | WzDirNode::Link(_))
    }
}

impl<'a> TreeData for WzValueNode<'a> {
    fn get_label(&self) -> Cow<'_, str> {
        let name = self.name;
        match self.value {
            WzValue::Object(_) => name.into(),
            WzValue::Null => format!("{name}: NULL").into(),
            WzValue::F32(v) => format!("{name}: {v}").into(),
            WzValue::F64(v) => format!("{name}: {v}").into(),
            WzValue::Short(v) => format!("{name}: {v}").into(),
            WzValue::Int(v) => format!("{name}: {v}").into(),
            WzValue::Long(v) => format!("{name}: {v}").into(),
            WzValue::String(v) => format!("{name}: {v}").into(),
            WzValue::Vec(v) => format!("{name}: {v}").into(),
            WzValue::Convex(v) => format!("{name}: {v:?}").into(),
            WzValue::Sound(_) => format!("♫ {name}").into(),
            WzValue::Canvas(_) => format!("🖼 {name}").into(),
            WzValue::Link(link) => format!("🔗 {name}: {link}").into(),
        }
    }

    fn can_select(&self) -> bool {
        matches!(self.value, WzValue::Canvas(_) | WzValue::Sound(_))
    }

    fn expanded_childs(&self) -> bool {
        self.can_select()
    }
}

pub struct WzData {
    tree: WzTree,
    reader: RefCell<WzFileReader>,
    val_cache: RefCell<HashMap<u32, Rc<WzValueTree>>>,
}

pub struct WzAnimationData {
    pub anim: Animation,
    pub frames: Vec<RgbaImage>,
}

impl PartialEq for WzAnimationData {
    fn eq(&self, other: &Self) -> bool {
        self.frames == other.frames
    }
}

impl Eq for WzAnimationData {}

impl WzData {
    #[cfg(feature = "mmap")]
    fn load(file: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut file = re_wz::WzReader::open_file_mmap(file, WzConfig::new(WzRegion::GMS, 95))?;
        Self::from_file(file)
    }

    pub fn from_file(filename: &str, file: WzFile, version: WzVersion) -> anyhow::Result<Self> {
        let mut file = shroom_wz::WzReader::open(file, WzConfig::new(WzRegion::GMS, version.0))?;
        let tree = WzTree::from_reader(&mut file, Some(filename))?;
        Ok(Self {
            tree,
            reader: RefCell::new(file),
            val_cache: RefCell::new(HashMap::new()),
        })
    }

    fn load_tree(&self, img: &WzImgHeader) -> anyhow::Result<&WzValueTree> {
        // let name = img.name.as_str();
        let tree = self
            .val_cache
            .borrow_mut()
            .entry(img.offset.0)
            .or_insert_with(|| {
                let mut rdr = self.reader.borrow_mut();
                let mut rdr = rdr.img_reader(img).unwrap();
                let root = WzValue::read(&mut rdr).unwrap();
                Rc::new(WzValueTree::build_from_img(img.clone(), root))
            })
            .clone();

        // Safety: The cache holds the RC alive until
        // It's dropped from the cache
        // Since the cache is never dropped It means It lives as long as &self does
        Ok(unsafe { std::mem::transmute(tree.as_ref()) })
    }

    fn load_anim(&self, img: &WzImgHeader, anim: Animation) -> anyhow::Result<WzAnimationData> {
        let frames = anim.load_all_frames(&mut self.reader.borrow_mut().img_reader(img)?)?;
        let frames = frames
            .into_iter()
            .map(|frame| frame.to_raw_rgba_image().unwrap())
            .collect();
        Ok(WzAnimationData { anim, frames })
    }

    fn load_canvas(&self, img: &WzImgHeader, canvas: &WzCanvas) -> anyhow::Result<RgbaImage> {
        self.reader
            .borrow_mut()
            .img_reader(img)?
            .read_canvas(canvas)?
            .to_raw_rgba_image()
    }

    fn load_sound(&self, img: &WzImgHeader, sound: &WzSound) -> anyhow::Result<AudioData> {
        let data = self
            .reader
            .borrow_mut()
            .img_reader(img)?
            .read_sound(sound)?;

        Ok(AudioData {
            data,
            format: sound.clone(),
        })
    }
}

pub enum WzContentData {
    Image(Rc<RgbaImage>),
    Animation(Rc<WzAnimationData>),
    Sound(Rc<AudioData>),
    Text(String),
    None,
}

#[inline_props]
fn WzContentView(cx: Scope, content: UseState<WzContentData>) -> Element {
    cx.render(match content.get() {
        WzContentData::Image(img) => rsx!(div {
            ImageView {
                image: img.clone()
            }
        }),
        WzContentData::Sound(sound) => rsx!(div {
            AudioView {
                audio: sound.clone()
            }
        }),
        WzContentData::Animation(ref anim) => rsx!(div {
            AnimationView {
                anim_data: anim.clone()
            }
        }),
        WzContentData::Text(ref txt) => rsx!(div {
            div {
                class: "card",
                div {
                    class: "card-body",
                    txt.clone()
                }
            }
        }),
        _ => rsx!(div {
            div {
                class: "card",
                div {
                    class: "card-body"
                }
            }
        }),
    })
}

#[inline_props]
fn WzImgView<'wz>(
    cx: Scope<'wz>,
    wz: &'wz WzData,
    img: &'wz WzImgHeader,
    on_select: EventHandler<'wz, (&'wz Tree<WzValueNode<'wz>>, NodeId, &'wz WzValueNode<'wz>)>,
) -> Element {
    let img_tree = wz.load_tree(img).expect("Must load img");
    let tree = img_tree.borrow_tree();

    cx.render(rsx! {
        TreeView {
            data: tree,
            on_select: move |node: NodeId| on_select.call((tree, node.clone(), tree.get(&node).unwrap().data())),
        }
    })
}

#[inline_props]
fn WzView<'wz>(cx: Scope<'wz>, wz: &'wz WzData) -> Element {
    let tree = wz.tree.get_tree();

    let selected_img_node = use_state::<Option<NodeId>>(cx, || None);
    let content = use_state(cx, || WzContentData::None);

    let selected_img = use_memo(cx, (selected_img_node.get(),), move |(node,)| {
        let Some(node) = node else {
            return None;
        };

        let img_data = wz.tree.get_tree().get(&node).unwrap().data();
        let img = match img_data {
            WzDirNode::Img(img) => img,
            WzDirNode::Link(link) => &link.link.link_img,
            _ => return None,
        };

        Some(img.clone())
    });

    let on_select_node = |(tree, node_id, node): (
        &'wz id_tree::Tree<WzValueNode<'wz>>,
        NodeId,
        &'wz WzValueNode<'wz>,
    )| {
        match node.value {
            WzValue::Canvas(canvas) => {
                // Check if the parent is an object
                if let Some(parent) = tree.ancestor_ids(&node_id).unwrap().next() {
                    let parent = tree.get(parent).unwrap().data();
                    if let Ok(anim) = Animation::from_obj_value(parent.value.as_object().unwrap()) {
                        let anim_data = wz.load_anim(selected_img.as_ref().unwrap(), anim).unwrap();
                        content.set(WzContentData::Animation(Rc::new(anim_data)));
                        return;
                    }
                }
                let img = selected_img.as_ref().unwrap();
                let img = wz.load_canvas(img, &canvas.canvas).unwrap();
                content.set(WzContentData::Image(Rc::new(img)));
            }
            WzValue::Sound(sound) => {
                let img = selected_img.as_ref().unwrap();
                let sound = wz.load_sound(img, &sound.sound).unwrap();
                content.set(WzContentData::Sound(Rc::new(sound)));
            }
            _ => content.set(WzContentData::None),
        }
    };

    let img_view = selected_img.as_ref().map(move |img| {
        rsx!(div {
                class: "flex-initial w-96 overflow-auto max-h-screen",
                WzImgView {
                    wz: wz,
                    img: img,
                    on_select: on_select_node
                }
        })
    });

    cx.render(rsx! {
        div {
            class: "flex gap-x-2",
            div {
                class: "flex-initial w-64 overflow-auto max-h-screen",
                TreeView {
                    data: tree,
                    on_select: move |node: NodeId| selected_img_node.set(Some(node))
                }
            }
            img_view

            div {
                class: "flex-1",
                WzContentView {
                    content: content.clone()
                }
            }
        }
    })
}

#[inline_props]
pub fn WzApp(cx: Scope, wz: UseState<Option<Rc<WzData>>>) -> Element {
    cx.render(rsx! {
        WzView {
            wz: wz.get().as_ref().unwrap()
        }
    })
}
