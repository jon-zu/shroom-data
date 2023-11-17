use std::{cell::RefCell, rc::Rc};

use serde::{
    ser::{SerializeMap, SerializeStruct},
    Serialize,
};

use crate::file::{WzIO, WzImgReader};

use super::{
    obj::WzObject,
    prop::{WzConvex2D, WzPropValue, WzVector2D},
};

pub const WZ_VEC2_STRUCT_NAME: &str = "_wz_vec2";
pub const WZ_VEX2_STRUCT_NAME: &str = "_wz_vex2";
pub const WZ_CANVAS_STRUCT_NAME: &str = "_wz_canvas";
pub const WZ_SOUND_STRUCT_NAME: &str = "_wz_sound";

impl Serialize for WzVector2D {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct(WZ_VEC2_STRUCT_NAME, 2)?;
        s.serialize_field("x", &self.x.0)?;
        s.serialize_field("y", &self.y.0)?;
        s.end()
    }
}

impl Serialize for WzConvex2D {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct(WZ_VEX2_STRUCT_NAME, 1)?;
        s.serialize_field("vectors", &self.0)?;
        s.end()
    }
}

pub struct WzValueSerializer<'r, R> {
    value: &'r WzPropValue,
    r: Rc<RefCell<WzImgReader<R>>>,
    skip_canvas: bool,
}

impl<'r, R: WzIO> Serialize for WzValueSerializer<'r, R> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let WzValueSerializer {
            r,
            value,
            skip_canvas,
        } = self;
        match &value {
            WzPropValue::Null => ser.serialize_none(),
            WzPropValue::Short1(v) | WzPropValue::Short2(v) => ser.serialize_i16(*v),
            WzPropValue::Int1(v) | WzPropValue::Int2(v) => ser.serialize_i32(v.0),
            WzPropValue::Long(v) => ser.serialize_i64(v.0),
            WzPropValue::F32(v) => ser.serialize_f32(v.0),
            WzPropValue::F64(v) => ser.serialize_f64(*v),
            WzPropValue::Str(v) => ser.serialize_str(v.0.as_str()),
            WzPropValue::Obj(obj) => {
                let r = r.clone();
                let obj_ser = WzObjectSerializer {
                    object: &obj.obj,
                    r,
                    skip_canvas: *skip_canvas,
                };
                obj_ser.serialize(ser)
            }
        }
    }
}

pub struct WzObjectSerializer<'r, R> {
    object: &'r WzObject,
    r: Rc<RefCell<WzImgReader<R>>>,
    skip_canvas: bool,
}

impl<'r, R: WzIO> Serialize for WzObjectSerializer<'r, R> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.object {
            super::obj::WzObject::Property(prop) => {
                let mut s = ser.serialize_map(prop.entries.0.len().into())?;
                for entry in prop.entries.0.iter() {
                    s.serialize_key(entry.name.0.as_str())?;
                    let val_ser = WzValueSerializer {
                        value: &entry.val,
                        r: self.r.clone(),
                        skip_canvas: self.skip_canvas,
                    };
                    s.serialize_value(&val_ser)?;
                }
                s.end()
            }
            super::obj::WzObject::Canvas(canvas) => {
                if self.skip_canvas {
                    return ser.serialize_none();
                }
                if let Some(ref prop) = canvas.property {
                    let mut s = ser.serialize_map(prop.entries.0.len().into())?;
                    for entry in prop.entries.0.iter() {
                        s.serialize_key(entry.name.0.as_str())?;
                        let val_ser = WzValueSerializer {
                            value: &entry.val,
                            r: self.r.clone(),
                            skip_canvas: self.skip_canvas,
                        };
                        s.serialize_value(&val_ser)?;
                    }
                    s.end()
                } else {
                    ser.serialize_none()
                }
            }
            super::obj::WzObject::UOL(_) => ser.serialize_none(),
            super::obj::WzObject::Vec2(vec) => vec.serialize(ser),
            super::obj::WzObject::Convex2D(vex) => vex.serialize(ser),
            super::obj::WzObject::SoundDX8(_) => ser.serialize_none(),
        }
    }
}

pub struct WzImgSerializer<R> {
    img_reader: Rc<RefCell<WzImgReader<R>>>,
    root: WzObject,
    skip_canvas: bool,
}

impl<R: WzIO> WzImgSerializer<R> {
    pub fn new(mut img_reader: WzImgReader<R>, skip_canvas: bool) -> anyhow::Result<Self> {
        let root = img_reader.read_root_obj()?;
        Ok(Self {
            img_reader: Rc::new(RefCell::new(img_reader)),
            root,
            skip_canvas,
        })
    }
}

impl<R: WzIO> Serialize for WzImgSerializer<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        WzObjectSerializer {
            object: &self.root,
            r: self.img_reader.clone(),
            skip_canvas: self.skip_canvas,
        }
        .serialize(serializer)
    }
}
