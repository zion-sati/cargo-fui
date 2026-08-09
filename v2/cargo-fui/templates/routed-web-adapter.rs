use fui::prelude::*;
use __ROUTE_CRATE__::__PAGE_TYPE__;

#[derive(Clone)]
struct WebPage {
    root: SelectionArea,
    _page: __PAGE_TYPE__,
}

fui_component!(WebPage => root);

impl WebPage {
    fn new() -> Self {
        Application::caption("__PROJECT_NAME__ - __PAGE_TITLE__");
        let page = __PAGE_TYPE__::new();
        let content = column()
            .fill_size()
            .child(&__CRATE_NAME___shared::navigation(__HOME_ACTIVE__))
            .child(&page.root())
            .clone();
        Self {
            root: selection_area().fill_size().child(&content).clone(),
            _page: page,
        }
    }
}

fui_app!(WebPage, WebPage::new);
