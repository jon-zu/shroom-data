use std::{
    cell::RefCell,
    collections::HashMap,
    io::{Read, Seek},
    rc::Rc,
};

use binrw::{BinRead, BinResult};

use crate::{crypto::WzCrypto, ty::WzStr};

#[derive(Debug, Default)]
pub struct WzStrTable(RefCell<HashMap<u32, Rc<WzStr>>>);

impl WzStrTable {
    pub fn get(&self, offset: &u32) -> Option<Rc<WzStr>> {
        self.0.borrow().get(offset).cloned()
    }

    pub fn must_get(&self, offset: &u32) -> anyhow::Result<Rc<WzStr>> {
        self.get(offset)
            .ok_or_else(|| anyhow::anyhow!("Missing string at offset {:#x}", offset))
    }

    pub fn insert(&self, offset: u32, s: Rc<WzStr>) {
        self.0.borrow_mut().insert(offset, s);
    }
}

#[derive(Debug, Default)]
pub struct WzStrWriteTable(RefCell<HashMap<String, u32>>);

impl WzStrWriteTable {
    pub fn get(&self, s: &str) -> Option<u32> {
        self.0.borrow().get(s).copied()
    }

    pub fn insert(&self, s: String, offset: u32) {
        self.0.borrow_mut().insert(s, offset);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WzContext<'a>(pub &'a WzCrypto);

#[derive(Debug, Clone, Copy)]
pub struct WzImgReadCtx<'a> {
    pub crypto: &'a WzCrypto,
    pub str_table: &'a WzStrTable,
}

#[derive(Debug, Clone, Copy)]
pub struct WzImgWriteCtx<'a> {
    pub crypto: &'a WzCrypto,
    pub str_table: &'a WzStrWriteTable,
}

impl<'a> WzContext<'a> {
    pub fn new(crypto: &'a WzCrypto) -> Self {
        Self(crypto)
    }
}

impl<'a> From<&WzImgReadCtx<'a>> for WzContext<'a> {
    fn from(ctx: &WzImgReadCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<&WzImgWriteCtx<'a>> for WzContext<'a> {
    fn from(ctx: &WzImgWriteCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<WzImgReadCtx<'a>> for WzContext<'a> {
    fn from(ctx: WzImgReadCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<WzImgWriteCtx<'a>> for WzContext<'a> {
    fn from(ctx: WzImgWriteCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> WzImgReadCtx<'a> {
    pub fn new(crypto: &'a WzCrypto, str_table: &'a WzStrTable) -> Self {
        Self { crypto, str_table }
    }

    pub fn get_str(&self, offset: u32) -> anyhow::Result<Rc<WzStr>> {
        self.str_table.must_get(&offset)
    }

    pub fn read_str<R: Read + Seek>(&self, mut r: R) -> BinResult<Rc<WzStr>> {
        let offset = r.stream_position()? as u32;
        let str = Rc::new(WzStr::read_le_args(&mut r, self.into())?);
        self.str_table.insert(offset, str.clone());
        Ok(str)
    }
}

impl<'a> WzImgWriteCtx<'a> {
    pub fn new(crypto: &'a WzCrypto, str_table: &'a WzStrWriteTable) -> Self {
        Self { crypto, str_table }
    }
}
