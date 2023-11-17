use std::{rc::Rc, time::Duration};

use dioxus::prelude::*;
use image::RgbaImage;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::wz::WzAnimationData;

pub struct CanvasContext {
    pub ctx: CanvasRenderingContext2d,
    pub canvas_element: HtmlCanvasElement,
}

impl CanvasContext {
    pub fn from_element(elem: HtmlCanvasElement) -> Self {
        let ctx = elem
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();
        Self {
            ctx,
            canvas_element: elem,
        }
    }

    pub fn clear(&self) {
        self.ctx.clear_rect(
            0.,
            0.,
            self.canvas_element.width() as f64,
            self.canvas_element.height() as f64,
        );
    }

    pub fn draw_img(&self, img_data: &web_sys::ImageData) {
        self.ctx.put_image_data(img_data, 0., 0.).unwrap();
    }
}

fn image_to_imgdata(img: &RgbaImage) -> web_sys::ImageData {
    let data = Clamped(img.as_raw().as_slice());
    web_sys::ImageData::new_with_u8_clamped_array_and_sh(data, img.width(), img.height())
        .expect("Img data")
}

#[inline_props]
pub fn ImageView(cx: Scope, image: Rc<RgbaImage>) -> Element {
    let canvas_ctx = use_state::<Option<CanvasContext>>(cx, || None);

    use_effect(cx, (canvas_ctx, image), |(canvas_ctx, image)| async move {
        let Some(canvas) = canvas_ctx.as_ref() else {
            return;
        };

        let data = image_to_imgdata(&image);
        canvas.clear();
        canvas.draw_img(&data);
    });

    cx.render(rsx! {
        canvas {
            onmounted: |ev| {
                let canvas = ev.get_raw_element().expect("Must access element")
                    .downcast_ref::<web_sys::Element>().expect("Must be element")
                    .dyn_ref::<HtmlCanvasElement>().expect("Must be canvas");
                canvas_ctx.set(Some(CanvasContext::from_element(canvas.clone())));
            }
        }
    })
}

#[inline_props]
pub fn AnimationView(cx: Scope, anim_data: Rc<WzAnimationData>) -> Element {
    let canvas_ctx = use_state::<Option<CanvasContext>>(cx, || None);
    let frame_ix = use_state(cx, || 0);

    use_effect(
        cx,
        (canvas_ctx, frame_ix, anim_data),
        |(canvas_ctx, frame_ix, anim_data)| async move {
            let Some(canvas) = canvas_ctx.as_ref() else {
                return;
            };

            let frames = &anim_data.frames;
            if frames.is_empty() {
                return;
            }

            let ix = frame_ix.min(frames.len() - 1);
            canvas.clear();
            canvas.draw_img(&image_to_imgdata(&frames[ix]));
            gloo::timers::future::sleep(
                anim_data.anim.frames[ix]
                    .delay
                    .unwrap_or(Duration::from_millis(100)),
            )
            .await;

            frame_ix.set((ix + 1) % frames.len());
        },
    );

    cx.render(rsx! {
        canvas {
            width: 400,
            height: 400,
            onmounted: |ev| {
                let canvas = ev.get_raw_element().expect("Must access element")
                    .downcast_ref::<web_sys::Element>().expect("Must be element")
                    .dyn_ref::<HtmlCanvasElement>().expect("Must be canvas");
                canvas_ctx.set(Some(CanvasContext::from_element(canvas.clone())));
            }
        }
    })
}
