use fui::prelude::*;

#[derive(Clone)]
pub struct __PAGE_TYPE__ {
    root: SelectionArea,
}

fui_component!(__PAGE_TYPE__ => root);

impl __PAGE_TYPE__ {
    pub fn new() -> Self {
        use_system_theme();
        let content = ui! {
            column().fill_size().padding(32.0, 32.0, 32.0, 32.0) {
                text("__PAGE_TITLE__").font_size(36.0),
                text("This retained page is shared by its browser route and the native shell.")
                    .font_size(16.0),
            }
        };
        let root = selection_area().fill_size().child(&content).clone();
        root.bind_theme(|root, theme| {
            root.bg_color(theme.colors.background);
        });
        Self { root }
    }

    pub fn root(&self) -> SelectionArea {
        self.root.clone()
    }
}
