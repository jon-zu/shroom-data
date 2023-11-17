use std::{
    fmt::Display,
    ops::{Index, IndexMut},
    time::Duration,
};

use derive_more::IsVariant;
use indexmap::IndexMap;

use crate::{
    canvas::Canvas,
    file::{WzIO, WzImgReader},
    l1::{
        canvas::WzCanvas,
        obj::WzObject,
        prop::{WzPropValue, WzProperty, WzVector2D},
        sound::WzSound,
    },
};

use serde::ser::SerializeMap;

pub type Map = IndexMap<String, WzValue>;

#[derive(Clone)]
pub struct CanvasVal {
    pub canvas: WzCanvas,
    pub sub: Option<Box<WzValue>>,
}

impl PartialEq for CanvasVal {
    fn eq(&self, other: &Self) -> bool {
        self.canvas.len.pos == other.canvas.len.pos
    }
}

impl serde::Serialize for CanvasVal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_map(Some(5))?;

        s.serialize_entry("$ty", "canvas")?;
        s.serialize_entry("scale", &self.canvas.scale.0)?;
        s.serialize_entry("sub", &self.sub)?;

        s.end()
    }
}

impl std::fmt::Debug for CanvasVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasVal").field("sub", &self.sub).finish()
    }
}

impl CanvasVal {
    pub fn read_canvas<R: WzIO>(&self, r: &mut WzImgReader<R>) -> anyhow::Result<Canvas> {
        r.read_canvas(&self.canvas)
    }
}

#[derive(Debug, Clone)]
pub struct SoundVal {
    pub sound: WzSound,
}

impl PartialEq for SoundVal {
    fn eq(&self, other: &Self) -> bool {
        self.sound.offset.pos == other.sound.offset.pos
    }
}

impl serde::Serialize for SoundVal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_map(Some(5))?;

        s.serialize_entry("$ty", "sound")?;
        s.serialize_entry("playTime", &self.sound.len_ms.0)?;

        s.end()
    }
}

impl SoundVal {
    pub fn read_data<R: WzIO>(&self, r: &mut WzImgReader<R>) -> anyhow::Result<Vec<u8>> {
        r.read_sound(&self.sound)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.sound.len_ms.0 as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vec2Val {
    pub x: i32,
    pub y: i32,
}

impl serde::Serialize for Vec2Val {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_map(Some(3))?;
        s.serialize_entry("$type", "vec2")?;
        s.serialize_entry("x", &self.x)?;
        s.serialize_entry("y", &self.y)?;

        s.end()
    }
}

impl From<(i32, i32)> for Vec2Val {
    fn from(value: (i32, i32)) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}

impl From<WzVector2D> for Vec2Val {
    fn from(value: WzVector2D) -> Self {
        Self {
            x: value.x.0,
            y: value.y.0,
        }
    }
}

impl Display for Vec2Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x={},y={}", self.x, self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vex2Val(pub Vec<Vec2Val>);

impl serde::Serialize for Vex2Val {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_map(Some(3))?;
        s.serialize_entry("$type", "vex2")?;
        s.serialize_entry("vex", &self.0)?;

        s.end()
    }
}

#[derive(Debug, serde::Serialize, Clone, PartialEq)]
pub struct ObjectVal(pub Map);

impl ObjectVal {
    pub fn get(&self, index: &str) -> Option<&WzValue> {
        self.0.get(index)
    }

    pub fn must_get(&self, index: &str) -> anyhow::Result<&WzValue> {
        self.0
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Missing entry {}", index))
    }

    pub fn get_into<'a, T: TryFrom<&'a WzValue>>(
        &'a self,
        index: &str,
    ) -> Result<Option<T>, T::Error> {
        self.0.get(index).map(|v| v.try_into()).transpose()
    }

    pub fn must_get_into<'a, T: TryFrom<&'a WzValue>>(&'a self, index: &str) -> anyhow::Result<T>
    where
        T::Error: std::fmt::Debug,
    {
        self.must_get(index)?.try_into().map_err(|e| {
            anyhow::anyhow!(
                "Failed to convert {} to {}: {:?}",
                index,
                std::any::type_name::<T>(),
                e
            )
        })
    }
}

impl Index<&str> for ObjectVal {
    type Output = WzValue;

