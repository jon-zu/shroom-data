use std::time::Duration;

//use image::{imageops::overlay, EncodableLayout, RgbaImage};

use crate::{
    canvas::Canvas,
    file::{WzIO, WzImgReader},
    l1::canvas::WzCanvas,
    val::{ObjectVal, Vec2Val, WzValue},
};

pub struct AnimationFrame {
    pub offset: Option<Vec2Val>,
    pub delay: Option<Duration>,
    pub canvas: WzCanvas,
}

pub struct Animation {
    pub frames: Vec<AnimationFrame>,
    pub dim: (u32, u32),
}

impl Animation {
    pub fn from_frames(frames: Vec<AnimationFrame>) -> Self {
        let mut dim_h = 0;
        let mut dim_w = 0;
        for frame in frames.iter() {
            dim_h = dim_h.max(frame.canvas.height());
            dim_w = dim_w.max(frame.canvas.width());
        }
        Self {
            frames,
            dim: (dim_w, dim_h),
        }
    }

    pub fn from_obj_value(obj_val: &ObjectVal) -> anyhow::Result<Self> {
        let mut dim_h = 0;
        let mut dim_w = 0;
        let mut frames = Vec::new();
        for (key, frame) in obj_val.0.iter() {
            // Skip non-numeric keys
            if key.parse::<usize>().is_err() {
                continue;
            }

            let frame = frame.as_canvas().ok_or_else(|| {
                anyhow::anyhow!("Expected canvas for animation frame, got {:?}", frame)
            })?;

            let mut delay = None;
            let mut origin = None;

            if let Some(WzValue::Object(obj)) = frame.sub.as_deref() {
                delay = obj
                    .0
                    .get("delay")
                    .and_then(|v| v.as_i32())
                    .map(|v| Duration::from_millis(v as u64));
                origin = obj.0.get("origin").and_then(|v| v.as_vec().cloned());
            }

            dim_h = dim_h.max(frame.canvas.height());
            dim_w = dim_w.max(frame.canvas.width());
            frames.push(AnimationFrame {
                offset: origin,
                delay,
                canvas: frame.canvas.clone(),
            });
        }

        if frames.is_empty() {
            anyhow::bail!("No frames found in animation");
        }

        Ok(Self {
            frames,
            dim: (dim_w, dim_h),
        })
    }

    pub fn load_all_frames<R: WzIO>(&self, r: &mut WzImgReader<R>) -> anyhow::Result<Vec<Canvas>> {
        let mut v = vec![];
        for frame in self.frames.iter() {
            v.push(r.read_canvas(&frame.canvas)?);
        }
        Ok(v)
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn get_canvas_frame(&self, frame: usize) -> Option<&WzCanvas> {
        self.frames.get(frame).map(|f| &f.canvas)
    }

    pub fn frames(&self) -> &[AnimationFrame] {
        &self.frames
    }

    pub fn dim(&self) -> (u32, u32) {
        self.dim
    }

    #[cfg(feature = "webp")]
    pub fn to_webp<R: WzIO>(
        &self,
        r: &mut WzImgReader<R>,
    ) -> anyhow::Result<webp_animation::WebPData> {
        use image::imageops::overlay;
        use image::EncodableLayout;
        use image::RgbaImage;
        let (w, h) = self.dim;

        let mut encoder = webp_animation::Encoder::new((w, h))?;

        let mut timestamp = 0;

        for frame in self.frames.iter() {
            let mut back = RgbaImage::from_pixel(w, h, [0u8; 4].into());
            let img = r.read_canvas(&frame.canvas)?;
            let img = img.to_raw_rgba_image()?;
            overlay(&mut back, &img, 0, 0);

            encoder.add_frame(back.as_bytes(), timestamp)?;

            timestamp += frame
                .delay
                .unwrap_or(Duration::from_millis(100))
                .as_millis() as i32;
        }
        Ok(encoder.finalize(timestamp)?)
    }
}
