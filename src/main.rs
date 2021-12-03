use adhoc_audio::{AdhocCodec, SeekFrom, StreamInfo, Streamable, WavCodec};
use audio_recorder::{
    collections::{LinkedList, Ptr},
    math,
    web_utils::{DomIter, ParentIter},
};
use js_sys::{Array, ArrayBuffer, Function, Uint8Array};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use wasm_bindgen::{closure::Closure, convert::FromWasmAbi, prelude::*, JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::*;

pub static mut GLOBAL_APP_STATE: Option<AppState> = None;

#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct RecordState {
    is_recording: bool,
    processing_node: Ptr,
}
impl RecordState {
    pub fn to_string(&self) -> String {
        let state_binary = bincode::serialize(self).expect("state serialization failed(binary)");
        base64::encode(state_binary)
    }
    pub fn from_string(base64: &str) -> Self {
        let binary = base64::decode(base64).expect("bad base64");
        bincode::deserialize::<RecordState>(&binary).expect("bad binary")
    }
}

pub struct AppState {
    pub audio_codec: AdhocCodec,
    pub processor_list: LinkedList<ScriptProcessorNode>,
}
impl AppState {
    fn init() {
        unsafe {
            GLOBAL_APP_STATE = Some(AppState {
                processor_list: LinkedList::new(),
                audio_codec: AdhocCodec::new().with_compression_level(4),
            });
        }
    }
    fn get() -> &'static Self {
        unsafe { GLOBAL_APP_STATE.as_ref().unwrap() }
    }

    fn get_mut() -> &'static mut Self {
        unsafe { GLOBAL_APP_STATE.as_mut().unwrap() }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace=console,js_name=log)]
    fn log_js(val: JsValue);

    #[wasm_bindgen(js_namespace=console,js_name=log)]
    fn log_u32(val: u32);
}

pub fn closure_to_function<CB, T>(cb: CB) -> js_sys::Function
where
    T: FromWasmAbi + 'static,
    CB: FnMut(T) + 'static,
{
    Closure::wrap(Box::new(cb) as Box<dyn FnMut(T)>)
        .into_js_value()
        .dyn_into::<Function>()
        .unwrap()
}

pub fn bytes_to_blob(byte_slice: &[u8]) -> Blob {
    let bytes = Array::new();
    let typed_array = Uint8Array::new_with_length(byte_slice.len() as u32);
    typed_array.copy_from(&byte_slice[..]);
    bytes.push(&typed_array);
    let mut options = BlobPropertyBag::new();
    options.type_("application/octet-stream");
    let blob =
        Blob::new_with_u8_array_sequence_and_options(&bytes, &options).expect("blob failed");

    log_js(blob.dyn_ref::<JsValue>().unwrap().clone());
    blob
}

#[test]
fn foo() {
    use std::fs::File;
    let file = File::open("./recorder_output/test/rec.adhoc").unwrap();
    let mut codec = AdhocCodec::load(file).unwrap();
    let mut wav = WavCodec::new(codec.info());
    let mut buffer = [0.0; 1024];
    while let Some(n) = codec.decode(&mut buffer) {
        wav.encode(&buffer[0..n]);
    }

    wav.save_to(File::create("./recorder_output/test/rec.wav").unwrap()).unwrap();
}

