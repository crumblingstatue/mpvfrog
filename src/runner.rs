use std::time::Duration;

use egui_sfml::{
    sfml::{
        graphics::{FloatRect, RenderTarget, RenderWindow, View},
        window::{Event, Style},
    },
    SfEgui,
};

use crate::app::{tray::AppTray, App};

pub fn run(w: u32, h: u32, title: &str) {
    let mut rw = RenderWindow::new((w, h), title, Style::RESIZE, &Default::default());
    rw.set_framerate_limit(60);
    let mut sf_egui = SfEgui::new(&rw);
    let mut app = App::new(sf_egui.context());
    let mut win_visible = true;
    loop {
        let mut event_flags = Default::default();
        app.tray_handle.update(|tray: &mut AppTray| {
            app.write_more_info(&mut tray.app_state.tray_info);
            tray.app_state.paused = app.paused_or_stopped();
            event_flags = tray.event_flags.take()
        });
        if event_flags.quit_clicked {
            break;
        }
        if event_flags.activated {
            win_visible ^= true;
            rw.set_visible(win_visible);
        }
        if win_visible {
            while let Some(event) = rw.poll_event() {
                sf_egui.add_event(&event);
                match event {
                    Event::Closed => {
                        rw.set_visible(false);
                        win_visible = false;
                    }
                    Event::Resized { width, height } => {
                        rw.set_view(&View::from_rect(FloatRect::new(
                            0.,
                            0.,
                            width as f32,
                            height as f32,
                        )));
                    }
                    _ => {}
                }
            }
            sf_egui
                .do_frame(|ctx| {
                    app.update(ctx, event_flags.pause_resume_clicked);
                })
                .unwrap();
            sf_egui.draw(&mut rw, None);
            rw.display();
        } else {
            app.bg_update(event_flags.pause_resume_clicked);

            std::thread::sleep(Duration::from_millis(250));
        }
    }
    app.save();
}
