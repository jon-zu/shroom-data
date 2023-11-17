use dioxus::prelude::*;

use std::rc::Rc;
use wasm_bindgen::JsCast;

use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement, Url};

use shroom_wz::l1::sound::WzSound;

#[derive(Debug)]
pub struct AudioData {
    pub data: Vec<u8>,
    pub format: WzSound,
}

impl PartialEq for AudioData {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for AudioData {}

#[inline_props]
pub fn AudioView(cx: Scope, audio: Rc<AudioData>) -> Element {
    let audio_ref = use_ref::<Option<HtmlAudioElement>>(cx, || None);

    use_effect(cx, (audio_ref, audio), |(audio_ref, audio)| async move {
        let audio_r = audio_ref.read();

        let Some(audio_elem) = audio_r.as_ref() else {
            return;
        };

        let uint8arr =
            js_sys::Uint8Array::new(&unsafe { js_sys::Uint8Array::view(&audio.data) }.into());
        let array = js_sys::Array::new();
        array.push(&uint8arr.buffer());
        let bag = BlobPropertyBag::new().type_("audio/mpeg").to_owned();
        let blob = Blob::new_with_u8_array_sequence_and_options(&array, &bag).unwrap();
        let url = Url::create_object_url_with_blob(&blob).unwrap();

        audio_elem.set_src(&url);
    });

    cx.render(rsx! {
        audio {
            controls: true,
            onmounted: |ev| {
                let elem = ev.get_raw_element().expect("Must access element")
                    .downcast_ref::<web_sys::Element>().expect("Must be element")
                    .dyn_ref::<HtmlAudioElement>().expect("Must be audio");
                *audio_ref.write() = Some(elem.clone());
            }
        }
    })
}
