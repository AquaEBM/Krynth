use atomic_refcell::AtomicRefCell;
use parking_lot::Mutex;
use plugin_util::{
    dsp::{
        graph::AudioGraph,
        processor::{ProcessSchedule, Processor},
    },
};

use nih_plug::prelude::*;
use nih_plug_egui::{
    egui::{panel::Side, CentralPanel, Response, SidePanel, Ui, Window},
    EguiState,
};

use rtrb::{Consumer, Producer};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use crate::params::wt_osc::WTOscParams;

pub type ProcessNode = dyn Processor + Send;

pub trait NodeParameters: Params + Any {

    fn new() -> Self
    where
        Self: Sized;

    fn type_name(&self) -> String;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;
}

pub trait ProcessorFactory: NodeParameters {
    type Processor: Processor;

    fn processor(self: Arc<Self>) -> Self::Processor;
}

pub trait ProcessorFactoryDyn: NodeParameters {
    fn processor_dyn(self: Arc<Self>) -> Box<ProcessNode>;
}

#[derive(Params)]
pub struct KrynthParams {
    pub editor_state: Arc<EguiState>,
    message_sender: Option<Mutex<(Producer<ProcessSchedule>, Consumer<ProcessSchedule>)>>,
    graph: AtomicRefCell<AudioGraph<Arc<dyn ProcessorFactoryDyn>>>,
    node_count_per_type: AtomicRefCell<HashMap<TypeId, usize>>,
}

impl NodeParameters for KrynthParams {

    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            editor_state: EguiState::from_size(1140, 560),
            message_sender: None,
            graph: Default::default(),
            node_count_per_type: Default::default(),
        }
    }

    fn type_name(&self) -> String { "Synth".into() }

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response {

        SidePanel::new(Side::Left, "banana").show_inside(ui, |ui| {
            ui.add_space(40.);

            if ui.button("new WTOsc").clicked() {
                self.insert_top_level_node(Arc::new(WTOscParams::new()));
            }

        }).response | CentralPanel::default().show_inside(ui, |ui| {
            let mut audio_thread_messages = self.message_sender.as_ref().unwrap().lock();

            #[allow(unused_must_use)]
            {
                audio_thread_messages.1.pop();
            }

            for (node_index, node_params) in self.graph.borrow()
                .iter()
                .enumerate()
            {
                Window::new(node_index.to_string())
                    .fixed_size((400., 500.))
                    .show(ui.ctx(), |ui| {
                        node_params.ui(ui, setter);
                    });
            }
        }).response
    }
}

impl KrynthParams {

    pub fn insert_top_level_node(&self, node: Arc<dyn ProcessorFactoryDyn>) {
        let mut map = self.node_count_per_type.borrow_mut();
        let id = node.type_id();

        *map.entry(id).or_insert(0) += 1;

        self.graph.borrow_mut().top_level_insert(node);
    }

    pub fn build_audio_graph(&self) -> ProcessSchedule {
        let graph = self.graph.borrow();
        let mut schedule = ProcessSchedule::default();

        for (node, edges) in graph.iter().zip(graph.edges().iter()) {
            schedule.push(node.clone().processor_dyn(), edges.clone());
        }

        schedule
    }
}
