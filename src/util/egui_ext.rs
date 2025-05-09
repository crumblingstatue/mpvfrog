use egui_sf2g::egui;

pub trait EguiResponseExt {
    fn h_pointer_ratio(&self) -> Option<f32>;
}

impl EguiResponseExt for egui::Response {
    fn h_pointer_ratio(&self) -> Option<f32> {
        self.hover_pos().map(|hover_pos| {
            let x = (hover_pos - self.rect.left_top()).x;
            (x / self.rect.width()).clamp(0.0, 1.0)
        })
    }
}
