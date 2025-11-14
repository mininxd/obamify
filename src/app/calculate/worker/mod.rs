use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use serde::{Deserialize, Serialize};
use web_sys::DedicatedWorkerGlobalScope;
use web_sys::js_sys;

use crate::app::calculate::ProgressMsg;
use crate::app::headless::Generator;
use crate::app::preset::UnprocessedPreset;
use crate::app::calculate::util::GenerationSettings;

#[derive(Serialize, Deserialize)]
pub enum WorkerReq {
    Process {
        source: UnprocessedPreset,
        target: UnprocessedPreset,
        settings: GenerationSettings,
    },
}

#[wasm_bindgen]
pub fn worker_entry() {
    let global: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let global_for_handler = global.clone();

    let handler = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        let req: WorkerReq = match serde_wasm_bindgen::from_value(e.data()) {
            Ok(v) => v,
            Err(err) => {
                let _ = global_for_handler.post_message(
                    &serde_wasm_bindgen::to_value(&ProgressMsg::Error(format!("bad req: {err}")))
                        .unwrap(),
                );
                return;
            }
        };

        match req {
            WorkerReq::Process {
                source,
                target,
                settings,
            } => {
                let generator = Generator::new(serde_wasm_bindgen::to_value(&settings).unwrap()).unwrap();

                match generator.generate(
                    serde_wasm_bindgen::to_value(&source).unwrap(),
                    serde_wasm_bindgen::to_value(&target).unwrap(),
                ) {
                    Ok(result) => {
                        let _ = global_for_handler.post_message(&result);
                    }
                    Err(e) => {
                        let _ = global_for_handler.post_message(
                            &serde_wasm_bindgen::to_value(&ProgressMsg::Error(format!("generation failed: {:?}", e)))
                                .unwrap(),
                        );
                    }
                }
            }
        }
    }) as Box<dyn FnMut(_)>);

    global.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
}
