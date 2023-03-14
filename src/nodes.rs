use atomic_refcell::AtomicRefCell;
pub use nih_plug::{formatters::*, prelude::*};
pub use nih_plug_egui::{
    egui::*,
    EguiState,
    create_egui_editor
};

use parking_lot::Mutex;
use plugin_util::{
    gui::widgets::*,
    parameter::{Modulable, ParamHandle},
};

use rtrb::{Consumer, Producer};
pub use std::sync::Arc;

use std::{any::Any, simd::f32x2};

pub(crate) trait Processor {

    fn add_voice(&mut self, norm_freq: f32);

    fn remove_voice(&mut self, voice_idx: usize);

    fn process(&mut self, input: f32x2, voice_idx: usize, editor_open: bool) -> f32x2;

    fn initialize(&mut self, sample_rate: f32) -> (bool, u32);

    fn reset(&mut self);
}

type ProcessNode = dyn Processor + Send;

pub(crate) trait SeenthNode: Params + Any {
    fn type_name(&self) -> &'static str;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor_node(self: Arc<Self>) -> Box<ProcessNode>;
}

pub(crate) trait SeenthStandAlonePlugin: SeenthNode + Default {
    type Processor: Processor + Send;

    fn processor(self: Arc<Self>) -> Self::Processor;
    fn editor_state(&self) -> Arc<EguiState>;
}

pub const MAX_POLYPHONY: usize = 16;

type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

pub mod audio_graph;
pub mod wavetable_oscillator;