pub mod distortion;
pub mod wt_osc;
pub mod audio_graph;

use plugin_util::parameter::Modulable;
use nih_plug::prelude::Param;

use crate::MAX_POLYPHONY;

pub const WAVETABLE_FOLDER_PATH: &str =
    "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

pub type ModulableParamHandle<T> = Modulable<T, MAX_POLYPHONY>;

pub fn modulable<T: Param>(param: T) -> ModulableParamHandle<T> {
    Modulable::from(param)
}