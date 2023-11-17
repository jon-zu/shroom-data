pub mod canvas;
pub mod crypto;
pub mod ctx;
pub mod file;
pub mod keys;
pub mod l0;
pub mod l1;
pub mod ty;
pub mod util;
pub mod val;
pub mod version;

use std::{
    io::{Seek, Write},
    rc::Rc,
};

use binrw::BinWrite;
use crypto::WzCrypto;
use ctx::{WzImgWriteCtx, WzStrWriteTable};
#[cfg(feature = "mmap")]
pub use file::mmap::{WzReaderMmap, WzReaderSharedMmap};
pub use file::WzReader;
use l1::{
    obj::{wz_ty_str, WzObject, OBJ_TYPE_PROPERTY},
    prop::{WzConvex2D, WzPropValue, WzUOL, WzVector2D},
    str::WzImgStr,
};
use ty::{WzF32, WzInt, WzLong, WzStr};
use val::{ObjectVal, WzValue};
use version::WzVersion;

#[derive(Debug, Clone, Copy)]
pub struct WzConfig {
    pub region: version::WzRegion,
    pub version: WzVersion,
}

impl WzConfig {
    pub const fn new(region: version::WzRegion, version: u16) -> Self {
        Self {
            region,
            version: WzVersion(version),
        }
    }

    pub const fn gms(version: u16) -> Self {
        Self {
            region: version::WzRegion::GMS,
            version: WzVersion(version),
        }
    }
}

pub const GMS95: WzConfig = WzConfig::gms(95);

pub struct WzImgBuilder<W> {
    crypto: WzCrypto,
    string_table: WzStrWriteTable,
    writer: W,
}

