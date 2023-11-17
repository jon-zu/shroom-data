#![allow(non_snake_case)]

pub mod audio_view;
pub mod image_view;
pub mod tree;
pub mod web_map;
pub mod wz;

use std::{io::Cursor, rc::Rc};

use anyhow::anyhow;
use dioxus::prelude::*;
use gloo::file::futures::read_as_bytes;
use shroom_wz::version::WzVersion;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::wz::{WzApp, WzData};

fn get_selected_file_from_input(file_input_id: &str) -> anyhow::Result<gloo::file::File> {
    let window = gloo::utils::document();
    let el = window
        .get_element_by_id(file_input_id)
        .unwrap()
        .dyn_into::<HtmlInputElement>()
        .unwrap();
    let files: gloo::file::FileList = el.files().expect("must have FileList").into();
    files
        .get(0)
        .ok_or_else(|| anyhow::format_err!("should contain one file"))
        .cloned()
}

async fn read_wz_data(file_input_id: &str, version: WzVersion) -> anyhow::Result<WzData> {
    let file = get_selected_file_from_input(file_input_id)?;
    let data = read_as_bytes(&file).await?;
    WzData::from_file(&file.name(), Cursor::new(data), version)
}

fn parse_version(form_data: &FormData) -> anyhow::Result<WzVersion> {
    let version = form_data
        .values
        .get("version")
        .and_then(|v| v.first())
        .ok_or(anyhow!("Must have version"))?;
    let version: usize = version.parse().map_err(|_| anyhow!("Invalid version"))?;
    Ok(version.into())
}

#[inline_props]
fn FileForm(cx: Scope, wz: UseState<Option<Rc<WzData>>>) -> Element {
    const FILE_INPUT_ID: &str = "wz-file-input";
    let alert_error = use_state(cx, || None);

    let load_file = |version: WzVersion| {
        to_owned!(wz);
        to_owned![alert_error];
        cx.spawn({
            async move {
                match read_wz_data(FILE_INPUT_ID, version).await {
                    Ok(file) => {
                        wz.set(Some(Rc::new(file)));
                        alert_error.set(None);
                    }
                    Err(e) => {
                        alert_error.set(Some(e.to_string()));
                    }
                }
            }
        });
    };

    cx.render(rsx! {
        div {
            class: "flex flex-col justify-center items-center gaps-4",
                if let Some(msg) = alert_error.get() {
                    Some(rsx!(div {
                        class: "alert alert-error",
                        "{msg}"
                    }))
                }
        form {
            class: "space-y-3",
            //prevent_default: "onsubmit",
            onsubmit: move |ev: Event<FormData>| {
                let Ok(selected_version) = parse_version(&ev.data) else {
                    alert_error.set(Some("Invalid version".to_string()));
                    return;
                };
                log::info!("files: {:?}", ev.data.values.get("file"));
                load_file(selected_version);
            },
            div {
                class: "form-control w-full max-w-xs",
                label {
                    class: "label",
                    "File(.wz)"
                }
                input {
                    id: FILE_INPUT_ID,
                    r#type: "file",
                    accept: "*.wz",
                    class: "file-input file-input-bordered w-full max-w-xs",
                    name: "file"
                }
            },
            div {
                class: "form-control w-full max-w-xs",
                label {
                    class: "label",
                    "Version"
                }
                input {
                    r#type: "number",
                    class: "input input-bordered w-full max-w-xs",
                    name: "version",
                    value: "95",
                }
            },
            input {
                class: "btn btn-primary w-full",
                r#type: "submit",
                "Load"
            }
    }}
    })
}

/// Convience function
fn WebApp(cx: Scope) -> Element {
    let wz = use_state::<Option<Rc<WzData>>>(cx, || None);

    let main = if wz.get().is_some() {
        rsx!(WzApp { wz: wz.clone() })
    } else {
        rsx!(FileForm { wz: wz.clone() })
    };

    cx.render(rsx! {
        div {
            class: "lex min-h-full flex-col justify-center px-6 py-12 lg:px-8",
            main
        }
    })
}

fn launch_web() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
    dioxus_web::launch(WebApp);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    launch_web()
}
