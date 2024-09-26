use {
    crate::{
        app::{ui::apply_colorix_theme, App},
        rect_math::{rect_ensure_within, Rect, Vec2},
        MODAL,
    },
    egui_sfml::{
        egui,
        sfml::{
            graphics::{Color, FloatRect, RenderTarget, RenderWindow, View},
            window::{Event, Style, VideoMode},
        },
        SfEgui,
    },
    std::time::Duration,
};

struct CtxMenuWin {
    rw: RenderWindow,
    sf_egui: SfEgui,
}

pub fn run(w: u32, h: u32, title: &str) {
    let mut rw = RenderWindow::new((w, h), title, Style::RESIZE, &Default::default());
    let mut tray_popup_win = None;
    rw.set_framerate_limit(60);
    let mut sf_egui = SfEgui::new(&rw);
    let mut app = App::new(sf_egui.context());
    *MODAL.lock().unwrap() = Some(egui_modal::Modal::new(sf_egui.context(), "modal_dialog"));
    let mut win_visible = true;
    'mainloop: loop {
        app.tray_handle.update();
        let mut event_flags = app.tray_handle.event_flags.take();
        if event_flags.quit_clicked {
            break;
        }
        if event_flags.activated {
            if tray_popup_win.is_some() {
                tray_popup_win = None;
            } else {
                win_visible ^= true;
                rw.set_visible(win_visible);
            }
        }
        if let Some((x, y)) = event_flags.ctx_menu.take() {
            if tray_popup_win.is_some() {
                tray_popup_win = None;
            } else {
                let desired = Rect {
                    pos: Vec2 { x, y },
                    size: Vec2 { x: 200, y: 100 },
                };
                let desk_size = VideoMode::desktop_mode();
                let desk_rect = Rect {
                    pos: Vec2 { x: 0, y: 0 },
                    size: Vec2 {
                        x: desk_size.width as i32,
                        y: desk_size.height as i32,
                    },
                };
                let put_rect = rect_ensure_within(desired, desk_rect, Vec2 { x: 16, y: 32 });
                let mut rw = RenderWindow::new(
                    (put_rect.size.x as u32, put_rect.size.y as u32),
                    "NOOO",
                    Style::NONE,
                    &Default::default(),
                );
                // Skip taskbar for context menu window
                unsafe {
                    let native = rw.system_handle();
                    let display = x11::xlib::XOpenDisplay(std::ptr::null());
                    let utility = x11::xlib::XInternAtom(
                        display,
                        c"_NET_WM_STATE_SKIP_TASKBAR".as_ptr(),
                        x11::xlib::False,
                    );
                    let property = x11::xlib::XInternAtom(
                        display,
                        c"_NET_WM_STATE".as_ptr(),
                        x11::xlib::False,
                    );
                    x11::xlib::XChangeProperty(
                        display,
                        native,
                        property,
                        x11::xlib::XA_ATOM,
                        32,
                        x11::xlib::PropModeReplace,
                        std::ptr::addr_of!(utility) as *const u8,
                        1,
                    );
                    x11::xlib::XCloseDisplay(display);
                }
                rw.set_position((put_rect.pos.x, put_rect.pos.y).into());
                rw.set_vertical_sync_enabled(true);
                let sf_egui = SfEgui::new(&rw);
                apply_colorix_theme(app.core.cfg.theme, sf_egui.context());
                tray_popup_win = Some(CtxMenuWin { rw, sf_egui });
            }
        }
        app.update_tooltip();
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
                .do_pass(&mut rw, |ctx| {
                    app.update(ctx);
                })
                .unwrap();
            sf_egui.draw(&mut rw, None);
            rw.display();
            // Update tray window if visible
            if let Some(win) = &mut tray_popup_win {
                let msg = update_tray_window(win, &mut app);
                if let Some(msg) = msg {
                    match msg {
                        TrayUpdateMsg::QuitApp => break 'mainloop,
                        TrayUpdateMsg::CloseTray => tray_popup_win = None,
                    }
                }
            }
        } else {
            // Update tray window if visible
            if let Some(win) = &mut tray_popup_win {
                app.tray_popup_update(win.sf_egui.context());
                let msg = update_tray_window(win, &mut app);
                if let Some(msg) = msg {
                    match msg {
                        TrayUpdateMsg::QuitApp => break 'mainloop,
                        TrayUpdateMsg::CloseTray => tray_popup_win = None,
                    }
                }
            } else {
                app.bg_update();
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
    app.save();
}

enum TrayUpdateMsg {
    QuitApp,
    CloseTray,
}

fn update_tray_window(win: &mut CtxMenuWin, app: &mut App) -> Option<TrayUpdateMsg> {
    let mut msg = None;
    while let Some(event) = win.rw.poll_event() {
        win.sf_egui.add_event(&event);
        if let Event::LostFocus = event {
            return Some(TrayUpdateMsg::CloseTray);
        }
    }
    win.rw.clear(Color::MAGENTA);
    win.sf_egui.begin_pass();
    let mut quit = false;
    egui_sfml::egui::CentralPanel::default().show(win.sf_egui.context(), |ui| {
        ui.horizontal(|ui| {
            ui.label("egui-mpv");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    quit = true;
                }
                if ui.checkbox(&mut app.core.cfg.video, "video").clicked() {
                    app.core.set_video(app.core.cfg.video);
                }
            })
        });
        ui.horizontal(|ui| {
            ui.label("🔈");
            app.update_volume();
            let re = ui.add(egui_sfml::egui::Slider::new(
                &mut app.core.cfg.volume,
                0..=150,
            ));
            if re.changed() {
                app.core.mpv_handler.set_volume(app.core.cfg.volume);
            }
        });
        let play_pause_label = if app.paused_or_stopped() {
            "▶"
        } else {
            "⏸"
        };
        if let Some(name) = app.currently_playing_name() {
            ui.add(egui_sfml::egui::Label::new(name).wrap_mode(egui::TextWrapMode::Extend));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(38.0);
            if ui.button("⏪").clicked() {
                app.core.play_prev();
            }
            if ui.button(play_pause_label).clicked() {
                app.core.play_or_toggle_pause();
            }
            if ui.button("⏹").clicked() {
                app.core.stop_music();
            }
            if ui.button("⏩").clicked() {
                app.core.play_next();
            }
        });
    });
    if quit {
        msg = Some(TrayUpdateMsg::QuitApp);
    }
    win.sf_egui.end_pass(&mut win.rw).unwrap();
    win.sf_egui.draw(&mut win.rw, None);
    win.rw.display();
    msg
}
