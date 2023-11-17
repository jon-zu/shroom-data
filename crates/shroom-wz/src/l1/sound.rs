use std::io::{Cursor, Seek};

use binrw::{binrw, BinRead, BinReaderExt, BinWrite, PosValue};
use uuid::uuid;

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    ty::WzInt,
    util::custom_binrw_error,
};

// TODO verify paddings

const WAVE_HEADER_SIZE: usize = 18;
const PCM_HEADER_SIZE: usize = 44;

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_MP3: u16 = 0x0055;

const MEDIA_TYPE_STREAM: uuid::Uuid = uuid!("E436EB83-524F-11CE-9F53-0020AF0BA770");
const MEDIA_SUBTYPE_MPEG1_PACKET: uuid::Uuid = uuid!("e436eb87-524f-11ce-9f53-0020af0ba770");
const MEDIA_SUBTYPE_WAVE: uuid::Uuid = uuid!("E436EB8B-524F-11CE-9F53-0020AF0BA770");
//const MEDIA_SUBTYPE_WAVE_EX: uuid::Uuid = uuid!("05589f81-c356-11ce-bf01-00aa0055595a");

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct GUID(
    #[br(map = uuid::Uuid::from_bytes_le)]
    #[bw(map = uuid::Uuid::to_bytes_le)]
    pub uuid::Uuid,
);

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct MediaHeader {
    pub unknown1: u8,
    pub major_type: GUID,
    pub sub_type: GUID,
    pub sample_size: u16,
    pub format_type: GUID,
}

#[derive(Debug, Clone)]
pub enum SoundFormat {
    Mpeg1([u8; 73]),
    Mpeg3(Mpeg3WaveHeader),
    Pcm(WaveHeader),
}

#[derive(Debug, Clone)]
pub struct SoundHeader {
    pub media_header: MediaHeader,
    pub fmt: SoundFormat,
}

impl BinRead for SoundHeader {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let media_header: MediaHeader = reader.read_le()?;
        let major = media_header.major_type.0;
        if major != MEDIA_TYPE_STREAM {
            return Err(custom_binrw_error(
                reader,
                anyhow::format_err!("Invalid sound major type: {major}"),
            ));
        }

        let mut hdr = [0u8; u8::MAX as usize];
        let hdr_len = u8::read_le(reader)? as usize;
        let hdr = &mut hdr[..hdr_len];
        reader.read_exact(hdr)?;

        let sub = media_header.sub_type.0;
        Ok(match sub {
            MEDIA_SUBTYPE_MPEG1_PACKET => Self {
                media_header,
                fmt: SoundFormat::Mpeg1(hdr.try_into().unwrap()), // TODO check properly
            },
            MEDIA_SUBTYPE_WAVE => {
                let mut sub = Cursor::new(hdr);
                let wave: WaveHeader = sub.read_le()?;
                sub.rewind().unwrap();

                let fmt = match wave.format {
                    WAVE_FORMAT_PCM => SoundFormat::Pcm(wave),
                    WAVE_FORMAT_MP3 => SoundFormat::Mpeg3(sub.read_le()?),
                    n => todo!("Invalid wave format: {n}"),
                };
                Self { media_header, fmt }
            }
            _ => {
                return Err(custom_binrw_error(
                    reader,
                    anyhow::format_err!("Unknown sound sub type: {major}"),
                ))
            }
        })
    }
}

impl BinWrite for SoundHeader {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        todo!()
    }
}

// See WAVEFORMATEX
// https://learn.microsoft.com/en-us/windows/win32/api/mmeapi/ns-mmeapi-waveformatex
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct WaveHeader {
    pub format: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    // Align tail, struct is not packed
    #[bw(pad_size_to = 4)]
    pub extra_size: u16,
}

impl WaveHeader {
    pub fn is_valid_header_size(&self, header_size: usize) -> bool {
        WAVE_HEADER_SIZE + (self.extra_size as usize) == header_size
    }
}

// see MPEGLAYER3WAVEFORMAT
// https://learn.microsoft.com/en-us/windows/win32/api/mmreg/ns-mmreg-mpeglayer3waveformat
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct Mpeg3WaveHeader {
    pub wav: WaveHeader,
    #[bw(pad_size_to = 4)]
    pub id: u16,
    pub flags: u32,
    pub block_size: u16,
    pub frames_per_block: u16,
    #[bw(pad_size_to = 4)]
    pub codec_delay: u16,
}

//PCMWAVEFORMAT
#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct PcmWaveFormat {
    pub wav: WaveHeader,
    #[bw(pad_size_to = 4)]
    pub bit_per_sample: u16,
}

/*
#[derive(Debug, Clone)]
pub enum SoundFormat {
    Mpeg3(Mpeg3WaveHeader),
    Mpeg1([u8; 73]),
    Pcm(WaveHeader),
}

impl BinRead for SoundFormat {
    type Args<'a> = WzContext<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let hdr_len = u8::read_le(reader)? as usize;

        // Header is atmost u8::MAX, read the header
        let mut buf = [0; u8::MAX as usize];
        let hdr_buf = &mut buf[..hdr_len];
        reader.read_exact(hdr_buf)?;

        // Build reader
        let mut wave: WaveHeader = Cursor::new(&hdr_buf).read_le()?;

        if wave.format == 0x3344 {
            return Ok(Self::Mpeg1(hdr_buf.try_into().unwrap()));
        }

        // Check if wave header looks valid, else wise try to decode
        if !wave.is_valid_header_size(hdr_len) {
            for _ in 0..5 {
                args.crypto.transform(hdr_buf.into());
                log::info!("hdr: {:?} - {hdr_len}", wave);
                wave = Cursor::new(&hdr_buf).read_le()?;

                log::info!("hdr: {:?} - {hdr_len}", wave);
                if wave.is_valid_header_size(hdr_len) {
                    break;
                }
            }

            if !wave.is_valid_header_size(hdr_len) {
                todo!("Invalid header size")
            }
        }

        // We got our wave header now check the extra data
        Ok(match wave.format {
            WAVE_FORMAT_PCM => SoundFormat::Pcm(wave),
            WAVE_FORMAT_MP3 => Self::Mpeg3(Cursor::new(&hdr_buf).read_le()?),
            n => todo!("Invalid wave format: {n}"),
        })
    }
}

impl BinWrite for SoundFormat {
    type Args<'a> = WzContext<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Encode it
        match self {
            Self::Mpeg3(mp3) => {
                (WAVE_HEADER_SIZE as u8 + mp3.wav.extra_size as u8).write_le(writer)?;
                mp3.write_args(writer, ())
            }
            Self::Pcm(wave) => {
                (WAVE_HEADER_SIZE as u8 + wave.extra_size as u8).write_le(writer)?;
                wave.write_args(writer, ())
            }
            _ => unimplemented!(),
        }
    }
}*/

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzSound {
    pub unknown: u8,
    pub size: WzInt,
    pub len_ms: WzInt,
    #[brw(args_raw = ctx)]
    pub header: SoundHeader,
    #[bw(ignore)]
    pub offset: PosValue<()>,
}

impl WzSound {
    pub fn data_size(&self) -> usize {
        let extra = match self.header.fmt {
            SoundFormat::Mpeg3(_) => 0,
            SoundFormat::Pcm(_) => PCM_HEADER_SIZE,
            SoundFormat::Mpeg1(_) => 0,
        };
        (self.size.0 as usize) + extra
    }
}
