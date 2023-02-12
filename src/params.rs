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

use nih_plug::prelude::*;
use nih_plug_egui::{EguiState, egui::{Ui, Response, Context, Window, epaint::ahash::HashMap}};
use rtrb::Producer;
use std::{fs::read_dir, sync::Arc, any::{Any, TypeId}, borrow::Borrow};

use crate::{MAX_POLYPHONY, dsp::wavetable::BandlimitedWaveTables};

pub const WAVETABLE_FOLDER_PATH: &str = "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

pub type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

pub fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

pub fn send<T>(message_sender: &mut Producer<T>, message: T) {
    while message_sender.is_full() {}
    message_sender.push(message).unwrap();
}

#[non_exhaustive]
pub enum AudioGraphEvent {
    Connect(usize, usize),
    Reschedule(Box<[usize]>),
    PushNode(Box<dyn Processor>),
    SetWaveTable(usize, BandlimitedWaveTables),
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

    fn ui(
        &self,
        node_index: usize,
        ui: &mut Ui,
        setter: &ParamSetter,
        messages_sender: &mut Producer<AudioGraphEvent>
    ) -> Response;

    fn processor(self: Arc<Self>) -> Box<dyn Processor>;
}

#[derive(Params)]
pub struct KrynthParams {
    pub editor_state: Arc<EguiState>,
    pub global_params: GlobalParams,
    /// used to send messages to the audio thread
    message_sender: Mutex<Producer<AudioGraphEvent>>,
    /// parameter values of the audio graph, in topological order
    graph: AtomicRefCell<AudioGraph<String, Arc<dyn NodeParameters>>>,
    /// used to keep track of how many of the same node type is there, (counters...)
    node_count_per_type: AtomicRefCell<HashMap<TypeId, usize>>,
}

impl KrynthParams {

    pub fn new(producer: Producer<AudioGraphEvent>) -> Self {

        Self {
            global_params: GlobalParams::new(),
            editor_state: EguiState::from_size(1140, 590),
            message_sender: Mutex::new(producer),
            graph: Default::default(),
            node_count_per_type: Default::default()
        }
    }

    pub fn ui(&self, ctx: &Context, setter: &ParamSetter) {

        for (node_index, node_params) in self.graph.borrow().iter().enumerate() {

            Window::new(node_params.id())
                .fixed_size((400., 500.))
                .show(ctx, |ui| {
                    node_params.data.ui(
                        node_index,
                        ui,
                        setter,
                        &mut self.message_sender.lock()
                    );
                });
        }
    }

    pub fn send(&self, event: AudioGraphEvent) {
        send(&mut self.message_sender.lock(), event);
    }

    pub fn connect<Q>(&self, from: &Q, to: &Q)
    where
        String: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        if let Some(((from_index, to_index), maybe_schedule)) = self.graph.borrow_mut().connect(from, to) {
            self.send(AudioGraphEvent::Connect(from_index, to_index));
            if let Some(new_schedule) = maybe_schedule {
                self.send(AudioGraphEvent::Reschedule(new_schedule));
            }
        }
    }

    pub fn insert_top_level_node(&self, node: Arc<dyn NodeParameters>) {

        let mut map = self.node_count_per_type.borrow_mut();
        let id = node.type_id();

        *map.entry(id).or_insert(0) += 1;

        let count = map.get(&id).unwrap().to_string();

        let node_name = format!("{} {count}", node.type_name());

        self.send(AudioGraphEvent::PushNode(Arc::clone(&node).processor()));

        self.graph.borrow_mut().top_level_insert(node_name, node);
    }

    pub fn build_audio_graph(&self, schedule: &mut ProcessSchedule) {

        for node in self.graph.borrow().iter() {

            schedule.push(
                node.data.clone().processor(),
                node.edges().iter().map(|&(Edge::Normal(i) | Edge::Feedback(i))| i).collect()
            );
        }
    }
}