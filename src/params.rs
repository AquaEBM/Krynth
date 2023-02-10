pub mod wt_osc;
pub mod distortion;

use atomic_refcell::AtomicRefCell;
use parking_lot::Mutex;
use plugin_util::{
    parameter::Modulable,
    dsp::{
        processor::{Processor, ProcessSchedule},
        graph::{Edge, AudioGraph}
    }
};

use nih_plug::{prelude::*};
use nih_plug_egui::{EguiState, egui::{Ui, Response, Context, Window, epaint::ahash::HashMap}};
use rtrb::Producer;
use std::{fs::read_dir, sync::Arc, any::{Any, TypeId}, borrow::Borrow};

use crate::MAX_POLYPHONY;

pub const WAVETABLE_FOLDER_PATH: &str = "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

pub type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

pub fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

#[non_exhaustive]
pub enum AudioGraphEvent {
    UpdateAudioGraph(ProcessSchedule),
    Connect(usize, usize),
    Reschedule(Box<[usize]>),
    
}

pub struct AudioGraphData {
    /// used to send messages to the audio thread
    message_sender: Mutex<Producer<AudioGraphEvent>>,
    /// parameter values of the audio graph, in topological order
    graph: AtomicRefCell<AudioGraph<String, Arc<dyn NodeParameters>>>,
    /// used to keep track of how many of the same node type is there, (counters...)
    node_count_per_type: AtomicRefCell<HashMap<TypeId, usize>>,
}

impl AudioGraphData {
    pub fn new(producer: Producer<AudioGraphEvent>) -> Self {
        Self {
            message_sender: Mutex::new(producer),
            graph: Default::default(),
            node_count_per_type: Default::default()
        }
    }

    pub fn ui(&self, ctx: &Context, setter: &ParamSetter) {

        for node_params in self.graph.borrow().iter() {
    
            Window::new(&node_params.data.type_name())
                .fixed_size((400., 500.))
                .show(ctx, |ui| {
                    node_params.data.ui(ui, setter)
                });
        }
    }

    pub fn send(&self, event: AudioGraphEvent) {

        let mut producer = self.message_sender.lock();
        while producer.is_full() {}
        producer.push(event).unwrap();
    }

    pub fn connect<Q>(&self, from: &Q, to: &Q)
    where
        String: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        if let Some(((from_index, to_index), maybe_schedule)) = self.graph.borrow_mut().connect(from, to) {
            self.send(AudioGraphEvent::Connect(from_index, to_index));
            if let Some(new_schedule) = maybe_schedule {
                self.send(AudioGraphEvent::Reschedule(new_schedule.into_boxed_slice()));
            }
        }
    }

    pub fn insert_top_level_node(&self, node: Arc<dyn NodeParameters>) {

        let mut map = self.node_count_per_type.borrow_mut();
        let id = node.type_id();

        *map.entry(id).or_insert(0) += 1;

        let count = map.get(&id).unwrap().to_string();

        let node_name = format!("{} {count}", node.type_name());

        self.graph.borrow_mut().top_level_insert(node_name, node);
    }
}

pub struct GlobalParams {
    pub wt_list: Arc<[String]>,
}

impl GlobalParams {
    pub fn new() -> Self {
        Self {
            wt_list: read_dir(WAVETABLE_FOLDER_PATH).unwrap().map(|dir| dir
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .trim_end_matches(".WAV")
                .into()
            ).collect::<Vec<_>>().into()
        }
    }
}

pub trait NodeParameters: Params + Any {

    fn new(params: &GlobalParams) -> Self where Self: Sized;

    fn type_name(&self) -> String;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor(&self, global_params: &GlobalParams) -> Box<dyn Processor>;

    fn reload(&self) {}
}

#[derive(Params)]
pub struct KrynthParams {
    #[persist = "editor"] pub editor_state   : Arc<EguiState>,
                          pub global_params  : GlobalParams,
                          pub graph_data     : AudioGraphData,
}

impl KrynthParams {

    pub fn new(producer: Producer<AudioGraphEvent>) -> Self {

        Self {
            global_params: GlobalParams::new(),
            editor_state: EguiState::from_size(1140, 590),
            graph_data: AudioGraphData::new(producer),
        }
    }

    pub fn build_audio_graph(&self) -> ProcessSchedule {

        let mut graph = ProcessSchedule::default();

        for node in self.graph_data.graph.borrow().iter() {

            graph.push(
                node.data.processor(&self.global_params),
                node.edges().iter().map(|&(Edge::Normal(i) | Edge::Feedback(i))| i).collect()
            );
        }

        graph
    }
}