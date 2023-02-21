pub mod distortion;
pub mod wt_osc;

use atomic_refcell::AtomicRefCell;
use parking_lot::Mutex;
use plugin_util::{
    dsp::{
        graph::AudioGraph,
        processor::{ProcessSchedule, Processor},
    },
    parameter::Modulable,
};

use nih_plug::prelude::*;
use nih_plug_egui::{
    egui::{Context, Response, Ui, Window},
    EguiState,
};
use rtrb::{Consumer, Producer};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fs::read_dir,
    sync::Arc,
};

use crate::MAX_POLYPHONY;

pub const WAVETABLE_FOLDER_PATH: &str =
    "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

pub type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

pub fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

pub fn send<T>(message_sender: &mut Producer<T>, message: T) {
    while message_sender.is_full() {}
    message_sender.push(message).unwrap();
}

#[non_exhaustive]
pub enum MainThreadMessage {
    BuildAudioGraph(ProcessSchedule),
}

#[non_exhaustive]
pub enum ProcessingThreadMessage {
    FreeGraph(ProcessSchedule),
}

pub struct GlobalParams {
    pub wt_list: Arc<[String]>,
}

impl GlobalParams {
    pub fn new() -> Self {
        Self {
            wt_list: read_dir(WAVETABLE_FOLDER_PATH)
                .unwrap()
                .map(|dir| {
                    dir.unwrap()
                        .file_name()
                        .to_str()
                        .unwrap()
                        .trim_end_matches(".WAV")
                        .into()
                })
                .collect::<Vec<_>>()
                .into(),
        }
    }
}

pub trait NodeParameters: Params + Any {
    fn new(params: &GlobalParams) -> Self
    where
        Self: Sized;

    fn type_name(&self) -> String;

    fn ui(&self, node_index: usize, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor(self: Arc<Self>) -> Box<dyn Processor + Send>;
}

#[derive(Params)]
pub struct KrynthParams {
    pub editor_state: Arc<EguiState>,
    pub global_params: GlobalParams,
    /// used to send messages to the audio thread
    message_sender: Mutex<(
        Producer<MainThreadMessage>,
        Consumer<ProcessingThreadMessage>,
    )>,
    /// parameter values of the audio graph, in topological order
    graph: AtomicRefCell<AudioGraph<Arc<dyn NodeParameters>>>,
    /// used to keep track of nodes of the same type
    node_count_per_type: AtomicRefCell<HashMap<TypeId, usize>>,
}

impl KrynthParams {
    pub fn new(
        producer: Producer<MainThreadMessage>,
        deallocator: Consumer<ProcessingThreadMessage>,
    ) -> Self {
        Self {
            global_params: GlobalParams::new(),
            editor_state: EguiState::from_size(1140, 590),
            message_sender: Mutex::new((producer, deallocator)),
            graph: Default::default(),
            node_count_per_type: Default::default(),
        }
    }

    pub fn ui(&self, ctx: &Context, setter: &ParamSetter) {
        let mut audio_thread_messages = self.message_sender.lock();

        #[allow(unused_must_use)]
        {
            audio_thread_messages.1.pop();
        }

        for (node_index, node_params) in self.graph.borrow().iter().enumerate() {
            Window::new(node_index.to_string())
                .fixed_size((400., 500.))
                .show(ctx, |ui| {
                    node_params.ui(node_index, ui, setter);
                });
        }
    }

    pub fn insert_top_level_node(&self, node: Arc<dyn NodeParameters>) {
        let mut map = self.node_count_per_type.borrow_mut();
        let id = node.type_id();

        *map.entry(id).or_insert(0) += 1;

        self.graph.borrow_mut().top_level_insert(node);
    }

    pub fn build_audio_graph(&self) -> ProcessSchedule {
        let graph = self.graph.borrow();
        let mut schedule = ProcessSchedule::default();

        for (node, edges) in graph.iter().zip(graph.edges().iter()) {
            schedule.push(node.clone().processor(), edges.clone());
        }

        schedule
    }
}
