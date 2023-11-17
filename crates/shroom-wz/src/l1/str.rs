use std::rc::Rc;

use binrw::{BinRead, BinWrite};

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    ty::WzStr,
};

#[derive(Debug, Clone)]
pub struct WzTypeStr(pub Rc<WzStr>);

impl WzTypeStr {
    pub fn new(s: String) -> Self {
        Self(Rc::new(WzStr(s)))
    }
}

impl BinRead for WzTypeStr {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let magic = u8::read_options(reader, endian, ())?;

        Ok(Self(match magic {
            0x73 => args.read_str(reader)?,
            0x1B => {
                let v = u32::read_options(reader, endian, ())?;
                args.get_str(v).map_err(|e| binrw::Error::Custom {
                    pos: reader.stream_position().unwrap_or(0),
                    err: Box::new(e),
                })?
            }
            _ => {
                return Err(binrw::Error::Custom {
                    pos: reader.stream_position().unwrap_or(0),
                    err: Box::new(anyhow::format_err!("Invalid type str magic: {:#x}", magic)),
                })
            }
        }))
    }
}

impl BinWrite for WzTypeStr {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        if let Some(offset) = args.str_table.get(self.0.as_str()) {
            (0x73u8).write_options(writer, endian, ())?;
            offset.write_options(writer, endian, ())
        } else {
            (0x1Bu8).write_options(writer, endian, ())?;
            self.0.write_options(writer, endian, args.into())
        }
    }
}

#[derive(Debug, Clone)]
pub struct WzImgStr(pub Rc<WzStr>);

impl WzImgStr {
    pub fn new(s: String) -> Self {
        Self(Rc::new(WzStr(s)))
    }
}

impl BinRead for WzImgStr {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let magic = u8::read_options(reader, endian, ())?;

        Ok(Self(match magic {
            0 => args.read_str(reader)?,
            1 => {
                let v = u32::read_options(reader, endian, ())?;
                args.get_str(v).map_err(|e| binrw::Error::Custom {
                    pos: reader.stream_position().unwrap_or(0),
                    err: Box::new(e),
                })?
            }
            _ => {
                return Err(binrw::Error::Custom {
                    pos: reader.stream_position().unwrap_or(0),
                    err: Box::new(anyhow::format_err!("Invalid str magic: {:#x}", magic)),
                })
            }
        }))
    }
}

impl BinWrite for WzImgStr {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        if let Some(offset) = args.str_table.get(self.0.as_str()) {
            (1u8).write_options(writer, endian, ())?;
            offset.write_options(writer, endian, ())
        } else {
            (0u8).write_options(writer, endian, ())?;
            self.0.write_options(writer, endian, args.into())
        }
    }
}
