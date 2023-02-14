#![feature(core_intrinsics, array_chunks, trait_upcasting)]

mod dsp;
mod params;

use arrayvec::ArrayVec;
use dsp::{*, wavetable_osc::WTOsc};
use params::{KrynthParams, AudioGraphEvent};
use rtrb::{Consumer, RingBuffer};
use std::{thread, time::Duration, sync::Arc, any::Any};

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui::{SidePanel, panel::Side}};

use plugin_util::{dsp::{processor::{Processor, ProcessSchedule}, sample::StereoSample}, util::Permute};

use crate::params::{NodeParameters, wt_osc::WTOscParams};

const MAX_POLYPHONY: usize = 16;

pub struct Krynth {
    voice_handler: ArrayVec<u8, MAX_POLYPHONY>,
    schedule: ProcessSchedule,
    params: Arc<KrynthParams>,
    gui_thread_messages: Consumer<AudioGraphEvent>,
}

impl Default for Krynth {
    fn default() -> Self {

        let (producer, consumer) = RingBuffer::new(32);

        Self {
            voice_handler: Default::default(),
            schedule: Default::default(),
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

            SidePanel::new(Side::Left, "banana").show(ctx, |ui| {

                ui.add_space(60.);

                if ui.button("new WTOsc").clicked() {
                    params.insert_top_level_node(Arc::new(WTOscParams::new(&params.global_params)));
                }
            });

            // Gross workaround for vsync not working.
            thread::sleep(Duration::from_secs_f64((FPS * 2.).recip()));
        })
    }

    fn initialize(
        &mut self,
        _bus_config: &BusConfig,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {

        self.params.build_audio_graph(&mut self.schedule);

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,

    ) -> ProcessStatus {

        let mut next_event = context.next_event();

        while let Ok(event) = self.gui_thread_messages.pop() {
            match event {

                // None of these actions are realtime safe for now (allocations, freeing...)

                AudioGraphEvent::Connect(from, to) => self.schedule.edges[from].push(to),

                AudioGraphEvent::Reschedule(mut permutation) => self.schedule.permute(&mut permutation),

                AudioGraphEvent::PushNode(node) => self.schedule.push(node, vec![]),

                AudioGraphEvent::SetWaveTable(index, wavetables) => {

                    let any = self.schedule[index].processor.as_mut() as &mut dyn Any;
                    let wt_osc: &mut WTOsc = any.downcast_mut().expect("this node is not a WTOsc");
                    wt_osc.wavetables = wavetables;
                },
            }
        }

        for (i, sample) in buffer.iter_samples().enumerate() {

            while let Some(event) = next_event {
                if event.timing() > i as u32 { break }

                match event {

                    NoteEvent::NoteOn { note, .. } => {
                        if let Ok(()) = self.voice_handler.try_push(note) {
                            self.schedule.add_voice(
                                util::midi_note_to_freq(note) / context.transport().sample_rate
                            )
                        };
                    },

                    NoteEvent::NoteOff { note, .. } => {

                        for (i, &id) in self.voice_handler.iter().enumerate() {
                            if note == id {
                                self.voice_handler.swap_remove(i);
                                self.schedule.remove_voice(i);
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