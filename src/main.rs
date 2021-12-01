use std::{
    cell::RefCell, cmp::Ordering, collections::btree_set::Iter, future::Future, ops, sync::Arc,
};

use wasm_bindgen::{closure::Closure, convert::FromWasmAbi, prelude::*, JsCast, JsValue};

use js_sys::Function;

use wasm_bindgen_futures::{spawn_local, JsFuture};

use web_sys::*;

use audio_recorder::{
    collections::{LinkedList, Ptr},
    math,
};

use adhoc_audio::{AdhocCodec, StreamInfo, Streamable};

pub static mut GLOBAL_APP_STATE: Option<AppState> = None;

pub struct JsCallBackPool {
    handlers: LinkedList<Option<JsCallbackHandler>>,
}
impl JsCallBackPool {
    pub fn new() -> Self {
        Self {
            handlers: LinkedList::new(),
        }
    }

    pub fn register_handler(&mut self, callback: js_sys::Function) -> JsCallbackHandler {
        self.handlers.push_rear(None);
        let thread_id = self.handlers.rear();

        let handler = JsCallbackHandler {
            thread_id,
            callback,
        };

        self.handlers
            .get_mut(thread_id)
            .map(|node| node.set_data(Some(handler.clone())));

        handler
    }
}

#[derive(Clone)]
pub struct JsCallbackHandler {
    pub thread_id: Ptr,
    pub callback: js_sys::Function,
}

pub static mut LMAO_A_GLOBAL_FUNCTION: Option<js_sys::Function> = None;

pub struct AppState {
    output_buffer: Vec<f32>,
    audio_codec: AdhocCodec,
    callback_registry: JsCallBackPool,
}
impl AppState {
    fn init() {
        unsafe {
            GLOBAL_APP_STATE = Some(AppState {
                output_buffer: vec![0.0; 1024],
                audio_codec: AdhocCodec::new(),
                callback_registry: JsCallBackPool::new(),
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

pub struct CollectionIterator {
    html: HtmlCollection,
    idx: u32,
}

impl CollectionIterator {
    pub fn new(c: HtmlCollection) -> Self {
        Self { html: c, idx: 0 }
    }
}

impl Iterator for CollectionIterator {
    type Item = Element;
    fn next(&mut self) -> Option<Self::Item> {
        let elem = self.html.get_with_index(self.idx);
        self.idx += 1;
        elem
    }
}

fn get_elements_by_class_name<T>(element: T, names: &str) -> CollectionIterator
where
    T: AsRef<Element>,
{
    CollectionIterator::new(element.as_ref().get_elements_by_class_name(names))
}
fn get_elements_by_tag_name<T>(element: T, names: &str) -> CollectionIterator
where
    T: AsRef<Element>,
{
    CollectionIterator::new(element.as_ref().get_elements_by_tag_name(names))
}

async fn start() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let navigator = window.navigator();
    let document = window.document().ok_or(JsValue::NULL)?;

    let mut constraints = MediaStreamConstraints::new();
    let ctx: AudioContext = AudioContext::new()?;

    AppState::init();

    let adhoc = Arc::new(RefCell::new(
        AdhocCodec::new()
            .with_compression_level(7)
            .with_info(StreamInfo::new(44_100, 1)),
    ));

    let adhoc_clone = adhoc.clone();
    let handler = AppState::get_mut()
        .callback_registry
        .register_handler(closure_to_function(move |e: AudioProcessingEvent| {
            let micophone_input = e.input_buffer().unwrap();
            let speaker_output = e.output_buffer().unwrap();
            let microphone_samples = micophone_input.get_channel_data(0).unwrap_or(Vec::new());
            let adhoc = adhoc_clone.clone();

            adhoc.borrow_mut().encode(&microphone_samples);
            speaker_output
                .copy_to_channel(&microphone_samples[..], 0)
                .unwrap();
        }));

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
            .expect("asdasd");

        log_js(mouse_event.dyn_ref::<JsValue>().unwrap().clone());
        let adhoc = (&adhoc).clone();
        let source = ctx.create_media_stream_source(&stream).unwrap();
        let processor = ctx.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(1024, 1, 1).unwrap();

        source
            .connect_with_audio_node(processor.dyn_ref().unwrap())
            .unwrap();
        // let audioprocess_cb = AppState::get().callback_registry.handlers[handler.thread_id]
        //     .data()
        //     .unwrap()
        //     .as_ref()
        //     .map(|e| &e.callback);
        // processor.set_onaudioprocess(audioprocess_cb);

        let audioprocess_cb = closure_to_function(move |e: AudioProcessingEvent| {
            let micophone_input = e.input_buffer().unwrap();
            let speaker_output = e.output_buffer().unwrap();
            let microphone_samples = micophone_input.get_channel_data(0).unwrap_or(Vec::new());
            adhoc.borrow_mut().encode(&microphone_samples);

            let amplitude = microphone_samples
                .iter()
                .max_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap_or(Ordering::Equal))
                .map(|&a| a.abs())
                .unwrap_or(1.0)
                * 4.0
                + 0.8;

            speaker_output
                .copy_to_channel(&microphone_samples[..], 0)
                .unwrap();

            let t = (amplitude.min(1.0) - 0.8) / 0.2;

            let color_0 = [128.0, 128.0, 128.0];
            let color_1 = [255.0, 0.0, 0.0];
            let lerp = math::lerp(color_0, color_1, t);

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
                        (t*t) * 0.8 + 0.8,
                    )
                    .as_str(),
                )
                .unwrap();
        });

        processor.set_onaudioprocess(Some(&audioprocess_cb));

        processor
            .connect_with_audio_node(ctx.destination().dyn_ref().unwrap())
            .unwrap();
    });

    CollectionIterator::new(document.get_elements_by_class_name("recorder_button"))
        .flat_map(|button_container| get_elements_by_tag_name(button_container, "button"))
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

/*
JsFuture::from(
    navigator
        .media_devices()?
        .get_user_media_with_constraints(
            &constraints
                .audio(&JsValue::from_bool(true))
                .video(&JsValue::from_bool(false)),
        )?
        .then(&Closure::wrap(Box::new(move |stream: JsValue| {
            log("entered stream");

            let stream = stream.dyn_into::<MediaStream>().unwrap();

            let source = ctx.create_media_stream_source(&stream).unwrap();

            let processor = ctx.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(1024, 1, 1).unwrap();

            source.connect_with_audio_node( processor.dyn_ref().unwrap()  ).unwrap();

            let audioprocess_cb = AppState::get()
                .callback_registry
                .handlers[handler.thread_id]
                .data()
                .unwrap()
                .as_ref()
                .map(|e| &e.callback);

            processor.set_onaudioprocess(audioprocess_cb);

            processor.connect_with_audio_node(ctx.destination().dyn_ref().unwrap()).unwrap();

        }) as Box<dyn FnMut(_)>)),
)
.await?;
*/
