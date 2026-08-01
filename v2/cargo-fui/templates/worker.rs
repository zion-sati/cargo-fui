use fui::{fui_worker, WorkerJob, WorkerJobState};

struct SampleWorkerJob {
    state: WorkerJobState,
    input: String,
    yielded: bool,
}

impl Default for SampleWorkerJob {
    fn default() -> Self {
        Self {
            state: WorkerJobState::new(),
            input: String::new(),
            yielded: false,
        }
    }
}

impl WorkerJob for SampleWorkerJob {
    fn state(&mut self) -> &mut WorkerJobState {
        &mut self.state
    }

    fn on_start(&mut self, input: String) {
        self.input = input;
    }

    fn run(&mut self) {
        if self.yielded {
            self.complete(format!("Worker completed: {}", self.input));
        } else {
            self.yielded = true;
            self.report_progress("Worker progress received");
            self.r#yield(1);
        }
    }
}

fui_worker!(sampleWorker => SampleWorkerJob);
