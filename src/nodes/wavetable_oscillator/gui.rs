use super::*;
use plot::*;
use std::{fs::read_dir, ops::Deref, sync::OnceLock};

use wavetable::write_wavetable_from_file;

static WT_LIST: OnceLock<Vec<String>> = OnceLock::new();

const WAVETABLE_FOLDER_PATH: &str =
    "C:\\Users\\etulyon1\\Documents\\Coding\\Krynth\\wavetables";

impl KrynthNode for WTOscParams {
    fn type_name(&self) -> String {
        "Oscillator".into()
    }

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add(ParamWidget::new(
                    Knob::new().radius(40.),
                    ParamHandle::from((self.level.deref(), setter)),
                ));

                ui.horizontal(|ui| {
                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.num_unison_voices.deref(), setter).into(),
                    ));

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.pan.deref(), setter).into(),
                    ));
                });

                ui.horizontal(|ui| {
                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.detune.deref(), setter).into(),
                    ));

                    ui.add(ParamWidget::<Knob, ParamHandle<_>>::default(
                        (self.detune_range.deref(), setter).into(),
                    ));
                });
            });

            ui.vertical_centered_justified(|ui| {
                let mut current_name_ref = self.wt_name.borrow_mut();

                let wt_list = WT_LIST.get_or_init(|| {
                    read_dir(WAVETABLE_FOLDER_PATH)
                        .unwrap()
                        .map(|dir| {
                            dir.unwrap()
                                .file_name()
                                .to_str()
                                .unwrap()
                                .trim_end_matches(".WAV")
                                .into()
                        })
                        .collect::<Vec<_>>()
                        .into()
                });

                ComboBox::from_id_source(ui.id().with("combobox"))
                    .width(ui.available_width())
                    .selected_text(current_name_ref.deref())
                    .show_ui(ui, |ui| {
                        for name in wt_list.iter() {
                            if ui
                                .selectable_label(name == current_name_ref.as_str(), name)
                                .clicked()
                            {
                                *current_name_ref = name.clone();
                                write_wavetable_from_file(
                                    format!("{WAVETABLE_FOLDER_PATH}\\{name}.WAV"),
                                    self.wavetable
                                        .borrow_mut()
                                        .as_mut_slice()
                                        .try_into()
                                        .unwrap(),
                                );
                            }
                        }
                    });

                ui.horizontal_centered(|ui| {
                    let wavetable = self.wavetable.borrow();

                    let points = PlotPoints::from_ys_f32(
                        wavetable[self.frame.unmodulated_plain_value() as usize].as_slice(),
                    );

                    plain_plot(
                        ui.id().with("Plot"),
                        0.0..points.points().len() as f64,
                        -1.0..1.0,
                    )
                    .show(ui, |plot_ui| plot_ui.line(Line::new(points).fill(0.)));

                    ui.add(ParamWidget::<VSlider, ParamHandle<_>>::default(
                        (self.frame.deref(), setter).into(),
                    ));
                });
            })
        })
        .response
    }

    fn processor_node(self: Arc<Self>) -> Box<ProcessNode> {
        Box::new(self.oscillator())
    }
}

impl KrynthStandAlonePlugin for WTOscParams {
    type Processor = WTOsc;

    fn processor(self: Arc<Self>) -> Self::Processor {
        self.oscillator()
    }
}
