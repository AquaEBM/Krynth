mod dsp;
mod gui;

use dsp::ProcessSchedule;
use super::*;

use plugin_util::dsp::graph::AudioGraph;
use std::{any::TypeId, collections::HashMap};

#[derive(Params)]
pub struct SeenthParams {
    editor_state: Arc<EguiState>,
    message_sender: Option<Mutex<(Producer<ProcessSchedule>, Consumer<ProcessSchedule>)>>,
    graph: AtomicRefCell<AudioGraph<Arc<dyn SeenthNode>>>,
    node_count_per_type: AtomicRefCell<HashMap<TypeId, usize>>,
}

impl Default for SeenthParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1140, 560),
            message_sender: None,
            graph: Default::default(),
            node_count_per_type: Default::default(),
        }
    }
}

impl SeenthParams {
    fn schedule(self: Arc<Self>) -> ProcessSchedule {
        ProcessSchedule::default()
    }
}
