use std::{
    io::{Read, Seek},
    num::Wrapping,
    ops::{Deref, DerefMut, Neg},
};

use binrw::{BinRead, BinWrite, VecArgs};

use crate::{crypto::WzCrypto, ctx::WzContext, util::custom_binrw_error};

pub type RefWzCrypto<'a> = (&'a WzCrypto,);

/// Compressed Int
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WzInt(pub i32);

impl Deref for WzInt {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WzInt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BinRead for WzInt {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Ok(Self(match i8::read_options(reader, endian, args)? {
            -128 => i32::read_options(reader, endian, args)?,
            flag => flag as i32,
        }))
    }
}

impl BinWrite for WzInt {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        match i8::try_from(self.0) {
            Ok(v) if v != -128 => v.write_options(writer, endian, args),
            _ => {
                (-128i8).write_options(writer, endian, args)?;
                (self.0).write_options(writer, endian, args)
            }
        }
    }
}

/// Compressed Long
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WzLong(pub i64);

impl BinRead for WzLong {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Ok(Self(match i8::read_options(reader, endian, args)? {
            -128 => i64::read_options(reader, endian, args)?,
            flag => flag as i64,
        }))
    }
}

impl BinWrite for WzLong {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        match i8::try_from(self.0) {
            Ok(v) if v != -128 => v.write_options(writer, endian, args),
            _ => {
                (-128i8).write_options(writer, endian, args)?;
                (self.0).write_options(writer, endian, args)
            }
        }
    }
}

/// Compressed float, converts value to Int value which is compressed
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WzF32(pub f32);

impl From<f32> for WzF32 {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl BinRead for WzF32 {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        Ok(Self(f32::from_bits(
            WzInt::read_options(reader, endian, args)?.0 as u32,
        )))
    }
}

impl BinWrite for WzF32 {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        WzInt(self.0.to_bits() as i32).write_options(writer, endian, args)
    }
}

// String mask helper

fn xor_mask_ascii(data: &mut [u8]) {
    let mut mask = Wrapping(0xAAu8);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

fn xor_mask_unicode(data: &mut [u16]) {
    let mut mask = Wrapping(0xAAAA);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WzStr(pub String);

impl WzStr {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Debug for WzStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::fmt::Display for WzStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for WzStr {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WzStr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BinRead for WzStr {
    type Args<'a> = WzContext<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let flag = i8::read_options(reader, endian, ())?;
        let str = if flag <= 0 {
            let ln = if flag == -128 {
                i32::read_options(reader, endian, ())? as usize
            } else {
                -flag as usize
            };

            let mut data = vec![0; ln];
            reader.read_exact(&mut data)?;
            xor_mask_ascii(&mut data);
            args.0.transform(data.as_mut_slice().into());
            encoding_rs::mem::decode_latin1(data.as_slice()).into_owned()
        } else {
            let ln = if flag == 127 {
                i32::read_options(reader, endian, ())? as usize
            } else {
                flag as usize
            };

            let mut data = vec![0u16; ln];
            reader.read_exact(bytemuck::cast_slice_mut(data.as_mut_slice()))?;
            xor_mask_unicode(&mut data);
            args.0
                .transform(bytemuck::cast_slice_mut(data.as_mut_slice()).into());

            String::from_utf16(&data).map_err(|err| custom_binrw_error(reader, err.into()))?
        };

        Ok(WzStr::new(str))
    }
}

impl BinWrite for WzStr {
    type Args<'a> = WzContext<'a>;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        let is_latin1 = encoding_rs::mem::is_str_latin1(self.0.as_str());

        if is_latin1 {
            let data = encoding_rs::mem::encode_latin1_lossy(self.0.as_str());
            let mut data = data.into_owned();
            let n = data.len();
            if n >= 128 {
                i8::MIN.write_options(writer, endian, ())?;
                (n as i32).neg().write_options(writer, endian, ())?;
            } else {
                (n as i8).neg().write_options(writer, endian, ())?;
            }

            args.0.transform(data.as_mut_slice().into());
            xor_mask_ascii(&mut data);

            data.write_options(writer, endian, ())?;
        } else {
            let mut data = self.0.encode_utf16().collect::<Vec<_>>();
            let n = data.len();
            if n >= 127 {
                i8::MAX.write_options(writer, endian, ())?;
                (n as i32).write_options(writer, endian, ())?;
            } else {
                (n as i8).write_options(writer, endian, ())?;
            }

            args.0
                .transform(bytemuck::cast_slice_mut(data.as_mut_slice()).into());
            xor_mask_unicode(&mut data);

            data.write_options(writer, endian, ())?;
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WzVec<B>(pub Vec<B>);

impl<B> BinRead for WzVec<B>
where
    B: BinRead + 'static,
    for<'a> B::Args<'a>: Clone,
{
    type Args<'a> = B::Args<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let n = WzInt::read_options(reader, endian, ())?;
        Ok(Self(Vec::read_options(
            reader,
            endian,
            VecArgs {
                count: n.0 as usize,
                inner: args,
            },
        )?))
    }
}

impl<B> BinWrite for WzVec<B>
where
    B: BinWrite + 'static,
    for<'a> B::Args<'a>: Clone,
{
    type Args<'a> = B::Args<'a>;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        WzInt(self.0.len() as i32).write_options(writer, endian, ())?;
        self.0.write_options(writer, endian, args)?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WzOffset(pub u32);

impl From<WzOffset> for u32 {
    fn from(value: WzOffset) -> Self {
        value.0
    }
}

impl From<WzOffset> for u64 {
    fn from(value: WzOffset) -> u64 {
        value.0 as u64
    }
}

impl BinRead for WzOffset {
    type Args<'a> = WzContext<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let pos = reader.stream_position()? as u32;
        let v = u32::read_options(reader, endian, ())?;
        let offset = args.0.decrypt_offset(v, pos);
        Ok(Self(offset))
    }
}

impl BinWrite for WzOffset {
    type Args<'a> = WzContext<'a>;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        let pos = writer.stream_position()? as u32;
        let enc_off = args.0.encrypt_offset(self.0, pos);
        enc_off.write_options(writer, endian, ())
    }
}
