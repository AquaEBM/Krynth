use super::*;

impl KrynthStandAlonePlugin for KrynthParams {
    type Processor = ProcessSchedule;

    fn processor(self: Arc<Self>) -> Self::Processor {
        self.schedule()
    }
}
