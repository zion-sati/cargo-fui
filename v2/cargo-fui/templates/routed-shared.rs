use fui::prelude::*;

pub fn navigation(home_active: bool) -> FlexBox {
    let home = nav_link("/");
    home.child(&text("Home").selectable(false));
    let settings = nav_link("/settings/");
    settings.child(&text("Settings").selectable(false));
    for (link, active) in [(&home, home_active), (&settings, !home_active)] {
        link.padding(14.0, 8.0, 14.0, 8.0).corner_radius(999.0);
        link.bind_theme(move |link, theme| {
            link.bg_color(if active {
                theme.colors.accent
            } else {
                theme.colors.surface
            });
        });
    }
    row()
        .fill_width()
        .justify_content(JustifyContent::End)
        .child(&home)
        .child(&settings)
        .clone()
}
