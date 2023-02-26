use plugin_util::dsp::processor::ProcessSchedule;

use super::{wavetable_oscillator::WTOscParams, *};

use panel::Side;

impl KrynthParams {
    fn insert_top_level_node(&self, node: Arc<dyn KrynthNode>) {
        let mut map = self.node_count_per_type.borrow_mut();
        let id = node.type_id();

        *map.entry(id).or_insert(0) += 1;

        self.graph.borrow_mut().top_level_insert(node);
    }

    fn build_audio_graph(&self) -> ProcessSchedule {
        let graph = self.graph.borrow();
        let mut schedule = ProcessSchedule::default();

        for (node, edges) in graph.iter().zip(graph.edges().iter()) {
            schedule.push(node.clone().processor_node(), edges.clone());
        }

        schedule
    }
}

impl KrynthNode for KrynthParams {
    fn type_name(&self) -> String {
        "Synth".into()
    }

    fn ui(&self, ui: &mut Ui, setter: &ParamSetter) -> Response {
        SidePanel::new(Side::Left, "banana")
            .show_inside(ui, |ui| {
                ui.add_space(40.);

                if ui.button("new WTOsc").clicked() {
                    self.insert_top_level_node(Arc::new(WTOscParams::default()));
                }
            })
            .response
            | CentralPanel::default()
                .show_inside(ui, |ui| {
                    let mut audio_thread_messages = self.message_sender.as_ref().unwrap().lock();

                    #[allow(unused_must_use)]
                    {
                        audio_thread_messages.1.pop();
                    }

                    for (node_index, node_params) in self.graph.borrow().iter().enumerate() {
                        Window::new(node_index.to_string())
                            .fixed_size((400., 500.))
                            .show(ui.ctx(), |ui| {
                                node_params.ui(ui, setter);
                            });
                    }
                })
                .response
    }

    fn processor_node(self: Arc<Self>) -> Box<ProcessNode> {
        Box::new(self.schedule())
    }
}
