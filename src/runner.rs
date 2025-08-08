//! Launch and main loop of the application

use {
    crate::{
        app::{self, App, tray::EventFlags},
        rect_math::{Rect, Vec2, rect_ensure_within},
        util::result_ext::ResultModalExt as _,
    },
    egui_sf2g::{
        SfEgui, egui,
        sf2g::{
            cpp::FBox,
            graphics::{Color, FloatRect, RenderTarget, RenderWindow, View},
            window::{Event, Key, Style, VideoMode},
        },
    },
    std::{path::PathBuf, time::Duration},
    x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode},
};

struct CtxMenuWin {
    rw: FBox<RenderWindow>,
    sf_egui: SfEgui,
}

pub fn run(
    w: u32,
    h: u32,
    title: &str,
    mut instance_listener: Option<existing_instance::Listener>,
    args: crate::Args,
) {
    let mut rw = RenderWindow::new((w, h), title, Style::RESIZE, &Default::default()).unwrap();
    let mut tray_popup_win = None;
    rw.set_framerate_limit(60);
    let mut sf_egui = SfEgui::new(&rw);
    let mut app = App::new(sf_egui.context(), &args);
    let mut win_visible = true;
    'mainloop: loop {
        let mut event_flags;
        if let Some(trhandle) = &mut app.tray_handle {
            trhandle.update();
            event_flags = trhandle.event_flags.take();
        } else {
            event_flags = EventFlags::default();
        }
        if let Some(listener) = &mut instance_listener {
            if let Some(mut stream) = listener.accept() {
                if let Some(msg) = stream.recv() {
                    match msg {
                        existing_instance::Msg::String(path) => {
                            let path: PathBuf = path.into();
                            if path.is_dir() {
                                app::open_folder(&mut app.core, &mut app.ui, path);
                            } else if path.is_file() {
                                if let Some(parent) = path.parent() {
                                    app::open_folder(&mut app.core, &mut app.ui, parent.to_owned());
                                    let stripped = path.strip_prefix(parent).unwrap();
                                    if let Some(pos) = app
                                        .core
                                        .playlist
                                        .iter()
                                        .position(|item| item.path == stripped)
                                    {
                                        app.focus_and_play(pos);
                                    }
                                }
                            }
                        }
                        existing_instance::Msg::Nudge => {
                            event_flags.activated = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        if event_flags.quit_clicked {
            break;
        }
        if event_flags.activated {
            // First, close the tray popup if it's open
            if tray_popup_win.is_some() {
                tray_popup_win = None;
            } else {
                // Try to focus unfocused window first, only hide if already focused
                if win_visible && !rw.has_focus() {
                    rw.set_visible(false);
                    rw.set_visible(true);
                } else {
                    toggle_win_visible(&mut tray_popup_win, &mut win_visible, &mut rw);
                }
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
                    "",
                    Style::NONE,
                    &Default::default(),
                )
                .unwrap();
                // Skip taskbar for context popup window
                if let Err(e) = skip_taskbar_for_window_x11(&rw) {
                    eprintln!("Failed to skip taskbar for ctx popup: {e}");
                }
                rw.set_position((put_rect.pos.x, put_rect.pos.y).into());
                rw.set_vertical_sync_enabled(true);
                let sf_egui = SfEgui::new(&rw);
                app.ui
                    .apply_colorix_theme(app.core.cfg.theme.as_ref(), sf_egui.context());
                tray_popup_win = Some(CtxMenuWin { rw, sf_egui });
            }
        }
        app.update_tooltip();
        if win_visible {
            while let Some(event) = rw.poll_event() {
                sf_egui.add_event(&event);
                match event {
                    Event::Closed => {
                        if app.tray_handle.is_none() {
                            eprintln!("No tray handle, quitting.");
                            break 'mainloop;
                        }
                        rw.set_visible(false);
                        win_visible = false;
                    }
                    Event::Resized { width, height } => {
                        rw.set_view(
                            &View::from_rect(FloatRect::new(0., 0., width as f32, height as f32))
                                .unwrap(),
                        );
                    }
                    Event::KeyPressed { code, ctrl, .. } => {
                        // Shortcut keys
                        if code == Key::Escape && !sf_egui.context().wants_keyboard_input() {
                            rw.set_visible(false);
                            win_visible = false;
                        }
                        if code == Key::Q && ctrl {
                            break 'mainloop;
                        }
                        if code == Key::O && ctrl {
                            app.ui.file_dialog.pick_directory();
                        }
                    }
                    _ => {}
                }
            }
            let di = sf_egui
                .run(&mut rw, |_rw, ctx| {
                    app.update(ctx);
                })
                .unwrap();
            sf_egui.draw(di, &mut rw, None);
            rw.display();
            // Update tray window if visible
            if let Some(win) = &mut tray_popup_win {
                app.tray_popup_update(win.sf_egui.context());
                let msg = update_tray_window(win, &mut app);
                if let Some(msg) = msg {
                    match msg {
                        TrayUpdateMsg::QuitApp => break 'mainloop,
                        TrayUpdateMsg::CloseTray => tray_popup_win = None,
                        TrayUpdateMsg::FocusApp => {
                            toggle_win_visible(&mut tray_popup_win, &mut win_visible, &mut rw)
                        }
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
                        TrayUpdateMsg::FocusApp => {
                            toggle_win_visible(&mut tray_popup_win, &mut win_visible, &mut rw)
                        }
                    }
                }
            } else {
                app.bg_update();
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        if app.ui.quit_requested {
            break;
        }
    }
    app.save();
}

fn skip_taskbar_for_window_x11(rw: &RenderWindow) -> anyhow::Result<()> {
    let native = rw.system_handle();
    let (conn, _) = x11rb::connect(None)?;
    let utility = conn
        .intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?
        .reply()?
        .atom;
    let property = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    x11rb::protocol::xproto::change_property(
        &conn,
        PropMode::REPLACE,
        native as u32,
        property,
        AtomEnum::ATOM,
        32,
        1,
        &utility.to_le_bytes(),
    )?
    .check()?;
    Ok(())
}

fn toggle_win_visible(
    tray_popup_win: &mut Option<CtxMenuWin>,
    win_visible: &mut bool,
    rw: &mut RenderWindow,
) {
    if tray_popup_win.is_some() {
        *tray_popup_win = None;
    }
    *win_visible ^= true;
    rw.set_visible(*win_visible);
}

enum TrayUpdateMsg {
    QuitApp,
    CloseTray,
    FocusApp,
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
    egui::CentralPanel::default().show(win.sf_egui.context(), |ui| {
        ui.horizontal(|ui| {
            if ui
                .add(egui::Label::new(crate::APP_LABEL).sense(egui::Sense::click()))
                .clicked()
            {
                msg = Some(TrayUpdateMsg::FocusApp);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    quit = true;
                }
                if ui.checkbox(&mut app.core.cfg.video, "video").clicked() {
                    app.core
                        .mpv_handler
                        .ipc(|b| b.set_video(app.core.cfg.video))
                        .err_popup("Video set error", &mut app.modal);
                }
            })
        });
        ui.horizontal(|ui| {
            ui.label("🔈");
            app.update_volume();
            let re = ui.add(egui::Slider::new(&mut app.core.cfg.volume, 0..=150));
            if re.changed() {
                app.core
                    .mpv_handler
                    .ipc(|b| b.set_volume(app.core.cfg.volume))
                    .err_popup("Volume set error", &mut app.modal);
            }
        });
        let play_pause_label = if app.paused_or_stopped() {
            "▶"
        } else {
            "⏸"
        };
        if let Some(name) = app.currently_playing_name() {
            ui.add(egui::Label::new(name).wrap_mode(egui::TextWrapMode::Extend));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(38.0);
            if ui.button(app::ui::ICO_PREV).clicked() {
                app.core.play_prev(&mut app.modal);
            }
            if ui.button(play_pause_label).clicked() {
                app.core.play_or_toggle_pause(&mut app.modal);
            }
            if ui.button("⏹").clicked() {
                app.core.stop_music();
            }
            if ui.button(app::ui::ICO_NEXT).clicked() {
                app.core.play_next(&mut app.modal);
            }
        });
    });
    if quit {
        msg = Some(TrayUpdateMsg::QuitApp);
    }
    let di = win.sf_egui.end_pass(&mut win.rw).unwrap();
    win.sf_egui.draw(di, &mut win.rw, None);
    win.rw.display();
    msg
}
