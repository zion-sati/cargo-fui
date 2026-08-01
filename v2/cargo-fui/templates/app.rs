use fui::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod services;

#[derive(Clone)]
pub struct App {
    root: SelectionArea,
    _active_worker: Rc<RefCell<Option<Worker>>>,
}

fui_component!(App => root);

impl App {
    pub fn new() -> Self {
        Application::caption("__CAPTION__");
        use_system_theme();

        let count = Rc::new(Cell::new(0_u32));
        let status = text("Clicked 0 times").font_size(18.0).clone();
        let worker_status = text("Worker ready").font_size(16.0).clone();
        let active_worker = Rc::new(RefCell::new(None));
        let action = button("Click me")
            .margin(0.0, 20.0, 0.0, 12.0)
            .configure(|action| {
                action.on_click({
                    let count = count.clone();
                    let status = status.clone();
                    move |_| {
                        let next = count.get() + 1;
                        count.set(next);
                        status.text(format!(
                            "Clicked {next} time{}",
                            if next == 1 { "" } else { "s" }
                        ));
                    }
                });
            })
            .clone();
        let run_worker = button("Run Worker")
            .margin(0.0, 12.0, 0.0, 12.0)
            .configure(|action| {
                action.on_click({
                    let active_worker = active_worker.clone();
                    let worker_status = worker_status.clone();
                    move |_| {
                        if active_worker.borrow().is_some() {
                            return;
                        }
                        worker_status.text("Worker running...");
                        let weak_worker = Rc::downgrade(&active_worker);
                        let worker = Worker::new("./workers.wasm", "sampleWorker")
                            .on_progress({
                                let worker_status = worker_status.clone();
                                move |event| {
                                    worker_status.text(event.message);
                                }
                            })
                            .on_complete({
                                let worker_status = worker_status.clone();
                                let weak_worker = weak_worker.clone();
                                move |event| {
                                    worker_status.text(event.result);
                                    if let Some(worker) = weak_worker.upgrade() {
                                        worker.borrow_mut().take();
                                    }
                                }
                            })
                            .on_error({
                                let worker_status = worker_status.clone();
                                move |event| {
                                    worker_status.text(format!("Worker error: {}", event.message));
                                    if let Some(worker) = weak_worker.upgrade() {
                                        worker.borrow_mut().take();
                                    }
                                }
                            })
                            .start("portable worker");
                        *active_worker.borrow_mut() = Some(worker);
                    }
                });
            })
            .clone();

        let content = ui! {
            column()
                .fill_size()
                .padding(32.0, 32.0, 32.0, 32.0)
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center) {
                    text("__CAPTION__").font_size(36.0).text_align(TextAlign::Center),
                    text("One retained Rust UI, ready for EffinDOM native and web targets.")
                        .font_size(16.0)
                        .text_align(TextAlign::Center),
                    action,
                    status,
                    run_worker,
                    worker_status,
                }
        };
        let root = selection_area().fill_size().child(&content).clone();
        Self {
            root,
            _active_worker: active_worker,
        }
    }
}

__APP_ENTRY__
