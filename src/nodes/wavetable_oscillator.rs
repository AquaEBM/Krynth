mod dsp;
mod gui;
mod wavetable;

use super::*;
use dsp::WTOsc;

const FRAMES_PER_WT: usize = 256;
const WAVE_FRAME_LEN: usize = 2048;

type WaveFrame = [f32; WAVE_FRAME_LEN + 1];
type WaveTable = [WaveFrame ; FRAMES_PER_WT];

#[derive(Params)]
pub struct WTOscParams {
    #[id = "level"]
    level: ModulableParamHandle<FloatParam>,
    #[id = "pan"]
    pan: Arc<ModulableParamHandle<FloatParam>>,
    #[id = "unison"]
    num_unison_voices: ModulableParamHandle<IntParam>,
    #[id = "frame"]
    frame: ModulableParamHandle<IntParam>,
    #[id = "det_range"]
    detune_range: ModulableParamHandle<FloatParam>,
    #[id = "detune"]
    detune: ModulableParamHandle<FloatParam>,
    #[persist = "wt_name"]
    wt_name: AtomicRefCell<String>,
    wavetable: AtomicRefCell<Vec<WaveFrame>>,
}

impl Default for WTOscParams {
    fn default() -> Self {
        Self {
            level: modulable(
                FloatParam::new(
                    "Level",
                    0.5,
                    FloatRange::Skewed {
                        min: 0.,
                        max: 1.,
                        factor: 0.5,
                    },
                )
                .with_value_to_string(v2s_f32_rounded(3)),
            ),

            pan: Arc::new(modulable(
                FloatParam::new("Pan", 0.5, FloatRange::Linear { min: 0., max: 1. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            )),

            num_unison_voices: modulable(IntParam::new(
                "Unison",
                1,
                IntRange::Linear { min: 1, max: 16 },
            )),

            frame: modulable(IntParam::new(
                "Frame",
                0,
                IntRange::Linear {
                    min: 0,
                    max: FRAMES_PER_WT as i32 - 1,
                },
            )),

            detune_range: modulable(
                FloatParam::new("Spread", 2., FloatRange::Linear { min: 0., max: 48. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            ),

            detune: modulable(
                FloatParam::new("Detune", 0.2, FloatRange::Linear { min: 0., max: 1. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            ),

            wt_name: AtomicRefCell::new("Basic Shapes".into()),

            wavetable: AtomicRefCell::new(Vec::new()),
        }
    }
}

impl WTOscParams {
    fn oscillator(self: Arc<Self>) -> WTOsc {
        WTOsc::new(self)
    }
}
