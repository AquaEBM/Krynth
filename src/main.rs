use nih_plug::prelude::nih_export_standalone;
use synth::{SeenthPlugin, nodes};

fn main() {
    nih_export_standalone::<SeenthPlugin<nodes::wavetable_oscillator::WTOscParams>>();
}
