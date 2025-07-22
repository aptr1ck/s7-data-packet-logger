use std::fs;
use std::thread;
use std::time::Duration;
use std::sync::Arc;
use chrono::Local;
use chrono::TimeZone;
use im::Vector;
use floem::{
    action::exec_after,
    event::{Event, EventListener}, 
    style::FontStyle,
    keyboard::{Key, NamedKey}, 
    menu::{Menu, MenuItem},
    peniko::Color, prelude::*, 
    reactive::{create_effect, create_memo, create_signal, ReadSignal, SignalGet, SignalUpdate, WriteSignal}, style::{AlignContent, CursorStyle, Position}, text::Weight, views::{button, container, h_stack, label, scroll, v_stack, Decorators}, window::{new_window, WindowConfig, WindowId}, IntoView, View, ViewId
};
use crate::comms::{ServerEntry, ServerStatus};
use crate::constants::*;
use crate::filehandling::file_tail;
use crate::{SERVER_STATUS, SERVER_CONFIG, ServerStatusInfo, mpsc::Receiver};
use crate::utils::*;

const LOG_LINES: usize = 500; // Number of lines to show in the log viewer

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum Tab {
    Servers,
    Log,
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Tab::Servers => write!(f, "Servers"),
            Tab::Log => write!(f, "Log"),
        }
    }
}

fn tab_button(
    this_tab: Tab,
    tabs: ReadSignal<Vector<Tab>>,
    set_active_tab: WriteSignal<usize>,
    active_tab: ReadSignal<usize>,
) -> impl IntoView {
    label(move || this_tab)
        .keyboard_navigable()
        .on_click_stop(move |_| {
            set_active_tab.update(|v: &mut usize| {
                *v = tabs
                    .get_untracked()
                    .iter()
                    .position(|it| *it == this_tab)
                    .unwrap();
            });
        })
        .style(move |s| {
            s.width(70)
                .hover(|s| s.font_weight(Weight::BOLD).cursor(CursorStyle::Pointer))
                .apply_if(
                    active_tab.get()
                        == tabs
                            .get_untracked()
                            .iter()
                            .position(|it| *it == this_tab)
                            .unwrap(),
                    |s| s.font_weight(Weight::BOLD),
                )
        })
}

const TABBAR_HEIGHT: f64 = 37.0;
const CONTENT_PADDING: f64 = 10.0;

fn tab_content(tab: Tab, status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    match tab {
        Tab::Servers => container(server_stack(status_signal)).style(|s| s.width_full().height_full()),
        Tab::Log => container(log_view(status_signal)).style(|s| s.width_full().height_full()),
    }
}

fn log_view(status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    let (log_lines_signal, set_log_lines_signal) = create_signal(Vector::<String>::new());
    
    create_effect(move |_| {
        let _status = status_signal.get();
        
        let log_content = file_tail("log.txt", LOG_LINES)
            .unwrap_or_else(|_| String::from("Failed to read log file."));
        
        let lines: Vector<String> = log_content.lines().map(|l| l.to_string()).collect();
        set_log_lines_signal.set(lines);
    });

    scroll(
        VirtualStack::new(move || log_lines_signal.get())
            .style(|s| {
                s.flex_col()
                    .width_full()
                    //.background(Color::from_rgb8(240, 240, 240))
                    .class(LabelClass, |s| {
                        s.width_full()
                            .font_family("monospace".to_string()) 
                            .font_size(13.0)
                            .padding_vert(1.0)
                            //.height(16.0) // Fixed height per line
                    })
            })
    )
    .style(|s| {
        s.width_full()
            .height_full()
            .padding(5.0)
            .background(Color::from_rgb8(255, 255, 255))
    })
}

fn tab_navigation_view(status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    let tabs = vec![Tab::Servers, Tab::Log]
        .into_iter()
        .collect::<Vector<Tab>>();
    let (tabs, _set_tabs) = create_signal(tabs);
    let (active_tab, set_active_tab) = create_signal(0);

    let tabs_bar = h_stack((
        tab_button(Tab::Servers, tabs, set_active_tab, active_tab),
        tab_button(Tab::Log, tabs, set_active_tab, active_tab),
        label(||"").style(|s| s.flex_grow(1.0)), // Spacer
        button("Exit")
            .action(|| {
                floem::quit_app();
            })
            .style(|s| s.padding_horiz(10.0)
                                .height_full()
                                .cursor(CursorStyle::Pointer)
                                .border(0.0)
                                .background(Color::TRANSPARENT)
                                .hover(|s| s.font_weight(Weight::BOLD).cursor(CursorStyle::Pointer).background(Color::TRANSPARENT))
            ),
    ))
    .style(|s| {
        s.flex_row()
            .width_full()
            .height(TABBAR_HEIGHT)
            .col_gap(5)
            .padding(CONTENT_PADDING)
            .border_bottom(1)
            .border_color(Color::from_rgb8(205, 205, 205))
    });

    //let status = status.clone();
    let main_content = container(
        scroll(
            tab(
                move || active_tab.get(),
                move || tabs.get(),
                |it| *it,
                move |it| container(tab_content(it, status_signal)).style(|s| s.width_full().height_full()),
            )
            .style(|s| s.width_full()
                                //.padding(CONTENT_PADDING)
                                //.padding_bottom(10.0)
                                .flex_grow(1.0)),
        )
        .style(|s| s.width_full().flex_col().flex_basis(0).min_width(0).flex_grow(1.0)),
    )
    .style(|s| {
        s.position(Position::Absolute)
            .inset_top(TABBAR_HEIGHT)
            .inset_bottom(0.0)
            .width_full()
    });

    let settings_view = v_stack((tabs_bar, main_content)).style(|s| s.width_full().height_full());
    settings_view
}

