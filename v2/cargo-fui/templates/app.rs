use fui::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

mod services;

#[derive(Clone)]
pub struct App {
    root: SelectionArea,
}

fui_component!(App => root);

impl App {
    pub fn new() -> Self {
        Application::caption("__CAPTION__");
        use_system_theme();

        let count = Rc::new(Cell::new(0_u32));
        let status = text("Clicked 0 times").font_size(18.0).clone();
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
                }
        };
        let root = selection_area().fill_size().child(&content).clone();
        Self { root }
    }
}

__APP_ENTRY__
