pub mod wt_osc;
pub mod distortion;

use atomic_refcell::{AtomicRefCell};
use parking_lot::Mutex;
use plugin_util::{parameter::Modulable, dsp::{processor::{self, Processor}, graph::{Edge, self}}};
use nih_plug::{prelude::*};
use nih_plug_egui::{EguiState, egui::{Ui, Response, Context, Window}};
use rtrb::Producer;
use std::{fs::read_dir, sync::Arc, any::Any};

use crate::{MAX_POLYPHONY, AudioGraphEvent};

pub const WAVETABLE_FOLDER_PATH: &str = "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

pub type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

pub fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

pub trait NodeParemeters: Params + Any {
    fn type_name(&self) -> String;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor(&self, global_params: &KrynthParams) -> Box<dyn Processor>;

    fn reload(&self) {}
}

#[derive(Params)]
pub struct KrynthParams {
                          pub wt_list        : Arc<[String]>,
                          pub message_sender : Mutex<Producer<AudioGraphEvent>>,
    #[persist = "editor"] pub editor_state   : Arc<EguiState>,
                          pub audio_graph    : AtomicRefCell<graph::AudioGraph<String, Arc<dyn NodeParemeters>>>
}

impl KrynthParams {

    pub fn new(producer: Producer<AudioGraphEvent>) -> Self {

        Self {
            wt_list: get_wavetable_name_list(),
            editor_state: EguiState::from_size(1140, 590),
            message_sender: Mutex::new(producer),
            audio_graph: Default::default()
        }
    }
 
    pub fn build_audio_graph(&self) -> processor::AudioGraph {

        let mut graph = processor::AudioGraph::default();

        for node in self.audio_graph.borrow().iter() {

            graph.push(
                node.data.processor(self),
                node.edges().iter().map(|&(Edge::Normal(i) | Edge::Feedback(i))| i).collect()
            );
        }

        graph
    }

    pub fn ui(&self, ctx: &Context, setter: &ParamSetter) {

        for node_params in self.audio_graph.borrow().iter() {
    
            Window::new(&node_params.data.type_name())
                .fixed_size((400., 500.))
                .show(ctx, |ui| {
                    node_params.data.ui(ui, setter)
                });
        }
    }
}

pub fn get_wavetable_name_list() -> Arc<[String]> {
    read_dir(WAVETABLE_FOLDER_PATH).unwrap().map(|dir| dir
        .unwrap()
        .file_name()
        .to_str()
        .unwrap()
        .trim_end_matches(".WAV")
        .into()
    ).collect::<Vec<_>>().into()
}