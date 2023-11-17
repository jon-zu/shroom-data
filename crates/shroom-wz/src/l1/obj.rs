use binrw::{BinRead, BinWrite};
use derive_more::Unwrap;

use crate::ctx::{WzImgReadCtx, WzImgWriteCtx};

use super::{
    canvas::WzCanvas,
    prop::{WzConvex2D, WzProperty, WzUOL, WzVector2D},
    sound::WzSound,
    str::WzTypeStr,
};

#[derive(Debug, Unwrap, Clone)]
pub enum WzObject {
    Property(WzProperty),
    Canvas(WzCanvas),
    UOL(WzUOL),
    Vec2(WzVector2D),
    Convex2D(WzConvex2D),
    SoundDX8(WzSound),
}

pub const OBJ_TYPE_PROPERTY: &[u8] = b"Property";
pub const OBJ_TYPE_CANVAS: &[u8] = b"Canvas";
pub const OBJ_TYPE_UOL: &[u8] = b"UOL";
pub const OBJ_TYPE_VEC2: &[u8] = b"Shape2D#Vector2D";
pub const OBJ_TYPE_CONVEX2D: &[u8] = b"Shape2D#Convex2D";
pub const OBJ_TYPE_SOUND_DX8: &[u8] = b"Sound_DX8";

impl BinRead for WzObject {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let ty_name = WzTypeStr::read_options(reader, endian, args)?;

        Ok(match ty_name.0.as_bytes() {
            OBJ_TYPE_PROPERTY => Self::Property(WzProperty::read_options(reader, endian, args)?),
            OBJ_TYPE_CANVAS => Self::Canvas(WzCanvas::read_options(reader, endian, args)?),
            OBJ_TYPE_UOL => Self::UOL(WzUOL::read_options(reader, endian, args)?),
            OBJ_TYPE_VEC2 => Self::Vec2(WzVector2D::read_options(reader, endian, ())?),
            OBJ_TYPE_CONVEX2D => Self::Convex2D(WzConvex2D::read_options(reader, endian, args)?),
            OBJ_TYPE_SOUND_DX8 => Self::SoundDX8(WzSound::read_options(reader, endian, args)?),
            _ => {
                return Err(binrw::Error::Custom {
                    pos: reader.stream_position().unwrap_or(0),
                    err: Box::new(anyhow::format_err!("Invalid obj: {ty_name:?}")),
                })
            }
        })
    }
}

pub fn wz_ty_str(s: &[u8]) -> WzTypeStr {
    WzTypeStr::new(String::from_utf8(s.to_vec()).unwrap())
}

impl BinWrite for WzObject {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        match self {
            WzObject::Property(v) => {
                wz_ty_str(OBJ_TYPE_PROPERTY).write_le_args(writer, args)?;
                v.write_options(writer, endian, args)
            }
            WzObject::Canvas(v) => {
                wz_ty_str(OBJ_TYPE_CANVAS).write_le_args(writer, args)?;
                v.write_options(writer, endian, args)
            }
            WzObject::UOL(v) => {
                wz_ty_str(OBJ_TYPE_UOL).write_le_args(writer, args)?;
                v.write_options(writer, endian, args)
            }
            WzObject::Vec2(v) => {
                wz_ty_str(OBJ_TYPE_VEC2).write_le_args(writer, args)?;
                v.write_options(writer, endian, ())
            }
            WzObject::Convex2D(v) => {
                wz_ty_str(OBJ_TYPE_CONVEX2D).write_le_args(writer, args)?;
                v.write_options(writer, endian, args)
            }
            WzObject::SoundDX8(v) => {
                wz_ty_str(OBJ_TYPE_SOUND_DX8).write_le_args(writer, args)?;
                v.write_options(writer, endian, args)
            }
        }
    }
}

//pub struct WzWritePropertyObj
