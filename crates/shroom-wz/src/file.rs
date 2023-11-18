use std::{
    collections::VecDeque,
    fs::File,
    io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};

use anyhow::Context;
use binrw::BinRead;

use crate::{
    canvas::Canvas,
    crypto::WzCrypto,
    ctx::{WzContext, WzImgReadCtx, WzStrTable},
    l0::{WzDir, WzDirHeader, WzDirNode, WzHeader, WzImgHeader},
    l1::{
        canvas::WzCanvas, obj::WzObject, prop::WzPropValue, ser::WzImgSerializer, sound::WzSound,
    },
    ty::WzOffset,
    util::{BufReadExt, PeekExt, SubReader},
    WzConfig,
};
pub trait WzIO: BufRead + Seek {}
impl<T> WzIO for T where T: BufRead + Seek {}

pub struct WzImgReader<R> {
    r: R,
    crypto: Arc<WzCrypto>,
    str_table: WzStrTable,
}

impl<R> WzImgReader<R>
where
    R: WzIO,
{
    pub fn new(r: R, crypto: Arc<WzCrypto>) -> Self {
        Self {
            r,
            crypto,
            str_table: Default::default(),
        }
    }

    pub fn ctx(&self) -> WzImgReadCtx<'_> {
        WzImgReadCtx::new(&self.crypto, &self.str_table)
    }

    /// Read the root object for that image
    pub fn read_root_obj(&mut self) -> anyhow::Result<WzObject> {
        self.r.rewind()?;
        Ok(WzObject::read_le_args(
            &mut self.r,
            WzImgReadCtx::new(&self.crypto, &self.str_table),
        ).context("Root")?)
    }

    /// Read an object with the given object header
    /*pub fn read_obj(&mut self, obj: &WzObj) -> anyhow::Result<WzObject> {
        // Check for root
        let ix = if obj.len.pos == 0 && obj.len.val == 0 {
            0
        } else {
            obj.len.pos + 4
        };

        // Skip first index
        self.r.seek(SeekFrom::Start(ix))?;
        Ok(WzObject::read_le_args(
            &mut self.r,
            WzImgReadCtx::new(&self.crypto, &self.str_table),
        )?)
    }*/

    fn read_canvas_from<T: BufRead>(mut r: T, canvas: &WzCanvas) -> anyhow::Result<Canvas> {
        let sz = canvas.raw_bitmap_size() as usize;
        let mut img_buf = Vec::with_capacity(sz);
        r.decompress_flate_size(&mut img_buf, sz)?;
        Ok(Canvas::from_data(img_buf, canvas))
    }

    pub fn read_canvas(&mut self, canvas: &WzCanvas) -> anyhow::Result<Canvas> {
        let len = canvas.data_len();
        let off = canvas.data_offset();
        self.r.seek(SeekFrom::Start(off))?;

        let hdr = self.r.peek_u16()?;
        // 5th bit => 3rd bit from the end -> 16-13
        let is_zlib = (hdr & 0xFF) == 0x78;
        let with_preset = hdr & (1 << 13) != 0;
        // For some reason the is_preset flag is used for chunked encoding
        if is_zlib && !with_preset {
            let mut sub = (&mut self.r).take(len as u64);
            Self::read_canvas_from(&mut sub, canvas)
        } else {
            let buf = self.r.read_chunked_data(&self.crypto, len)?;
            Self::read_canvas_from(Cursor::new(buf), canvas)
        }
    }

    pub fn read_sound(&mut self, sound: &WzSound) -> anyhow::Result<Vec<u8>> {
        let ln = sound.data_size();
        self.r.seek(SeekFrom::Start(sound.offset.pos))?;
        let mut data = vec![0; ln];
        self.r.read_exact(&mut data)?;

        Ok(data)
    }

    pub fn read_path<'obj>(
        &mut self,
        root: &'obj WzObject,
        path: &str,
    ) -> anyhow::Result<&'obj WzObject> {
        let mut cur = root;

        for part in path.split('/') {
            let WzObject::Property(ref prop) = cur else {
                anyhow::bail!("Invalid prop: {cur:?}");
            };

            let next = prop
                .entries
                .0
                .iter()
                .find(|x| x.name.0.as_str() == part)
                .ok_or_else(|| anyhow::format_err!("Invalid {path}"))?;

            let obj = match &next.val {
                WzPropValue::Obj(ref obj) => obj,
                _ => anyhow::bail!("Invalid obj: {cur:?}"),
            };
            cur = &obj.obj;
        }

        Ok(cur)
    }

    pub fn into_serializer(self, skip_canvas: bool) -> anyhow::Result<WzImgSerializer<R>> {
        WzImgSerializer::new(self, skip_canvas)
    }
}

