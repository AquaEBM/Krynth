use std::{ops::Deref, array};
use realfft::num_complex::Complex32;
use plugin_util::{dsp::lerp_table, mul};
use crate::{PHASE_RANGE, WAVE_FRAME_LEN, FRAMES_PER_WT, NUM_WAVETABLES};
use hound::{WavReader, SampleFormat};

pub type WaveFrame = Box<[f32 ; WAVE_FRAME_LEN + 1]>;
pub type WaveTable = [WaveFrame ; FRAMES_PER_WT];

pub fn empty_wavetable() -> WaveTable {
    array::from_fn(|_| Box::new([0. ; WAVE_FRAME_LEN + 1]))
}

/// Bandlimited wavetable data structure
pub struct BandlimitedWaveTables {
    data: [WaveTable ; NUM_WAVETABLES],
}

impl Deref for BandlimitedWaveTables {
    type Target = [WaveTable ; NUM_WAVETABLES];

    fn deref(&self) -> &Self::Target { &self.data }
}

impl Default for BandlimitedWaveTables {
    fn default() -> Self {
        Self { data: array::from_fn(|_| empty_wavetable()) }
    }
}

impl BandlimitedWaveTables {

    const LAST: usize = NUM_WAVETABLES - 1;

    pub fn from_file(path: String) -> Self {

        let reader = WavReader::open(path).unwrap();
        let spec = reader.spec();

        assert_eq!(spec.channels, 1, "only mono supported");
        assert_eq!(spec.sample_format, SampleFormat::Float, "Only FP samples supported");

        let mut samples = reader.into_samples::<f32>().map(Result::unwrap);

        assert_eq!(
            samples.len(),
            WAVE_FRAME_LEN * FRAMES_PER_WT,
            "invalid wavetable size, wavetable size must be {WAVE_FRAME_LEN} x {FRAMES_PER_WT} samples"
        );

        let samples_iter = samples.by_ref();

        let wavetable = array::from_fn( |_| {

            let mut buffer = Box::new([0. ; WAVE_FRAME_LEN + 1]);

            let (wrap_around, window) = buffer.split_last_mut().unwrap();

            window.fill_with(|| samples_iter.next().unwrap());

            *wrap_around = window[0];

            buffer
        });

        let spectra = spectra_from_wavetable(wavetable.clone());

        Self { data: bandlimited_wavetables(wavetable, spectra) }
    }

    /// Resample the value at the given `frame` and `phase` `phase_delta` is
    /// the magnitude of the last phase increment of the oscillator and is used to determine
    /// which bandlimited copy of the wavetable to resample from, hopefully reducing aliasing.
    #[inline]
    pub fn get_sample(&self, phase: f32, frame: usize, phase_delta: f32) -> f32 {
        const INV_PR: f32 = 1. / PHASE_RANGE;
        let index = 126usize.saturating_sub(mul(phase_delta, INV_PR).to_bits() as usize >> 23);
        // omit bounds checks
        unsafe {
            lerp_table(
                self.get_unchecked(index.min(Self::LAST)).get_unchecked(frame).as_slice(),
                phase
            )
        }
    }

    /// the last in the list of bandlimited wavetables,
    /// i.e the original, untouched, non-bandlimited version
    pub fn last_at(&self, frame: usize) -> &[f32] {
        self[Self::LAST][frame].split_last().unwrap().1
    }
}

/// Computes the frequency spectra of the wavetable. Panics when the waveforms
/// are of different for lengths. Assumes the waveforms are not aliased. It is,
/// therefore, the caller's responsibiliy to pass in non-aliased wavetables.
pub fn spectra_from_wavetable(mut wavetable: WaveTable) -> Box<[Box<[Complex32]>]> {

    let mut r2c = realfft::RealFftPlanner::<f32>::new();
    let wt_len = wavetable[0].len() - 1;
    let fft = r2c.plan_fft_forward(wt_len);

    let mut scratch = fft.make_scratch_vec().into_boxed_slice();
    let mut spectra = vec![fft.make_output_vec().into_boxed_slice(); wavetable.len()].into_boxed_slice();

    for (spectrum, window) in spectra.iter_mut().zip(wavetable.iter_mut()) {

        fft.process_with_scratch(
            &mut window[..wt_len],
            spectrum,
            &mut scratch
        ).expect("wrong buffer sizes");

        // remove DC
        spectrum[0].re = 0.;
    }
    spectra
}

/// Computes bandlimited copies of the wavetable with the given
/// frequecncy spectra. The first will be DC. The second will have one harmonic,
/// the third 2, the forth 4, the fifth 8, etc...
pub fn bandlimited_wavetables(
    wavetable: WaveTable,
    spectra: Box<[Box<[Complex32]>]>,
) -> [WaveTable ; NUM_WAVETABLES] {

    let mut c2r = realfft::RealFftPlanner::<f32>::new();

    let wt_len = (spectra[0].len() - 1) * 2;
    let fft = c2r.plan_fft_inverse(wt_len);

    let mut scratch = fft.make_scratch_vec().into_boxed_slice();

    let num_frames = spectra.len();
    let mut band_lim_spectra = vec![fft.make_input_vec().into_boxed_slice() ; num_frames].into_boxed_slice();

    let mut output = array::from_fn(|_| empty_wavetable());

    *output.last_mut().unwrap() = wavetable;

    let mut partials = 1;

    for terrain in output[1..].iter_mut() {

        for (spectrum, bl_spectrum) in spectra.iter().zip(band_lim_spectra.iter_mut()) {

            let bins = partials + 1;
            let (pass_band, stop_band) = bl_spectrum.split_at_mut(bins);
            pass_band.copy_from_slice(&spectrum[..bins]);
            stop_band.fill(Complex32::new(0., 0.));
        }

        for (table, in_spectrum) in terrain.iter_mut().zip(band_lim_spectra.iter_mut()) {

            let (wrap_around, window) = table.split_last_mut().unwrap();

            fft.process_with_scratch(in_spectrum, window, &mut scratch).unwrap();

            let normalize = (window.len() * 2) as f32;
            window.iter_mut().for_each(|sample| *sample /= normalize);
            *wrap_around = window[0];
        }

        partials *= 2;
    }
    output
}