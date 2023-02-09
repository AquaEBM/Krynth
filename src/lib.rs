#![feature(core_intrinsics, array_chunks)]

mod dsp;
mod params;

use arrayvec::ArrayVec;
use dsp::*;
use params::KrynthParams;
use rtrb::{Consumer, RingBuffer};
use std::{thread, time::Duration, sync::Arc};

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor};

use plugin_util::dsp::{processor::{Processor, ProcessSchedule}, sample::StereoSample};

use crate::params::NodeParemeters;

pub const NUM_OSCS: usize = 3;
pub const FRAMES_PER_WT: usize = 256;
pub const WAVE_FRAME_LEN: usize = 2048;
pub const NUM_WAVETABLES: usize = WAVE_FRAME_LEN.ilog2() as usize + 1;
pub const PHASE_RANGE: f32 = WAVE_FRAME_LEN as f32;
pub const MAX_POLYPHONY: usize = 16;
pub const MAX_UNISON: usize = 16;

#[non_exhaustive]
pub enum AudioGraphEvent {
    UpdateAudioGraph(ProcessSchedule),
}

pub struct Krynth {
    voice_handler: ArrayVec<u8, MAX_POLYPHONY>,
    graph: ProcessSchedule,
    params: Arc<KrynthParams>,
    gui_thread_messages: Consumer<AudioGraphEvent>,
}

impl Default for Krynth {
    fn default() -> Self {

        let (producer, consumer) = RingBuffer::new(32);

        Self {
            voice_handler: Default::default(),
            graph: Default::default(),
            params: Arc::new(KrynthParams::new(producer)),
            gui_thread_messages: consumer,
        }
    }
}

impl Plugin for Krynth {

    const NAME: &'static str = "Senpaaiiiiii";
    const VENDOR: &'static str = "AquaEBM";
    const URL: &'static str = "lol";
    const EMAIL: &'static str = "AquaEBM@gmail.com";
    const VERSION: &'static str = "0.6.9";

    const DEFAULT_INPUT_CHANNELS: u32 = 0;
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    // Do not expose our plugin's parameters as part of the param map, since plugin APIs
    // do not really support dynamic adding/removing of parameters, this breaks automation and preset saving
    // TODO: Is there a way Around this?.

    fn params(&self) -> Arc<dyn Params> {

        #[derive(Params)]
        #[allow(dead_code)]
        struct DummyParams {
            named_field: (),
        }

        Arc::new(DummyParams { named_field: () })
    }

    fn editor(&self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {

        const FPS: f64 = 48.;

        let params = self.params.clone();
        create_egui_editor(params.editor_state.clone(), (), |_, _| {}, move |ctx, setter, _| {

            params.ui(ctx, setter);

            // Gross workaround for vsync not working.
            thread::sleep(Duration::from_secs_f64((FPS * 1.5).recip()));
        })
    }

    fn initialize(
        &mut self,
        _bus_config: &BusConfig,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {

        self.graph = self.params.build_audio_graph();

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,

    ) -> ProcessStatus {

        let mut next_event = context.next_event();

        match self.gui_thread_messages.pop() {
            Ok(AudioGraphEvent::UpdateAudioGraph(new_graph)) => self.graph = new_graph,
            _ => (),
        }

        for (i, sample) in buffer.iter_samples().enumerate() {

            while let Some(event) = next_event {
                if event.timing() > i as u32 { break }

                match event {

                    NoteEvent::NoteOn { note, .. } => {
                        match self.voice_handler.try_push(note) {
                            Ok(()) => self.graph.add_voice(
                                util::midi_note_to_freq(note) / context.transport().sample_rate
                            ),
                            _ => ()
                        };
                    },

                    NoteEvent::NoteOff { note, .. } => {

                        for (i, &id) in self.voice_handler.iter().enumerate() {
                            if note == id {
                                self.voice_handler.swap_remove(i);
                                self.graph.remove_voice(i);
                                break;
                            }
                        }
                    },
                    _ => (),
                }
                next_event = context.next_event();
            }

            let mut output = StereoSample::splat(0.);

            output = output * StereoSample::splat(0.5);

            let mut in_samples = sample.into_iter();
            unsafe {
                *in_samples.next().unwrap_unchecked() = output.l;
                *in_samples.next().unwrap_unchecked() = output.r;
            }
        }
        ProcessStatus::Normal
    }
}

use Vst3SubCategory::*;

impl Vst3Plugin for Krynth {

    const VST3_CLASS_ID: [u8; 16] = *b"bassriddimriddim";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Instrument, Stereo];
}

nih_export_vst3!(Krynth);