    fn index(&self, index: &str) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<&str> for ObjectVal {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.0.get_mut(index).unwrap()
    }
}

#[derive(Debug, IsVariant, Clone, PartialEq)]
pub enum WzValue {
    Object(ObjectVal),
    Null,
    F32(f32),
    F64(f64),
    Short(i16),
    Int(i32),
    Long(i64),
    String(String),
    Vec(Vec2Val),
    Convex(Vex2Val),
    Sound(SoundVal),
    Canvas(CanvasVal),
    Link(String),
}

impl From<Map> for WzValue {
    fn from(value: Map) -> Self {
        Self::Object(ObjectVal(value))
    }
}

impl WzValue {
    pub fn get_path(&self, path: &str) -> Option<&WzValue> {
        let mut cur = self;
        for part in path.split('/') {
            let cur_obj = match cur {
                WzValue::Object(v) => v,
                WzValue::Canvas(v) => {
                    // We get the next object from the canvas If there's one
                    if let Some(WzValue::Object(v)) = v.sub.as_deref() {
                        v
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };

            if let Some(v) = cur_obj.0.get(part) {
                cur = v;
            } else {
                return None;
            }
        }

        Some(cur)
    }

    pub fn as_object(&self) -> Option<&ObjectVal> {
        match self {
            WzValue::Object(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            WzValue::F32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            WzValue::F64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i16(&self) -> Option<i16> {
        match self {
            WzValue::Short(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            WzValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match self {
            WzValue::Int(v) => Some(*v as u32),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            WzValue::Long(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            WzValue::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_vec(&self) -> Option<&Vec2Val> {
        match self {
            WzValue::Vec(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_convex(&self) -> Option<&Vex2Val> {
        match self {
            WzValue::Convex(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_sound(&self) -> Option<&SoundVal> {
        match self {
            WzValue::Sound(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_canvas(&self) -> Option<&CanvasVal> {
        match self {
            WzValue::Canvas(v) => Some(v),
            _ => None,
        }
    }
}

macro_rules! try_into_val {
    ($ty:ty, $into_fn:ident) => {
        impl TryFrom<&WzValue> for $ty {
            type Error = anyhow::Error;

            fn try_from(v: &WzValue) -> Result<$ty, Self::Error> {
                v.$into_fn()
                    .ok_or_else(|| anyhow::anyhow!("Expected {}, got {:?}", stringify!($ty), v))
            }
        }
    };
    (ref, $ty:ty, $into_fn:ident) => {
        impl<'a> TryFrom<&'a WzValue> for &'a $ty {
            type Error = anyhow::Error;

            fn try_from(v: &'a WzValue) -> Result<&'a $ty, Self::Error> {
                v.$into_fn()
                    .ok_or_else(|| anyhow::anyhow!("Expected {}, got {:?}", stringify!($ty), v))
            }
        }
    };
}

try_into_val!(f32, as_f32);
try_into_val!(f64, as_f64);
try_into_val!(i16, as_i16);
try_into_val!(i32, as_i32);
try_into_val!(u32, as_u32);
try_into_val!(i64, as_i64);

try_into_val!(ref, Vec2Val, as_vec);
try_into_val!(ref, str, as_string);
try_into_val!(ref, ObjectVal, as_object);
try_into_val!(ref, SoundVal, as_sound);
try_into_val!(ref, Vex2Val, as_convex);

impl TryFrom<&WzValue> for bool {
    type Error = anyhow::Error;

    fn try_from(v: &WzValue) -> Result<bool, Self::Error> {
        match v {
            WzValue::Int(v) => Ok(*v != 0),
            _ => Err(anyhow::anyhow!("Expected bool, got {:?}", v)),
        }
    }
}

impl WzValue {
    pub fn read<R: WzIO>(r: &mut WzImgReader<R>) -> anyhow::Result<WzValue> {
        let obj = r.read_root_obj()?;
        Self::read_obj(r, &obj)
    }

    fn read_val<R: WzIO>(r: &mut WzImgReader<R>, val: &WzPropValue) -> anyhow::Result<WzValue> {
        Ok(match val {
            WzPropValue::Null => WzValue::Null,
            WzPropValue::Short1(v) | WzPropValue::Short2(v) => WzValue::Short(*v),
            WzPropValue::Int1(v) | WzPropValue::Int2(v) => WzValue::Int(v.0),
            WzPropValue::Long(v) => WzValue::Long(v.0),
            WzPropValue::F32(v) => WzValue::F32(v.0),
            WzPropValue::F64(v) => WzValue::F64(*v),
            WzPropValue::Str(v) => WzValue::String(v.0.to_string()),
            WzPropValue::Obj(v) => Self::read_obj(r, &v.obj)?,
        })
    }

    fn read_prop<R: WzIO>(r: &mut WzImgReader<R>, prop: &WzProperty) -> anyhow::Result<WzValue> {
        let mut map = Map::new();
        for entry in prop.entries.0.iter() {
            map.insert(entry.name.0.to_string(), Self::read_val(r, &entry.val)?);
        }
        Ok(WzValue::Object(ObjectVal(map)))
    }

    fn read_obj<R: WzIO>(r: &mut WzImgReader<R>, obj: &WzObject) -> anyhow::Result<WzValue> {
        Ok(match obj {
            WzObject::Property(prop) => Self::read_prop(r, &prop)?,
            WzObject::Canvas(canvas) => {
                let prop = if let Some(prop) = canvas.property.as_ref() {
                    Some(Box::new(Self::read_prop(r, prop)?))
                } else {
                    None
                };
                WzValue::Canvas(CanvasVal {
                    canvas: canvas.clone(),
                    sub: prop,
                })
            }
            WzObject::UOL(link) => WzValue::Link(link.entries.0.to_string()),
            WzObject::Vec2(vec2) => WzValue::Vec(vec2.clone().into()),
            WzObject::Convex2D(vex) => {
                WzValue::Convex(Vex2Val(vex.0.iter().map(|v| Vec2Val::from(*v)).collect()))
            }
            WzObject::SoundDX8(sound) => WzValue::Sound(SoundVal {
                sound: sound.clone(),
            }),
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct WzValueLink {
    #[serde(rename = "$type")]
    pub ty: &'static str,
    #[serde(rename = "$link")]
    pub link: String,
}

impl serde::Serialize for WzValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            WzValue::Object(v) => v.serialize(serializer),
            WzValue::Null => serializer.serialize_none(),
            WzValue::F32(v) => serializer.serialize_f32(*v),
            WzValue::F64(v) => serializer.serialize_f64(*v),
            WzValue::Short(v) => serializer.serialize_i16(*v),
            WzValue::Int(v) => serializer.serialize_i32(*v),
            WzValue::Long(v) => serializer.serialize_i64(*v),
            WzValue::String(v) => serializer.serialize_str(v),
            WzValue::Vec(v) => v.serialize(serializer),
            WzValue::Convex(v) => v.serialize(serializer),
            WzValue::Sound(_v) => serializer.serialize_str("SOUND"),
            WzValue::Canvas(v) => v.serialize(serializer),
            WzValue::Link(v) => WzValueLink {
                ty: "link",
                link: v.to_string(),
            }
            .serialize(serializer),
        }
    }
}

fn visit_vec2<'de, A>(vis: &WzValueVisitor, mut map: A) -> Result<Vec2Val, A::Error>
where
    A: serde::de::MapAccess<'de>,
{
    let x = map
        .next_key::<&str>()?
        .ok_or_else(|| serde::de::Error::invalid_length(0, vis))?;
    if x != "x" {
        return Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(x),
            &"x",
        ));
    }
    let x = map.next_value::<i32>()?;
    let y_key = map
        .next_key::<&str>()?
        .ok_or_else(|| serde::de::Error::invalid_length(0, vis))?;
    if y_key != "y" {
        return Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(y_key),
            &"y",
        ));
    }
    let y = map.next_value::<i32>()?;
    return Ok((x, y).into());
}

struct WzValueVisitor;

impl<'de> serde::de::Visitor<'de> for WzValueVisitor {
    type Value = WzValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a WzValue")
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(WzValue::Int(if v { 1 } else { 0 }))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(WzValue::Long(v))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(WzValue::Long(v as i64))
    }

    fn visit_i32<E: serde::de::Error>(self, v: i32) -> Result<Self::Value, E> {
        Ok(WzValue::Int(v))
    }

    fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<Self::Value, E> {
        Ok(WzValue::Int(v as i32))
    }

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v as i64)
    }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_i64(v as i64)
    }

    fn visit_i128<E>(self, _v: i128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other("i128"),
            &self,
        ))
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_u64(v as u64)
    }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_u64(v as u64)
    }

    fn visit_u128<E>(self, _v: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other("u128"),
            &self,
        ))
    }

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(WzValue::F32(v))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(WzValue::F64(v))
    }

    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(v.encode_utf8(&mut [0u8; 4]))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(WzValue::String(v.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(WzValue::String(v))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Bytes(v),
            &self,
        ))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(WzValue::Null)
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let _ = seq;
        Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Seq,
            &self,
        ))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let ty = map
            .next_key::<&str>()?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;

        if ty == "$type" {
            let ty_val = map.next_value::<String>()?;

            if ty_val == "link" {
                let _ = map.next_key::<&str>()?;
                let link = map.next_value::<String>()?;
                return Ok(WzValue::Link(link));
            }

            if ty_val == "vec2" {
                return Ok(WzValue::Vec(visit_vec2(&self, map)?));
            }

            if ty_val == "vex2" {
                let _ = map.next_key::<&str>()?;
                //let vex = map.next_value::<Vec<Vec2Val>>()?;
                //return Ok(WzValue::Convex(Vex2Val(vex)));
                todo!()
            }

            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Other(&ty_val),
                &"custom type",
            ));
        }

        let mut m = Map::new();
        m.insert(ty.to_string(), map.next_value()?);
        while let Some((k, v)) = map.next_entry::<String, WzValue>()? {
            m.insert(k, v);
        }
        return Ok(WzValue::Object(ObjectVal(m)));
    }
}

impl<'de> serde::Deserialize<'de> for WzValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WzValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::indexmap;

    use super::*;

    fn check_val(val: WzValue) {
        let json_str = serde_json::to_string(&val).unwrap();
        dbg!(&json_str);
        let cmp: WzValue = serde_json::from_str(&json_str).unwrap();
        assert_eq!(val, cmp);
    }

    #[test]
    fn link() {
        check_val(WzValue::from(indexmap! {
            "mylink".to_string() => WzValue::Link("Link".to_string())
        }));

        check_val(WzValue::Vec((-1, 1).into()));
    }
}