#[derive(Debug, Clone)]
pub struct WzReader<R> {
    inner: R,
    data_offset: u64,
    crypto: Arc<WzCrypto>,
}

pub type SubWzReader<'a, R> = WzReader<SubReader<'a, R>>;
pub type WzReaderFile = WzReader<BufReader<File>>;

impl WzReaderFile {
    pub fn open_file(path: impl AsRef<Path>, cfg: WzConfig) -> anyhow::Result<Self> {
        Self::open(BufReader::new(File::open(path)?), cfg)
    }
}

impl<R> WzReader<R>
where
    R: WzIO,
{
    pub fn open(mut rdr: R, cfg: WzConfig) -> anyhow::Result<Self> {
        let hdr = WzHeader::read_le(&mut rdr)?;
        rdr.seek(SeekFrom::Start(hdr.data_offset as u64))?;

        let encrypted_version = u16::read_le(&mut rdr)?;
        let ver = cfg.version;
        if ver.encrypted_version() != encrypted_version {
            anyhow::bail!("Wrong version: {}, expected: {ver:?}", encrypted_version);
        }

        Ok(Self::new(rdr, cfg, hdr.data_offset as u64))
    }

    pub fn open_img(rdr: R, cfg: WzConfig) -> Self {
        Self::new(rdr, cfg, 0)
    }

    fn new(rdr: R, cfg: WzConfig, data_offset: u64) -> Self {
        Self {
            inner: rdr,
            crypto: WzCrypto::from_cfg(cfg, data_offset as u32).into(),
            data_offset,
        }
    }

    fn sub_reader(&mut self, offset: u64, size: u64) -> SubReader<'_, R> {
        SubReader::new(&mut self.inner, offset, size)
    }

    pub fn root_offset(&self) -> WzOffset {
        WzOffset(self.data_offset as u32 + 2)
    }

    pub fn read_root_dir(&mut self) -> anyhow::Result<WzDir> {
        // Skip encrypted version at the start
        self.read_dir(self.root_offset().0 as u64)
    }

    pub fn read_dir_node(&mut self, hdr: &WzDirHeader) -> anyhow::Result<WzDir> {
        self.read_dir(hdr.offset.0 as u64)
    }

    fn read_dir(&mut self, offset: u64) -> anyhow::Result<WzDir> {
        self.set_pos(offset)?;
        Ok(WzDir::read_le_args(
            &mut self.inner,
            WzContext::new(&self.crypto),
        )?)
    }

    pub fn root_img_reader(&mut self) -> io::Result<WzImgReader<SubReader<'_, R>>> {
        // Get size by seeking to end
        let end = self.inner.seek(SeekFrom::End(0))?;
        let off = 0;
        self.set_pos(off)?;
        let crypto = self.crypto.clone();

        Ok(WzImgReader::new(self.sub_reader(off, end), crypto))
    }

    pub fn img_reader(&mut self, hdr: &WzImgHeader) -> io::Result<WzImgReader<SubReader<'_, R>>> {
        let off = hdr.offset.into();
        self.set_pos(off)?;
        let crypto = self.crypto.clone();

        Ok(WzImgReader::new(
            self.sub_reader(off, hdr.blob_size.0 as u64),
            crypto,
        ))
    }

    pub fn checksum(&mut self, offset: u64, ln: u64) -> anyhow::Result<i32> {
        let old = self.inner.stream_position()?;
        self.set_pos(offset)?;
        let checksum = self.inner.wz_checksum(ln)?;
        self.set_pos(old)?;
        Ok(checksum)
    }
    /*
        pub fn link_img_reader(
            &mut self,
            hdr: &WzLinkData,
        ) -> io::Result<WzImgReader<SubReader<'_, R>>> {
            let off = hdr.offset.into();
            self.set_pos(off)?;
            let crypto = self.crypto.clone();
            let str_table = self.str_table.clone();

            Ok(WzImgReader {
                r: self.sub_reader(off, hdr..0 as u64),
                crypto,
                str_table,
            })
        }
    */
    pub fn traverse_images(&mut self) -> WzImgTraverser<'_, R> {
        let mut q = VecDeque::new();
        q.push_back((
            Arc::new("".to_string()),
            WzDirNode::Dir(WzDirHeader::root("root", 1, self.root_offset())),
        ));
        WzImgTraverser { r: self, q }
    }

    pub fn read_path(&mut self, root: &WzDirNode, path: &str) -> anyhow::Result<WzDirNode> {
        let mut cur = root.clone();

        for part in path.split('/') {
            let WzDirNode::Dir(dir) = cur else {
                anyhow::bail!("Invalid dir: {cur:?}");
            };

            let dir = self.read_dir_node(&dir)?;
            let next = dir.get(part).ok_or_else(|| {
                anyhow::format_err!("Invalid {path}: {part} not found in {dir:?}")
            })?;
            cur = next.clone();
        }

        Ok(cur)
    }

    fn set_pos(&mut self, p: u64) -> io::Result<()> {
        self.inner.seek(SeekFrom::Start(p))?;
        Ok(())
    }
}

