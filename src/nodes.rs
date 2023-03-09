use atomic_refcell::AtomicRefCell;
pub use nih_plug::{formatters::*, prelude::*};
pub use nih_plug_egui::{
    egui::*,
    EguiState,
    create_egui_editor
};

use parking_lot::Mutex;
use arrayvec::ArrayVec;
use plugin_util::{
    dsp::sample::*,
    gui::widgets::*,
    parameter::{Modulable, ParamHandle},
};

use rtrb::{Consumer, Producer};
pub use std::sync::Arc;

use std::any::Any;

pub(crate) trait Processor {

    fn add_voice(&mut self, norm_freq: f32);

    fn remove_voice(&mut self, voice_idx: usize);

    fn process(&mut self, voice_index: usize, inputs: &mut StereoSample);

    fn initialize(&mut self) -> (bool, u32);

    fn reset(&mut self);
}

type ProcessNode = dyn Processor + Send;

pub(crate) trait SeenthNode: Params + Any {
    fn type_name(&self) -> &'static str;

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response;

    fn processor_node(self: Arc<Self>) -> Box<ProcessNode>;

    fn ports(&self) -> AudioIOLayout {
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        }
    }
}

pub(crate) trait SeenthStandAlonePlugin: SeenthNode + Default {
    type Processor: Processor + Send;

    /// Audio layout for this plugin, all frame sizes must be set to 2 (stereo),
    /// or 0 (None) on a main input/output to indicate that there is no such port
    const PORTS: AudioIOLayout = AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    };

    fn processor(self: Arc<Self>) -> Self::Processor;
    fn editor_state(&self) -> Arc<EguiState>;
}

pub const MAX_POLYPHONY: usize = 16;

type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}

mod audio_graph;
mod wavetable_oscillator;