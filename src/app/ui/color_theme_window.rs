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
        opt_colorix: &mut Option<Colorix>,
    ) {
        egui::Window::new("Color config")
            .open(&mut self.open)
            .show(ctx, |ui| {
                let Some(colorix) = opt_colorix else {
                    if ui.button("Enable custom theme").clicked() {
                        *opt_colorix = Some(Colorix::global(ctx, egui_colors::utils::EGUI_THEME));
                    }
                    return;
                };
                let mut reset_default = false;
                ui.horizontal(|ui| {
                    colorix.themes_dropdown(ui, None, false);
                    if ui.button("Reset default").clicked() {
                        reset_default = true;
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
                if reset_default {
                    core.cfg.theme = None;
                    ctx.set_visuals(egui::Visuals::dark());
                    *opt_colorix = None;
                    return;
                }
                // If we have a theme, update it from colorix
                // (so when the user edits a color, it updates)
                if let Some(theme) = &mut core.cfg.theme {
                    *theme = colorix.theme().map(|theme| theme.rgb())
                }
            });
    }
}
