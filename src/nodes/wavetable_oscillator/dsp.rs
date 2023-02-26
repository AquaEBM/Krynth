use super::{
    wavetable::{BandlimitedWaveTables, PHASE_RANGE},
    WTOscParams, *,
};

use std::{iter, ops::Add};

use arrayvec::ArrayVec;
use plugin_util::dsp::sample::{StereoSample, ZERO_SAMPLE};
use rand::random;

const MAX_UNISON: usize = 16;

struct WTOscModValues {
    level: StereoSample,
    pan: StereoSample,
    num_unison_voices: [usize; 2],
    frame: [usize; 2],
    detune_range: StereoSample,
    detune: StereoSample,
    stereo_pos: StereoSample,
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
            num_unison_voices: [unison_l as usize, unison_r as usize],
            frame: [frame_l as usize, frame_r as usize],
            detune_range: self.detune_range.get_value(voice_idx).into(),
            detune: self.detune.get_value(voice_idx).into(),
            stereo_pos,
        }
    }
}

/// Describes a wavetable oscillator
#[derive(Default)]
struct Oscillator {
    pub phase: StereoSample,
    pub phase_delta: StereoSample,
}

impl Oscillator {
    fn new(phase: f32) -> Self {
        Self {
            phase: StereoSample::splat(phase),
            ..Default::default()
        }
    }

    #[inline]
    fn get_sample_from_table(
        &self,
        table: &BandlimitedWaveTables,
        frame: [usize; 2],
    ) -> StereoSample {
        StereoSample {
            l: table.get_sample(self.phase.l, frame[0], self.phase_delta.l),
            r: table.get_sample(self.phase.r, frame[1], self.phase_delta.r),
        }
    }

    #[inline]
    fn update_phase(&mut self, phase_delta: StereoSample) {
        self.phase_delta = phase_delta;
        self.phase = (self.phase + self.phase_delta) % StereoSample::splat(PHASE_RANGE);
    }
}

#[derive(Default)]
struct WTOscVoice {
    base_phase_delta: StereoSample,
    inv_num_steps: f32, // -2. / (self.oscillators.len() - 1)
    oscillators: ArrayVec<Oscillator, MAX_UNISON>,
}

impl WTOscVoice {
    fn new(base_phase_delta: f32) -> Self {
        Self {
            oscillators: Default::default(),
            base_phase_delta: base_phase_delta.into(),
            ..Default::default()
        }
    }

    #[inline]
    fn update_num_unison_voices(&mut self, [new, _]: [usize; 2]) {
        // TODO?: You can't really stereo modulate the number of unison voices
        // (or can you?), so, for now, just don't, and use only the left value.

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

        self.inv_num_steps = -2. / (new - 1) as f32;
    }

    #[inline]
    fn update_phases(&mut self, detune_range: StereoSample) {
        let odd = self.oscillators.len() & 1;
        if odd == 1 {
            self.oscillators[0].update_phase(self.base_phase_delta);
        }

        let mut tune = detune_range.semitones();
        let tune_delta = (detune_range * self.inv_num_steps).semitones();

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
        frame: [usize; 2],
        detune: StereoSample,
    ) -> StereoSample {
        let pan = (detune * StereoSample::splat(0.5)).sqrt();

        let odd = self.oscillators.len() & 1;
        let accumulator = if odd == 0 {
            ZERO_SAMPLE
        } else {
            self.oscillators[0].get_sample_from_table(table, frame)
        };

        self.oscillators[odd..]
            .array_chunks()
            .map(|[voice1, voice2]| {
                let y1 = voice1.get_sample_from_table(table, frame);
                let y2 = voice2.get_sample_from_table(table, frame);
                y1 * pan + y2.mul_rev(pan)
            })
            .fold(accumulator, Add::add)
    }

    #[inline]
    fn process(&mut self, params: WTOscModValues, table: &BandlimitedWaveTables) -> StereoSample {
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
    fn process(&mut self, inputs: &mut [StereoSample]) {
        let params = self.params.as_ref();
        for (i, (input, voice)) in inputs.iter_mut().zip(self.voices.iter_mut()).enumerate() {
            *input = voice.process(params.modulated(i), &self.wavetables);
        }
    }
}