use egui_sfml::{
    sfml::{
        graphics::RenderWindow,
        window::{Event, Style},
    },
    SfEgui,
};

use crate::app::App;

pub fn run(w: u32, h: u32, title: &str) {
    let mut rw = RenderWindow::new((w, h), title, Style::CLOSE, &Default::default());
    rw.set_framerate_limit(60);
    let mut sf_egui = SfEgui::new(&rw);
    let mut app = App::new(sf_egui.context());
    while rw.is_open() {
        while let Some(event) = rw.poll_event() {
            sf_egui.add_event(&event);
            if event == Event::Closed {
                rw.close();
            }
        }
        sf_egui.do_frame(|ctx| {
            app.update(ctx);
        });
        sf_egui.draw(&mut rw, None);
        rw.display();
    }
    app.save();
}
