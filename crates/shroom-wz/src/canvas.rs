use std::vec;

use image::{Rgba, RgbaImage};

use crate::l1::canvas::{WzCanvas, WzCanvasDepth, WzCanvasScaling};

const fn bit_pix<const N: u32>(v: u32, shift: u8) -> u8 {
    let mask: u32 = (1 << N) - 1;
    let m = 1 << (8 - N);
    ((v >> shift) & mask) as u8 * m
}

fn bgra4_to_rgba8(v: u16) -> Rgba<u8> {
    let b = bit_pix::<4>(v as u32, 0);
    let g = bit_pix::<4>(v as u32, 4);
    let r = bit_pix::<4>(v as u32, 8);
    let a = bit_pix::<4>(v as u32, 12);

    [r, g, b, a].into()
}

fn bgr565_to_rgba8(v: u16) -> Rgba<u8> {
    let b = bit_pix::<5>(v as u32, 0);
    let g = bit_pix::<6>(v as u32, 5);
    let r = bit_pix::<5>(v as u32, 11);

    [r, g, b, 0xff].into()
}

fn bgra8_to_rgba8(v: u32) -> Rgba<u8> {
    v.to_le_bytes().into()
}

pub struct Canvas {
    data: Vec<u8>,
    depth: WzCanvasDepth,
    pub raw_w: u32,
    pub raw_h: u32,
    pub width: u32,
    pub height: u32,
    pub scale: WzCanvasScaling,
}

impl Canvas {
    pub fn from_data(data: Vec<u8>, wz_canvas: &WzCanvas) -> Self {
        Self {
            data,
            depth: wz_canvas.depth,
            width: wz_canvas.width(),
            height: wz_canvas.height(),
            scale: wz_canvas.scale,
            raw_w: wz_canvas.raw_width(),
            raw_h: wz_canvas.raw_height(),
        }
    }

    pub fn to_raw_rgba_image(&self) -> anyhow::Result<image::RgbaImage> {
        let w = self.raw_w;
        let h = self.raw_h;

        match self.depth {
            WzCanvasDepth::BGRA4444 => {
                let data: &[u16] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgra4_to_rgba8(data[(x + y * self.width) as usize])
                }))
            }
            WzCanvasDepth::BGRA8888 => {
                let data: &[u32] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgra8_to_rgba8(data[(x + y * self.width) as usize])
                }))
            }
            WzCanvasDepth::BGR565 => {
                let data: &[u16] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgr565_to_rgba8(data[(x + y * w) as usize])
                }))
            }
            WzCanvasDepth::DXT3 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc3.decompress(&self.data, w as usize, h as usize, &mut buf);
                Ok(RgbaImage::from_raw(w, h, buf)
                    .ok_or_else(|| anyhow::anyhow!("Failed to convert DXT3 to RGBA image"))?)
            }
            WzCanvasDepth::DXT5 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc5.decompress(
                    &self.data,
                    self.width as usize,
                    self.height as usize,
                    &mut buf,
                );
                Ok(RgbaImage::from_raw(self.width, self.height, buf)
                    .ok_or_else(|| anyhow::anyhow!("Failed to convert DXT5 to RGBA image"))?)
            }
        }
    }

    pub fn canvas_size(&self) -> u32 {
        self.height * self.width * self.depth.depth_size()
    }
}

#[cfg(test)]
mod tests {
    use crate::canvas::bit_pix;

    #[test]
    fn bit_pix_() {
        assert_eq!(bit_pix::<8>(0x1234, 8), 0x12);
        assert_eq!(bit_pix::<4>(0x1234, 8), 0x2 * 16);
        assert_eq!(bit_pix::<3>(0x1234, 8), 2 * 32);
        assert_eq!(bit_pix::<3>(0x123F, 0), 7 * 32);
    }
}