impl<W: Write + Seek> WzImgBuilder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            crypto: WzCrypto::from_cfg(GMS95, 0),
            string_table: WzStrWriteTable::default(),
            writer,
        }
    }

    fn write_property(&mut self, obj: &ObjectVal) -> anyhow::Result<()> {
        wz_ty_str(OBJ_TYPE_PROPERTY).write_le_args(
            &mut self.writer,
            WzImgWriteCtx {
                crypto: &self.crypto,
                str_table: &self.string_table,
            },
        )?;
        (0u16).write_le_args(&mut self.writer, ())?;
        for (key, value) in obj.0.iter() {
            WzImgStr::new(key.clone()).write_le_args(
                &mut self.writer,
                WzImgWriteCtx {
                    crypto: &self.crypto,
                    str_table: &self.string_table,
                },
            )?;
            self.write_value(&value)?;
        }

        Ok(())
    }

    pub fn write_value(&mut self, value: &WzValue) -> anyhow::Result<()> {
        let ctx = WzImgWriteCtx {
            crypto: &self.crypto,
            str_table: &self.string_table,
        };

        match value {
            WzValue::Object(obj) => self.write_property(obj)?,
            WzValue::Sound(_) => todo!(),
            WzValue::Canvas(_canvas) => {}
            WzValue::Link(link) => {
                let entry_link = WzImgStr::new(link.clone());
                WzObject::UOL(WzUOL {
                    unknown: 0,
                    entries: entry_link,
                })
                .write_le_args(&mut self.writer, ctx)?
            }
            WzValue::Convex(v) => {
                let vex = WzConvex2D(
                    v.0.iter()
                        .map(|v| WzVector2D {
                            x: WzInt(v.x),
                            y: WzInt(v.y),
                        })
                        .collect(),
                );
                WzObject::Convex2D(vex).write_le_args(&mut self.writer, ctx)?;
            }
            WzValue::Vec(v) => {
                WzObject::Vec2(WzVector2D {
                    x: WzInt(v.x),
                    y: WzInt(v.y),
                })
                .write_le_args(&mut self.writer, ctx)?;
            }
            WzValue::Null => {
                WzPropValue::Null.write_le_args(&mut self.writer, ctx)?;
            }
            WzValue::F32(v) => WzPropValue::F32(WzF32(*v)).write_le_args(&mut self.writer, ctx)?,
            WzValue::F64(v) => WzPropValue::F64(*v).write_le_args(&mut self.writer, ctx)?,
            WzValue::Short(v) => WzPropValue::Short1(*v).write_le_args(&mut self.writer, ctx)?,
            WzValue::Int(v) => WzPropValue::Int1(WzInt(*v)).write_le_args(&mut self.writer, ctx)?,
            WzValue::Long(v) => {
                WzPropValue::Long(WzLong(*v)).write_le_args(&mut self.writer, ctx)?
            }
            WzValue::String(v) => WzPropValue::Str(WzImgStr(Rc::new(WzStr(v.clone()))))
                .write_le_args(&mut self.writer, ctx)?,
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rodio::{OutputStream, Source};

    use crate::{
        l0::{tree::WzTree, WzDirNode},
        l1::obj::WzObject,
        val::WzValue,
        WzReader, GMS95,
    };

    fn get_file_from_home(path: &str) -> std::path::PathBuf {
        #[allow(deprecated)]
        let home = std::env::home_dir().unwrap();
        home.join(path)
    }

    #[test]
    fn quest_str() -> anyhow::Result<()> {
        let mut r =
            WzReader::open_file(get_file_from_home("Dokumente/shared_vm/wz/Quest.wz"), GMS95)?;

        let tree = WzTree::from_reader(&mut r, None)?;
        let img_hdr = tree.get_img_by_path("QuestData/28376.img").unwrap();
        //let img_hdr = tree.get_img_by_path("3110302.img").unwrap();

        let mut img_rdr = r.img_reader(img_hdr)?;
        let val = WzValue::read(&mut img_rdr)?;
        dbg!(val);
        Ok(())
    }

    #[test]
    fn load_map() -> anyhow::Result<()> {
        let mut skill =
            WzReader::open_file(get_file_from_home("Dokumente/shared_vm/wz/Map.wz"), GMS95)?;

        let tree = WzTree::from_reader(&mut skill, None)?;

        let img_hdr = tree.get_img_by_path("Back/midForest.img").unwrap();
        //let img_hdr = tree.get_img_by_path("3110302.img").unwrap();

        let mut img_rdr = skill.img_reader(img_hdr)?;
        let val = WzValue::read(&mut img_rdr)?;

        let back = val.get_path("back/0").unwrap();
        let back = back.as_canvas().unwrap();
        let img = img_rdr.read_canvas(&back.canvas)?;
        let img = img.to_raw_rgba_image()?;
        img.save("back.png")?;
        Ok(())
    }

    #[test]
    fn load_mob() -> anyhow::Result<()> {
        let mut skill =
            WzReader::open_file(get_file_from_home("Dokumente/shared_vm/wz/Mob.wz"), GMS95)?;

        let tree = WzTree::from_reader(&mut skill, None)?;

        let img_hdr = tree.get_img_by_path("9500332.img").unwrap();
        //let img_hdr = tree.get_img_by_path("3110302.img").unwrap();
        let mut img_rdr = skill.img_reader(img_hdr)?;
        let val = WzValue::read(&mut img_rdr)?;

        let canvas = val.get_path("attack1/info/effect/5").unwrap();
        //let canvas = val.get_path("move/0").unwrap();
        let canvas = &canvas.as_canvas().unwrap().canvas;
        dbg!(&canvas);

        let img = img_rdr.read_canvas(&canvas)?;
        let img = img.to_raw_rgba_image()?;
        img.save("mob5.png")?;

        Ok(())
    }

    #[test]
    fn load3() -> anyhow::Result<()> {
        let mut skill =
            WzReader::open_file(get_file_from_home("Dokumente/shared_vm/wz/Skill.wz"), GMS95)?;

        let tree = WzTree::from_reader(&mut skill, None)?;

        let img_hdr = tree.get_img_by_path("1111.img").unwrap();

        dbg!(img_hdr);
        let check = skill.checksum(img_hdr.offset.0 as u64, img_hdr.blob_size.0 as u64)?;
        dbg!(check);

        /*
        let mut img = skill.img_reader(tree.get_img_by_path("1111.img").unwrap())?;
        let val = WzValue::read(&mut img)?;

        let effect = val.get_path("skill/11111001/effect").unwrap();
        let anim = Animation::from_obj_value(effect.as_object().unwrap()).unwrap();

        let webp = anim.to_webp(&mut img).unwrap();

        let mut webp_file = File::create("effect.webp").unwrap();
        webp_file.write_all(&webp).unwrap();*/

        Ok(())
    }

    #[test]
    fn load() -> anyhow::Result<()> {
        let mut item =
            WzReader::open_file(get_file_from_home("Documents/open-rust-ms/Item.wz"), GMS95)?;

        let root = item.read_root_dir()?;
        let WzDirNode::Dir(ref pet) = root.entries.0[1] else {
            anyhow::bail!("Invalid pet");
        };
        let pets = item.read_dir_node(pet)?;
        let WzDirNode::Img(ref pet0) = pets.entries.0[0] else {
            anyhow::bail!("Invalid pet");
        };

        let mut img = item.img_reader(pet0)?;
        let root = img.read_root_obj()?;

        let WzObject::Canvas(ref canvas) = img.read_path(&root, "info/icon")? else {
            anyhow::bail!("Invalid canvas");
        };

        let icon = img.read_canvas(canvas)?;
        let icon_img = icon.to_raw_rgba_image()?;
        icon_img.save("icon.png")?;

        let v = WzValue::read(&mut img)?;
        dbg!(&v);

        Ok(())
    }

    #[test]
    fn load_audio() {
        let mut sound =
            WzReader::open_file(get_file_from_home("Dokumente/shared_vm/wz/Sound.wz"), GMS95)
                .unwrap();

        let tree = WzTree::from_reader(&mut sound, None).unwrap();
        let mob = tree.get_img_by_path("BgmGL.img").unwrap();

        let mut img = sound.img_reader(&mob).unwrap();
        let val = WzValue::read(&mut img).unwrap();

        let sound = val
            .get_path("Amorianchallenge")
            .unwrap()
            .as_sound()
            .unwrap();

        let sound_data = sound.read_data(&mut img).unwrap();
        let dec = rodio::Decoder::new(std::io::Cursor::new(sound_data)).unwrap();
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        stream_handle.play_raw(dec.convert_samples()).unwrap();
        std::thread::sleep(sound.duration());

        /*

        let entry = img.read_root_obj().unwrap();

        let prop = entry.unwrap_property();
        let first_prop = &prop.entries.0[10];
        let first = first_prop.val.clone().unwrap_obj();

        let entry = img.read_obj(&first).unwrap();

        dbg!(entry);
        let prop = entry.unwrap_property();
        let first_prop = &prop.entries.0[0];
        let first = first_prop.val.clone().unwrap_obj();

        let sound = img.read_obj(&first).unwrap();
        let sound = sound.unwrap_sound_dx_8();
        dbg!(sound.data_size());

        let sound_data = img.read_sound(&sound).unwrap();
        let mut dec = rodio::Decoder::new(std::io::Cursor::new(sound_data)).unwrap();

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();

        stream_handle.play_raw(dec.convert_samples());
        std::thread::sleep(std::time::Duration::from_secs(5));*/
    }
}
