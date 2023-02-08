use std::ops::Deref;

use nih_plug_egui::egui::*;
use plot::*;
use crate::{params::{modulable, WAVETABLE_FOLDER_PATH}, NodeParemeters, params::ModulableParamHandle, dsp::wavetable_osc::WTOsc};
use plugin_util::{gui::widgets::*, parameter::ParamHandle, dsp::processor::Processor};
use nih_plug::prelude::ParamSetter;

use std::sync::Arc;
use nih_plug::{prelude::*, formatters::v2s_f32_rounded, params::persist};
use nih_plug_egui::egui::Response;
use crate::wavetable::BandlimitedWaveTables;
use parking_lot::RwLock;
use plugin_util::{*, dsp::sample::StereoSample};
use persist::PersistentField;

use atomic_refcell::AtomicRefCell;

use crate::FRAMES_PER_WT;

use super::KrynthParams;

#[derive(Params)]
pub struct WTOscParams {
    #[id = "level"]         pub level:             ModulableParamHandle<FloatParam>,
    #[id = "pan"]           pub pan:               ModulableParamHandle<FloatParam>,
    #[id = "unison"]        pub num_unison_voices: ModulableParamHandle<IntParam>,
    #[id = "frame"]         pub frame:             ModulableParamHandle<IntParam>,
    #[id = "det_range"]     pub detune_range:      ModulableParamHandle<FloatParam>,
    #[id = "detune"]        pub detune:            ModulableParamHandle<FloatParam>,
    #[persist = "wt_name"]  pub wt_name:           AtomicRefCell<String>,
                            pub wavetable:         RwLock<BandlimitedWaveTables>,
                            pub wt_list:           Arc<[String]>
}

impl Default for WTOscParams {
    fn default() -> Self {

        Self {

            level: modulable(
                FloatParam::new("Level", 0.5, FloatRange::Skewed { min: 0., max: 1., factor: 0.5 })
                    .with_value_to_string(v2s_f32_rounded(3))
            ),

            pan: modulable(
                FloatParam::new("Pan", 0.5, FloatRange::Linear { min: 0., max: 1. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            ),

            num_unison_voices: modulable(
                IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: 16 }),
            ),

            frame: modulable(
                IntParam::new("Frame", 0, IntRange::Linear  { min: 0, max: FRAMES_PER_WT  as i32 - 1}),
            ),

            detune_range: modulable(
                FloatParam::new("Spread", 2., FloatRange::Linear { min: 0., max: 48. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            ),

            detune: modulable(
                FloatParam::new("Detune", 0.2, FloatRange::Linear { min: 0., max: 1. })
                    .with_value_to_string(v2s_f32_rounded(3)),
            ),

            wt_name: AtomicRefCell::new("Basic Shapes".into()),

            wavetable: Default::default(),

            wt_list: Arc::new(["Basic Shapes".into()])
        }
    }
}

pub struct WTOscModValues {
    pub level: StereoSample,
    pub pan: StereoSample,
    pub num_unison_voices: [usize ; 2],
    pub frame: [usize ; 2],
    pub detune_range: StereoSample,
    pub detune: StereoSample,
    pub stereo_pos: StereoSample,
}

impl WTOscParams {

    pub fn new(global_params: &KrynthParams) -> Self {
        Self {
            wt_list: global_params.wt_list.clone(),
            ..Default::default()
        }
    }

    pub fn modulated(&self, voice_idx: usize) -> WTOscModValues {

        let [lvl_l, lvl_r] = self.detune.get_value(voice_idx);
        let [pan_l, pan_r] = self.pan.get_value(voice_idx);

        let stereo_pos = [sub(1., lvl_l), lvl_r].into();
        let pan = [sub(1., pan_l), pan_r].into();

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

impl NodeParemeters for WTOscParams {

    fn type_name(&self) -> String { "Oscillator".into() }

    fn processor(&self, global_params: &KrynthParams) -> Box<dyn Processor> {
        let params = Arc::new(Self::new(global_params));
        Box::new(WTOsc::new(params))
    }

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response {

        ui.horizontal(|ui| {

            ui.vertical(|ui| {

                ui.add(ParamWidget::new(
                    Knob::new().radius(40.),
                    ParamHandle::from((self.level.deref(), setter))
                ));

                ui.horizontal(|ui| {

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.num_unison_voices.deref(), setter).into()
                    ));

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.pan.deref(), setter).into()
                    ));
                });

                ui.horizontal(|ui| {

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.detune.deref(), setter).into()
                    ));

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.detune_range.deref(), setter).into()
                    ));
                });
            });

            ui.vertical_centered_justified(|ui| {
                let current_name_ref = self.wt_name.borrow();

                ComboBox::from_id_source(ui.id().with("combobox"))
                    .width(ui.available_width())
                    .selected_text(current_name_ref.deref())
                    .show_ui(ui, |ui| {
                        for name in self.wt_list.iter() {

                            if ui.selectable_label(name == current_name_ref.deref(), name).clicked() {

                                // to free the AtomicRefCell borrow
                                drop(current_name_ref);

                                self.wt_name.set(name.to_string());
                                self.reload();
                                break;
                            }
                        }
                    });

                ui.horizontal_centered( |ui| {

                    let wavetable = self.wavetable.read();

                    let points = PlotPoints::from_ys_f32(wavetable.last_at(
                        self.frame.unmodulated_plain_value() as usize
                    ));

                    plain_plot(ui.id().with("Plot"), 0.0..points.points().len() as f64, -1.0..1.0)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(points).fill(0.))
                        });

                    ui.add(ParamWidget::<VSlider, ParamHandle<_>>::default(
                        (self.frame.deref(), setter).into()
                    ));
                });
            })
        }).response
    }

    fn reload(&self) {

        let wt_name = self.wt_name.borrow();
        let wt_name = wt_name.as_str();
        let new_wavetable = BandlimitedWaveTables::from_file(format!("{WAVETABLE_FOLDER_PATH}\\{wt_name}.WAV"));
        *self.wavetable.write() = new_wavetable;
    }
}