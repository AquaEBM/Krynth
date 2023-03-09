use super::*;

#[derive(Default)]
pub struct ProcessSchedule { 
    nodes: Vec<ProcessComponent>,
    edges: Vec<Vec<usize>>,
}

impl Processor for ProcessSchedule {

    fn add_voice(&mut self, norm_freq: f32) {

        for node in &mut self.nodes {
            node.sample_buffer.push(ZERO_SAMPLE);
            node.processor.add_voice(norm_freq);
        }
    }

    fn remove_voice(&mut self, voice_idx: usize) {

        for node in &mut self.nodes {
            node.sample_buffer.swap_remove(voice_idx);
            node.processor.remove_voice(voice_idx);
        }
    }

    fn process(&mut self, _voice_idx: usize, _inputs: &mut StereoSample) {

        // C++ like index iteration is required here in order to work around Rust's borrowing
        // rules because indexing, as opposed to, say, iter_mut() doesn't hold a long borrow

        for i in 0..self.nodes.len() {

            self.nodes[i].process();

            if  self.edges[i].is_empty() {

                // self.nodes[i].output_to_buffer(inputs);
            }

            for &j in &self.edges[i] {

                for k in 0..self.nodes[i].sample_buffer.len() {

                    let sample = self.nodes[i].sample_buffer[k];

                    self.nodes[j].sample_buffer[k] += sample;
                }
            }
        }
    }

    fn initialize(&mut self) -> (bool, u32) {
        self.nodes.iter_mut().map(|node| node.processor.initialize());
        (true, 0)
    }

    fn reset(&mut self) {
        self.nodes.iter_mut().for_each(|node| node.processor.reset());
    }
}

impl ProcessSchedule {
    pub(super) fn push(&mut self, processor: Box<dyn Processor + Send>, successors: Vec<usize>) {
        self.nodes.push(processor.into());
        self.edges.push(successors);
    }
}

pub struct ProcessComponent {
    processor: Box<dyn Processor + Send>,
    sample_buffer: ArrayVec<StereoSample, 16>,
}

impl From<Box<dyn Processor + Send>> for ProcessComponent {
    fn from(processor: Box<dyn Processor + Send>) -> Self {
        Self {
            processor,
            sample_buffer: Default::default()
        }
    }
}

impl ProcessComponent {

    pub fn process(&mut self) {
        // for sample in self.sample_buffer.iter_mut() {
        //     self.processor.process(sample);
        // }   
    }

    pub fn output_to_buffer(&mut self, inputs: &mut [StereoSample]) {

        for (&output, input) in self.sample_buffer.iter().zip(inputs.iter_mut()) {
            *input += output;
        }
    }
}

impl SeenthStandAlonePlugin for SeenthParams {
    type Processor = ProcessSchedule;

    fn processor(self: Arc<Self>) -> Self::Processor {
        self.schedule()
    }

    fn editor_state(&self) -> Arc<EguiState> {
        self.editor_state.clone()
    }
}
