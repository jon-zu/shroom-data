use std::io::{Read, Seek};

use binrw::{binrw, BinRead, BinWrite};
use derive_more::Unwrap;

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    ty::{WzF32, WzInt, WzLong, WzVec},
};

use super::{
    obj::WzObject,
    str::{WzImgStr, WzTypeStr},
};

#[derive(Debug, Clone)]
pub struct WzObjectValue {
    pub len: u32,
    pub obj: Box<WzObject>,
}

impl BinRead for WzObjectValue {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let len = u32::read_options(reader, endian, ())? as u64;
        let pos = reader.stream_position()?;

        // TODO sub reader
        let obj = Box::new(WzObject::read_options(reader, endian, args)?);

        // We don't read canvas/sound so we need to skip
        let after = pos + len as u64;
        reader.seek(std::io::SeekFrom::Start(after))?;

        Ok(Self {
            len: len as u32,
            obj,
        })
    }
}

impl BinWrite for WzObjectValue {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        let pos = writer.stream_position()?;
        self.len.write_le(writer)?;
        self.obj.write_options(writer, endian, args)?;
        let end = writer.stream_position()?;
        let len = end - pos - 4;

        writer.seek(std::io::SeekFrom::Start(pos))?;
        (len as u32).write_le(writer)?;
        writer.seek(std::io::SeekFrom::Start(end))?;

        Ok(())
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone, Unwrap)]
pub enum WzPropValue {
    #[brw(magic(0u8))]
    Null,

    // Short
    #[brw(magic(2u8))]
    Short1(i16),
    #[brw(magic(11u8))]
    Short2(i16),

    // Int
    #[brw(magic(3u8))]
    Int1(WzInt),
    #[brw(magic(19u8))]
    Int2(WzInt),

    // Long
    #[brw(magic(20u8))]
    Long(WzLong),

    // Floats
    #[brw(magic(4u8))]
    F32(WzF32),
    #[brw(magic(5u8))]
    F64(f64),

    #[brw(magic(8u8))]
    Str(#[brw(args_raw(ctx))] WzImgStr),

    #[brw(magic(9u8))]
    Obj(#[brw(args_raw(ctx))] WzObjectValue),
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzPropertyEntry {
    #[brw(args_raw(ctx))]
    pub name: WzImgStr,
    #[brw(args_raw(ctx))]
    pub val: WzPropValue,
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzProperty {
    pub unknown: u16,
    #[brw(args_raw(ctx))]
    pub entries: WzVec<WzPropertyEntry>,
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzUOL {
    pub unknown: u8,
    #[brw(args_raw(ctx))]
    pub entries: WzImgStr,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy)]
pub struct WzVector2D {
    pub x: WzInt,
    pub y: WzInt,
}

#[derive(Debug, Clone)]
pub struct WzConvex2D(pub Vec<WzVector2D>);

impl BinRead for WzConvex2D {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let len = WzInt::read_le(reader)?.0 as usize;
        let mut v = Vec::with_capacity(len);

        for _ in 0..len {
            let _ = WzTypeStr::read_le_args(reader, args)?;
            // TODO: ensure uol str is Vec2
            v.push(WzVector2D::read_le(reader)?);
        }

        Ok(Self(v))
    }
}

impl BinWrite for WzConvex2D {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        WzInt(self.0.len() as i32).write_le(writer)?;
        for v in self.0.iter() {
            WzObject::Vec2(*v).write_le_args(writer, args)?;
        }
        Ok(())
    }
}