async fn start() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let navigator = window.navigator();
    let document = window.document().ok_or(JsValue::NULL)?;

    let mut constraints = MediaStreamConstraints::new();
    let ctx: AudioContext = AudioContext::new()?;

    AppState::init();
    AppState::get_mut()
        .audio_codec
        .set_info(StreamInfo::new(44_100, 1));

    let stream = JsFuture::from(
        navigator.media_devices()?.get_user_media_with_constraints(
            &constraints
                .audio(&JsValue::from_bool(true))
                .video(&JsValue::from_bool(false)),
        )?,
    )
    .await?
    .dyn_into::<MediaStream>()?;

    let start_recording = closure_to_function(move |mouse_event: MouseEvent| {
        log("button pressed");

        let button = mouse_event
            .current_target()
            .and_then(|t| t.dyn_into::<HtmlButtonElement>().ok())
            .expect("button target expected");

        let form = ParentIter::new(&button)
            .find_map(|e| e.dyn_into::<HtmlFormElement>().ok())
            .expect("button must be imbedded into a form");

        form.set_onsubmit(Some(&closure_to_function(move |e: FocusEvent| {
            log("submitting data");
            e.prevent_default();

            let form = e
                .current_target()
                .and_then(|e| e.dyn_into::<HtmlFormElement>().ok())
                .expect("not form");
            let form_data = FormData::new_with_form(&form).expect("form data not possible");

            let codec = &mut AppState::get_mut().audio_codec;
            let mut compressed_audio = Vec::<u8>::new();

            codec.save_to(&mut compressed_audio).expect("error");
            // log(format!("{:?}", compressed_audio).as_str());
            codec.init();
            let blob = bytes_to_blob(&compressed_audio);

            form_data
                .append_with_blob_and_filename("audio_recording", &blob, "rec.adhoc")
                .expect("append failed");

            let request = XmlHttpRequest::new().unwrap();
            request.open("POST", form.action().as_str()).unwrap();
            request
                .set_request_header("accept", "application/octet-stream")
                .unwrap();
            request
                .override_mime_type("application/octet-stream")
                .ok()
                .unwrap();
            request.send_with_opt_form_data(Some(&form_data)).unwrap();

            request.set_onloadend(Some(&closure_to_function(move |_v: JsValue| {
                //to emulate a regular form behaviour refresh page after request
                web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|doc| doc.location())
                    .and_then(|loc| loc.reload().ok());
            })));
        })));

        if let Some(data) = button.get_attribute("data-state") {
            let record_state = RecordState::from_string(&data);
            match record_state {
                RecordState {
                    is_recording: true,
                    processing_node,
                } => {
                    let processor_list = &mut AppState::get_mut().processor_list;
                    let script_node = processor_list.get(processing_node).unwrap().data().unwrap();
                    script_node.set_onaudioprocess(None);
                    button
                        .remove_attribute("data-state")
                        .expect("state delete failed");
                    button
                        .remove_attribute("style")
                        .expect("style delete failed");
                    processor_list.remove_at(processing_node);
                    log("stop recording..");
                }
                _ => panic!("shouldn't possible to reach"),
            }
        } else {
            let source = ctx.create_media_stream_source(&stream).unwrap();
            let processor = ctx.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(1024, 1, 1).unwrap();

            source
                .connect_with_audio_node(processor.dyn_ref().unwrap())
                .unwrap();

            let mut t = 0.0f32;
            const BEEP_DURATION_IN_SECONDS: f32 = 1.0;
            const BEEP_DAMPEN_DELTA:f32 =0.5;
            let dt = 1.0 / 44_100.0;

            processor.set_onaudioprocess(Some(&closure_to_function(
                move |e: AudioProcessingEvent| {
                    let micophone_input = e.input_buffer().unwrap();
                    let speaker_output = e.output_buffer().unwrap();

                    let microphone_samples =
                        micophone_input.get_channel_data(0).unwrap_or(Vec::new());
                    let mut beep_buffer = [0.0; 1024];

                    if t < BEEP_DURATION_IN_SECONDS {
                        let s = math::linear_step(t, BEEP_DURATION_IN_SECONDS-BEEP_DAMPEN_DELTA, BEEP_DURATION_IN_SECONDS); 
                        for sample in &mut beep_buffer {
                            let dampening = 1.0-s*s;
                            *sample = (t * 5_000.0).sin() * 0.5 * dampening;
                            t += dt;
                        }
                        speaker_output.copy_to_channel(&beep_buffer[..], 0).unwrap();
                    }

                    AppState::get_mut().audio_codec.encode(&microphone_samples);

                    let amplitude = microphone_samples
                        .iter()
                        .max_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap_or(Ordering::Equal))
                        .map(|&a| a.abs())
                        .unwrap_or(1.0) *10.0;

                    let t = (amplitude.clamp(0.8, 2.0) - 0.8) / (2.0-0.8);

                    let color_0 = [128.0, 128.0, 128.0];
                    let color_1 = [255.0, 0.0, 0.0];
                    let lerp = math::lerp(color_0, color_1, t*0.8 + 0.2 );

                    button
                        .set_attribute(
                            "style",
                            format!(
                                r"color:rgb({:.2},{:.2},{:.2}); 
                            --ggs:{:.3}; 
                            ",
                                lerp[0],
                                lerp[1],
                                lerp[2],
                                (t * t) * 0.8+ 0.8,
                            )
                            .as_str(),
                        )
                        .unwrap();
                },
            )));
            processor
                .connect_with_audio_node(ctx.destination().dyn_ref().unwrap())
                .unwrap();

            let processor_list = &mut AppState::get_mut().processor_list;

            processor_list.push_front(processor);
            let processor_pointer = processor_list.front();
            let state = RecordState {
                is_recording: true,
                processing_node: processor_pointer,
            };
            let button = mouse_event
                .current_target()
                .and_then(|t| t.dyn_into::<HtmlButtonElement>().ok())
                .expect("button target expected");
            button
                .set_attribute("data-state", &state.to_string())
                .expect("state failed to set");
        }
    });

    DomIter::new(document.get_elements_by_class_name("recorder_button"))
        .flat_map(|button_container| DomIter::by_tag_name(button_container, "button"))
        .filter_map(|e| e.dyn_into::<HtmlButtonElement>().ok())
        .for_each(|button| button.set_onclick(Some(&start_recording)));

    Ok(())
}

fn main() {
    spawn_local(async {
        match start().await {
            Ok(_) => {
                log("module exited nicely");
            }
            Err(_) => {
                log("we hit an error somewhere... ");
            }
        }
    });
}
