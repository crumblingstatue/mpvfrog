use {
    crate::app::core::Core,
    egui_colors::{Colorix, tokens::ThemeColor},
    egui_sf2g::egui,
    rand::Rng as _,
};

#[derive(Default)]
pub struct ColorThemeWindow {
    pub open: bool,
}
impl ColorThemeWindow {
    pub(crate) fn update(
        &mut self,
        core: &mut Core,
        ctx: &egui::Context,
        colorix: &mut Option<Colorix>,
    ) {
        egui::Window::new("Color config")
            .open(&mut self.open)
            .show(ctx, |ui| {
                let colorix = colorix.as_mut().unwrap();
                ui.horizontal(|ui| {
                    colorix.themes_dropdown(ui, None, false);
                    if ui.button("Reset default").clicked() {
                        core.cfg.theme = None;
                        ctx.set_visuals(egui::Visuals::dark());
                    }
                    if ui.button("Random").clicked() {
                        let mut rng = rand::rng();
                        let theme = std::array::from_fn(|_| rng.random());
                        core.cfg.theme = Some(theme);
                        *colorix = Colorix::global(ctx, theme.map(ThemeColor::Custom));
                    }
                });
                ui.separator();
                colorix.ui_combo_12(ui, true);

                core.cfg.theme = Some(colorix.theme().map(|theme| theme.rgb()));
            });
    }
}
