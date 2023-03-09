use hound::{SampleFormat, WavReader};
use plugin_util::dsp::lerp_table;
use realfft::num_complex::Complex32;
use std::path::Path;

use super::*;

pub const fn empty_wavetable() -> WaveTable {
    [[0.; WAVE_FRAME_LEN + 1]; FRAMES_PER_WT]
}

pub const PHASE_RANGE: f32 = WAVE_FRAME_LEN as f32;
const NUM_WAVETABLES: usize = WAVE_FRAME_LEN.ilog2() as usize + 1;
const SPECTRUM_SIZE: usize = WAVE_FRAME_LEN / 2 + 1;

type Spectrum = [Complex32; SPECTRUM_SIZE];

pub(super) fn write_wavetable_from_file(path: impl AsRef<Path>, wt: &mut WaveTable) {
    let reader = WavReader::open(path).unwrap();
    let spec = reader.spec();

    assert_eq!(spec.channels, 1, "only mono supported");
    assert_eq!(
        spec.sample_format,
        SampleFormat::Float,
        "Only FP samples supported"
    );

    let mut samples = reader.into_samples::<f32>().map(Result::unwrap);

    assert_eq!(
        samples.len(),
        WAVE_FRAME_LEN * FRAMES_PER_WT,
        "invalid wavetable size, wavetable size must be {WAVE_FRAME_LEN} x {FRAMES_PER_WT} samples"
    );

    for buffer in wt.iter_mut() {
        let (wrap_around, window) = buffer.split_last_mut().unwrap();

        window
            .iter_mut()
            .zip(samples.by_ref())
            .for_each(|(output, input)| *output = input);

        *wrap_around = window[0];
    }
}

/// Bandlimited wavetable data structure
#[derive(Default)]
pub(super) struct BandlimitedWaveTables {
    data: Option<Box<[WaveTable ; NUM_WAVETABLES]>>,
}

impl BandlimitedWaveTables {
    pub fn set_wavetable(&mut self, wt: &WaveTable) {

        let spectra = spectra_from_wavetable(wt);

        self.data.replace(bandlimited_wavetables(*wt, spectra.as_ref()));
    }

    /// Resample the value at the given `frame` and `phase` `phase_delta` is
    /// the magnitude of the last phase increment of the oscillator and is used to determine
    /// which bandlimited copy of the wavetable to resample from, reducing aliasing.
    #[inline]
    pub fn get_sample(&self, phase: f32, frame: usize, phase_delta: f32) -> f32 {
        let index = 126usize.saturating_sub(
            (phase_delta / PHASE_RANGE)
                .to_bits() as usize >> 23
        );

        unsafe {
            // omit bounds checks
            lerp_table(
                self.data.as_ref().unwrap_unchecked()
                    .get_unchecked(index.min(NUM_WAVETABLES - 1))
                    .get_unchecked(frame)
                    .as_slice(),
                phase,
            )
        }
    }
}

/// Computes the frequency spectra of the wavetable. It is the
/// caller's responsibiliy to pass in non-aliased wavetables.
pub fn spectra_from_wavetable(wavetable: &WaveTable) -> Box<[Spectrum ; FRAMES_PER_WT]> {
    let mut r2c = realfft::RealFftPlanner::<f32>::new();
    let fft = r2c.plan_fft_forward(WAVE_FRAME_LEN);

    let mut scratch = fft.make_scratch_vec();

    let mut spectra = Vec::<Spectrum>::with_capacity(FRAMES_PER_WT);
    #[allow(clippy::uninit_vec)]
    unsafe { spectra.set_len(FRAMES_PER_WT) };

    let mut input = [0. ; WAVE_FRAME_LEN];

    for (spectrum, window) in spectra.iter_mut().zip(wavetable.iter()) {
        input.copy_from_slice(&window[..WAVE_FRAME_LEN] /* all but the last element */);

        fft.process_with_scratch(&mut input, spectrum, &mut scratch)
            .expect("wrong buffer sizes");

        // remove DC
        spectrum[0].re = 0.;
    }
    spectra.try_into().unwrap()
}

/// Computes bandlimited copies of the wavetable with the given
/// frequecncy spectra. The first will be DC. The second will have one
/// harmonic, the third 2, the forth 4, the fifth 8, etc...
pub fn bandlimited_wavetables(
    wavetable: WaveTable,
    spectra: &[Spectrum; FRAMES_PER_WT],
) -> Box<[WaveTable ; NUM_WAVETABLES]> {

    let mut output = Vec::with_capacity(NUM_WAVETABLES);
    output.push(empty_wavetable());
    unsafe { output.set_len(NUM_WAVETABLES) };

    let (full_wt, bandlimited_versions) = output.split_last_mut().unwrap();
    *full_wt = wavetable;

    let mut c2r = realfft::RealFftPlanner::<f32>::new();
    let fft = c2r.plan_fft_inverse(WAVE_FRAME_LEN);
    let mut scratch = [Complex32::new(0., 0.); SPECTRUM_SIZE];
    let mut input = scratch;

    let mut partials = 1;

    for terrain in bandlimited_versions[1..].iter_mut() {
        let bins = partials + 1;

        for (spectrum, table) in spectra.iter().zip(terrain.iter_mut()) {
            let (pass_band, stop_band) = input.split_at_mut(bins);
            pass_band.copy_from_slice(&spectrum[..bins]);
            stop_band.fill(Complex32::new(0., 0.));

            let (wrap_around, window) = table.split_last_mut().unwrap();

            fft.process_with_scratch(&mut input, window, &mut scratch)
                .unwrap();

            let normalize = (window.len() * 2) as f32;
            window.iter_mut().for_each(|sample| *sample /= normalize);
            *wrap_around = window[0];
        }
        partials *= 2;
    }
    output.try_into().unwrap()
}
