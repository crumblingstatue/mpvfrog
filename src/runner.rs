use std::time::Duration;

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
    let mut win_visible = true;
    app.tray_handle.update(|tray| {
        tray.paused = true;
    });
    loop {
        let mut should_toggle_window = false;
        let mut should_quit = false;
        let mut should_pause_resume = false;
        app.tray_handle.update(|tray| {
            tray.paused = app.paused_or_stopped();
            if tray.should_toggle_window {
                should_toggle_window = true;
                tray.should_toggle_window = false;
            }
            if tray.should_quit {
                should_quit = true;
                tray.should_quit = false;
            }
            if tray.should_pause_resume {
                should_pause_resume = true;
                tray.should_pause_resume = false;
            }
        });
        if should_quit {
            break;
        }
        if should_toggle_window {
            win_visible ^= true;
            rw.set_visible(win_visible);
        }
        if win_visible {
            while let Some(event) = rw.poll_event() {
                sf_egui.add_event(&event);
                if event == Event::Closed {
                    rw.set_visible(false);
                    win_visible = false;
                }
            }
            sf_egui.do_frame(|ctx| {
                app.update(ctx, should_pause_resume);
            });
            sf_egui.draw(&mut rw, None);
            rw.display();
        } else {
            app.bg_update(should_pause_resume);

            std::thread::sleep(Duration::from_millis(250));
        }
    }
    app.save();
}