pub struct WzImgTraverser<'r, R> {
    r: &'r mut WzReader<R>,
    q: VecDeque<(Arc<String>, WzDirNode)>,
}

impl<'r, R: WzIO> WzImgTraverser<'r, R> {
    fn handle_dir(
        &mut self,
        root_name: &str,
        dir: &WzDirHeader,
    ) -> anyhow::Result<(Arc<String>, WzDir)> {
        let node = self.r.read_dir_node(dir)?;
        let node_name = Arc::new(format!("{}/{}", root_name, dir.name.as_str()));

        self.q.extend(
            node.entries
                .0
                .iter()
                .map(|x| (node_name.clone(), x.clone())),
        );

        Ok((node_name.clone(), node))
    }
}

impl<'r, R> Iterator for WzImgTraverser<'r, R>
where
    R: WzIO,
{
    type Item = anyhow::Result<(String, WzImgHeader)>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((root_name, node)) = self.q.pop_front() {
            match node {
                WzDirNode::Dir(dir) => {
                    if let Err(err) = self.handle_dir(root_name.as_str(), &dir) {
                        return Some(Err(err));
                    }
                }
                WzDirNode::Img(img) => {
                    let name = format!("{}/{}", root_name, img.name.as_str());
                    return Some(Ok((name, img)));
                }
                WzDirNode::Link(link) => {
                    let img = link.link.link_img;
                    let name = format!("{}/{}", root_name, img.name.as_str());
                    return Some(Ok((name, img)));
                }
                _ => {
                    continue;
                }
            }
        }

        None
    }
}

#[cfg(feature = "mmap")]
pub mod mmap {
    use std::{fs::File, io::Cursor, path::Path, sync::Arc};

    use memmap2::Mmap;

    use crate::{WzConfig, WzReader};

    #[derive(Debug, Clone)]
    pub struct SharedMmapFile(Arc<Mmap>);
    impl AsRef<[u8]> for SharedMmapFile {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    pub type WzReaderMmap = WzReader<Cursor<Mmap>>;
    pub type WzReaderSharedMmap = WzReader<Cursor<SharedMmapFile>>;

    impl WzReaderMmap {
        pub fn open_file_mmap(path: impl AsRef<Path>, cfg: WzConfig) -> anyhow::Result<Self> {
            let file = File::open(path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            Self::new_mmap(mmap, cfg)
        }

        fn new_mmap(mmap: Mmap, cfg: WzConfig) -> anyhow::Result<Self> {
            Self::open(Cursor::new(mmap), cfg)
        }
    }

    impl WzReaderSharedMmap {
        pub fn open_file_mmap_shared(
            path: impl AsRef<Path>,
            cfg: WzConfig,
        ) -> anyhow::Result<Self> {
            let file = File::open(path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            Self::new_mmap_shared(SharedMmapFile(mmap.into()), cfg)
        }

        fn new_mmap_shared(mmap: SharedMmapFile, cfg: WzConfig) -> anyhow::Result<Self> {
            Self::open(Cursor::new(mmap), cfg)
        }
    }
}
