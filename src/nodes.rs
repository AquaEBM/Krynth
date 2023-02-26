use atomic_refcell::AtomicRefCell;
use nih_plug::{formatters::*, prelude::*};
use nih_plug_egui::{egui::*, EguiState};
use parking_lot::Mutex;
use arrayvec::ArrayVec;
use plugin_util::{
    dsp::sample::*,
    gui::widgets::*,
    parameter::{Modulable, ParamHandle},
};
use rtrb::{Consumer, Producer};
use std::sync::Arc;

use std::any::Any;

pub trait Processor: Any {

    fn add_voice(&mut self, norm_freq: f32);

    fn remove_voice(&mut self, voice_idx: usize);

    fn process(&mut self, inputs: &mut [StereoSample]);
}

type ProcessNode = dyn Processor + Send;

trait KrynthNode: Params + Any {
    fn type_name(&self) -> String;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor_node(self: Arc<Self>) -> Box<ProcessNode>;
}

trait KrynthStandAlonePlugin: KrynthNode + Default {
    type Processor: Processor;

    fn processor(self: Arc<Self>) -> Self::Processor;
}

const MAX_POLYPHONY: usize = 16;

type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

mod audio_graph;
mod wavetable_oscillator;
