use fui::prelude::*;
use __CRATE_NAME___home::HomePage;
use __CRATE_NAME___settings::SettingsPage;

#[derive(Clone)]
struct App {
    root: FlexBox,
    _tabs: TabView,
}

fui_component!(App => root);

impl App {
    fn new() -> Self {
        Application::caption("__PROJECT_NAME__");
        use_system_theme();
        let tabs = tab_view();
        tabs.fill_size()
            .add_item(TabItem::with_content("Home", || {
                let page = HomePage::new();
                retained_view(&page.root()).keep_alive(page)
            }))
            .add_item(TabItem::with_content("Settings", || {
                let page = SettingsPage::new();
                retained_view(&page.root()).keep_alive(page)
            }));
        let home = button("Home");
        home.on_click({
            let tabs = tabs.clone();
            move |_| {
                tabs.select_index(0);
            }
        });
        let settings = button("Settings");
        settings.on_click({
            let tabs = tabs.clone();
            move |_| {
                tabs.select_index(1);
            }
        });
        let root = ui! { column().fill_size() { row() { home, settings }, tabs } };
        Self { root, _tabs: tabs }
    }
}

fui_app!(App, App::new);
