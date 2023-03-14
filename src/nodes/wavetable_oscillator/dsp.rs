use super::{
    wavetable::{BandlimitedWaveTables, PHASE_RANGE},
    WTOscParams, *,
};

use std::{iter, ops::Add, simd::{usizex2, StdFloat, simd_swizzle}};

use arrayvec::ArrayVec;
use plugin_util::dsp::semitones;
use rand::random;

const MAX_UNISON: usize = 16;

struct WTOscModValues {
    level: f32x2,
    pan: f32x2,
    num_unison_voices: usizex2,
    frame: usizex2,
    detune_range: f32x2,
    detune: f32x2,
    stereo_pos: f32x2,
}

impl WTOscParams {
    fn modulated(&self, voice_idx: usize) -> WTOscModValues {
        let [lvl_l, lvl_r] = self.detune.get_value(voice_idx);
        let [pan_l, pan_r] = self.pan.get_value(voice_idx);

        let stereo_pos = [1. - lvl_l, lvl_r].into();
        let pan = [1. - pan_l, pan_r].into();

        let [frame_l, frame_r] = self.frame.get_value(voice_idx);
        let [unison_l, unison_r] = self.num_unison_voices.get_value(voice_idx);

        WTOscModValues {
            level: self.level.get_value(voice_idx).into(),
            pan,
            num_unison_voices: [unison_l as usize, unison_r as usize].into(),
            frame: [frame_l as usize, frame_r as usize].into(),
            detune_range: self.detune_range.get_value(voice_idx).into(),
            detune: self.detune.get_value(voice_idx).into(),
            stereo_pos,
        }
    }
}

/// Describes a wavetable oscillator
#[derive(Default)]
struct Oscillator {
    pub phase: f32x2,
    pub phase_delta: f32x2,
}

impl Oscillator {
    fn new(phase: f32) -> Self {
        Self {
            phase: f32x2::splat(phase),
            ..Default::default()
        }
    }

    #[inline]
    fn get_sample_from_table(
        &self,
        table: &BandlimitedWaveTables,
        frame: usizex2,
    ) -> f32x2 {
        table.get_sample(self.phase, frame, self.phase_delta)
    }

    #[inline]
    fn update_phase(&mut self, phase_delta: f32x2) {
        self.phase_delta = phase_delta;
        self.phase = (self.phase + self.phase_delta) % f32x2::splat(PHASE_RANGE);
    }
}

#[derive(Default)]
struct WTOscVoice {
    base_phase_delta: f32x2,
    inv_num_steps: f32x2, // -2. / (self.oscillators.len() - 1)
    oscillators: ArrayVec<Oscillator, MAX_UNISON>,
}

impl WTOscVoice {
    fn new(base_phase_delta: f32) -> Self {
        Self {
            oscillators: Default::default(),
            base_phase_delta: f32x2::splat(base_phase_delta),
            ..Default::default()
        }
    }

    #[inline]
    fn update_num_unison_voices(&mut self, num_voices: usizex2) {
        // TODO?: You can't really stereo modulate the number of unison voices
        // (or can you?), so, for now, just don't, and use only the left value.

        let new = num_voices.to_array()[0];

        let current = self.oscillators.len();
        if new == current {
            return;
        }

        // TODO: use a thread_rng and pass it in here.

        if new > current {
            self.oscillators.extend(
                iter::repeat_with(|| Oscillator::new(random::<f32>() * PHASE_RANGE))
                    .take(new - current),
            );
        } else {
            self.oscillators.truncate(new);
        }

        self.inv_num_steps = f32x2::splat(-2.) / (num_voices - usizex2::splat(1)).cast();
    }

    #[inline]
    fn update_phases(&mut self, detune_range: f32x2) {
        let odd = self.oscillators.len() & 1;
        if odd == 1 {
            self.oscillators[0].update_phase(self.base_phase_delta);
        }
        
        fn x2semitones(val: f32x2) -> f32x2 {
            let &[l, r] = val.as_array();
            f32x2::from_array([semitones(l), semitones(r)])
        }

        let mut tune = x2semitones(detune_range);
        let tune_delta = x2semitones(detune_range * self.inv_num_steps);

        self.oscillators[odd..]
            .array_chunks_mut()
            .for_each(|[osc1, osc2]| {
                osc1.update_phase(self.base_phase_delta * tune);
                osc2.update_phase(self.base_phase_delta / tune);
                tune = tune * tune_delta;
            });
    }

    #[inline]
    fn get_sample_from_table(
        &self,
        table: &BandlimitedWaveTables,
        frame: usizex2,
        detune: f32x2,
    ) -> f32x2 {
        let pan = (detune * f32x2::splat(0.5)).sqrt();
        let rev_pan = simd_swizzle!(pan, [1, 0]);

        let odd = self.oscillators.len() & 1;
        let accumulator = if odd == 0 {
            f32x2::from_array([0., 0.])
        } else {
            self.oscillators[0].get_sample_from_table(table, frame)
        };

        self.oscillators[odd..]
            .array_chunks()
            .map(|[voice1, voice2]| {
                let y1 = voice1.get_sample_from_table(table, frame);
                let y2 = voice2.get_sample_from_table(table, frame);
                y1 * pan + y2 * rev_pan
            })
            .fold(accumulator, Add::add)
    }

    #[inline]
    fn process(&mut self, params: WTOscModValues, table: &BandlimitedWaveTables) -> f32x2 {
        self.update_num_unison_voices(params.num_unison_voices);
        self.update_phases(params.detune_range * params.detune);

        let sample = self.get_sample_from_table(table, params.frame, params.stereo_pos);

        sample * params.level * params.pan.sqrt()
    }
}

pub struct WTOsc {
    params: Arc<WTOscParams>,
    wavetables: BandlimitedWaveTables,
    voices: ArrayVec<WTOscVoice, MAX_POLYPHONY>,
}

impl WTOsc {
    pub(super) fn new(params: Arc<WTOscParams>) -> Self {
        Self {
            wavetables: Default::default(),
            params,
            voices: Default::default(),
        }
    }
}

impl Processor for WTOsc {
    fn add_voice(&mut self, norm_freq: f32) {
        let phase_delta = norm_freq * PHASE_RANGE;
        self.voices.push(WTOscVoice::new(phase_delta));
    }

    fn remove_voice(&mut self, voice_idx: usize) {
        self.voices.swap_remove(voice_idx);
    }

    #[inline]
    /// pre-condition: inputs.len() = number of voices in self
    fn process(&mut self, _input: f32x2, voice_idx: usize, _editor_open: bool) -> f32x2 {

        self.voices[voice_idx].process(
            self.params.modulated(voice_idx),
            &self.wavetables
        )
    }

    fn initialize(&mut self, _sample_rate: f32) -> (bool, u32) {
        self.wavetables.set_wavetable(
            self.params.wavetable.borrow().as_slice().try_into().unwrap()
        );
        (true, 0)
    }

    fn reset(&mut self) {
        self.voices.clear()
    }
}

impl SeenthStandAlonePlugin for WTOscParams {
    type Processor = WTOsc;

    fn processor(self: Arc<Self>) -> Self::Processor {
        self.oscillator()
    }

    fn editor_state(&self) -> Arc<EguiState> {
        EguiState::from_size(1000, 200)
    }
}