fn window_menu(
    //view_id: WindowId,
) -> Menu {
    Menu::new(APPNAME)
        .entry({
            Menu::new("File")
                .entry(MenuItem::new("Exit").action(move || {
                    //workbench_command.send(WorkBenchCommand::CloseWindow);
                    //floem::close_window(view_id);
                    floem::quit_app();
                }))
            })
}

fn server_view(i: usize, status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    // Server Config Data
    let server = unsafe { &SERVER_CONFIG.server[i] };
    let name = server.name.clone();
    let ip_address = RwSignal::new(server.ip_address.clone());
    let port = RwSignal::new(server.port.clone().to_string());
    // Server Status Data
    let default_status = ServerStatusInfo {
        idx: i,
        new_data: false,
        is_alive: false,
        is_connected: false,
        last_packet_time: 0,
    };

    // Main h-stack for server view
    h_stack((
        // Basic server info
        v_stack((
            label(move || {name.clone()}).style(|s| s.font_size(20.0)),
            label(move || "").style(|s| s.flex_grow(1.0)), // Spacer
            label(move || {
                status_signal.get().get(i)
                    .map(|s| {
                        let secs = (s.last_packet_time / 1000) as i64;
                        let nsec = ((s.last_packet_time % 1000) * 1_000_000) as u32;
                        let datetime = Local.timestamp_opt(secs, nsec).unwrap();
                        datetime.format("Last packet at %Y-%m-%d %H:%M:%S").to_string()})//s.last_packet_time).unwrap().format("%Y-%m-%d %H:%M:%S").to_string())})
                    .unwrap_or_else(|| "No packets received.".to_string())
            }).style(|s| s.font_size(12.0).font_style(floem::text::Style::Italic).color(Color::from_rgb8(100, 100, 100))),
            label(move || {
                status_signal.get().get(i)
                    .map(|s| if s.is_alive { "Alive".to_string() } else { "Not Alive".to_string() })
                    .unwrap_or_else(|| "Not Alive".to_string())
            }).style(move |s| {
                let is_alive = status_signal.get().get(i)
                    .map(|s| s.is_alive)
                    .unwrap_or(false);
                s.color(if is_alive { Color::from_rgb8(50, 200, 50) } else { Color::from_rgb8(200, 50, 50) })
            }),
        )).style(|s| s.flex_grow(1.0).gap(8.0).items_start()),
        // Input fields for IP and Port
        v_stack((
            h_stack((
                label(||"IP Address"),
                text_input(ip_address),
            )).style(|s| s.justify_end().gap(10.0).items_center()),
            h_stack((
                label(||"Port"),
                text_input(port),
            )).style(|s| s.justify_end().gap(10.0).items_center()),
        )).style(|s| s.gap(5.0)),
        v_stack((
            button("Start Server").action(|| {
                println!("Start Server clicked!");
            }).style(|s| s.height_full()),
            button("Stop Server").action(|| {
                println!("Stop Server clicked!");
            }).style(|s| s.height_full()),
        )).style(|s| s.gap(5.0)),
    ))
    .style(|s| s.padding(CONTENT_PADDING)
                        .gap(5.0)
                        .width_full()
                        .flex_row())
}

fn server_stack(status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    unsafe {
        dyn_stack(
            move || (0..SERVER_CONFIG.server.len()).collect::<Vec<_>>(),
            |i| *i,
            move |i| server_view(i, status_signal).style(|s| s.border_bottom(1).border_color(Color::from_rgb8(205, 205, 205)))
        ).style(|s| s.width_full()
                            .height_full()
                            .flex_col()
                            .gap(5.0))
    }
}

pub fn app_view(rx: Receiver<ServerStatusInfo>) -> impl IntoView {
    let (status_signal, set_status_signal) = create_signal(Vec::<ServerStatusInfo>::new());

    // Convert rx to Arc<Mutex<>> so we can share it between contexts
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let rx_clone = rx.clone();

    // Create a simple polling function
    fn schedule_poll(
        rx: std::sync::Arc<std::sync::Mutex<Receiver<ServerStatusInfo>>>,
        set_status_signal: WriteSignal<Vec<ServerStatusInfo>>
    ) {
        if let Ok(rx_guard) = rx.try_lock() {
            while let Ok(status) = rx_guard.try_recv() {
                set_status_signal.update(|statuses| {
                    // Update existing status or add new one
                    if let Some(existing) = statuses.iter_mut().find(|s| s.idx == status.idx) {
                        *existing = status;
                    } else {
                        statuses.push(status);
                    }
                });
            }
        }
        
        // Schedule the next poll
        exec_after(Duration::from_millis(100), move |_| {
            schedule_poll(rx, set_status_signal);
        });
    }

    // Start the polling
    schedule_poll(rx_clone, set_status_signal);

    /*let menu_bar = container(
        label(||"File")
        .popout_menu(
        ||{window_menu()})
    );*/

    let view = v_stack((
        //menu_bar,
        tab_navigation_view(status_signal),
        //server_stack,
    ))
    .style(|s| s.width_full().height_full().flex_col());//.align_content(AlignContent::FlexStart));

    let id = view.id();
    //let window_id = 0 as WindowId;//floem::window::WindowContext;
    
    view.on_event_stop(EventListener::KeyUp, move |e| {
        if let Event::KeyUp(e) = e {
            if e.key.logical_key == Key::Named(NamedKey::F11) {
                id.inspect();
            }
        }
    })
    .window_menu(move || {window_menu()})// Doesn't actually work in floem for Windows
    .window_title(|| String::from(APPNAME))
}