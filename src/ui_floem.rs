use std::fs;
use std::thread;
use std::time::Duration;
use std::sync::Arc;
use im::Vector;
use floem::{
    action::exec_after,
    event::{Event, EventListener}, 
    keyboard::{Key, NamedKey}, 
    menu::{Menu, MenuItem},
    peniko::Color, prelude::*, 
    reactive::{create_effect, create_signal, ReadSignal, SignalGet, SignalUpdate, WriteSignal}, style::{AlignContent, CursorStyle, Position}, text::Weight, views::{button, container, h_stack, label, scroll, v_stack, Decorators}, window::{new_window, WindowConfig, WindowId}, IntoView, View, ViewId
};
use crate::comms::{ServerEntry, ServerStatus};
use crate::constants::*;
use crate::{SERVER_STATUS, SERVER_CONFIG, ServerStatusInfo, mpsc::Receiver};
use crate::utils::widestring;

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
        Tab::Servers => container(server_stack(status_signal)),
        Tab::Log => container(log_view()),
    }
}

fn log_view() -> impl IntoView {
    // Read the log file and display its contents in a scrollable label
    let log_content = fs::read_to_string("log.txt").unwrap_or_else(|_| String::from("Failed to read log file."));
    let lines: Vector<String> = log_content.lines().map(|l| l.to_string()).collect();
    VirtualStack::new(move || lines.clone())
        .style(|s| {
            s.flex_col().items_center().class(LabelClass, |s| {
                s.padding_vert(2.5).width_full()
            })
        })
        .scroll()
        //.style(|s| s.size_pct(50., 75.).border(1.0))
        .container()
        .style(|s| {
            s.size(100.pct(), 100.pct())
                .padding_vert(20.0)
                .flex_col()
                .items_center()
                .justify_center()
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
                move |it| container(tab_content(it, status_signal)),
            )
            .style(|s| s.padding(CONTENT_PADDING).padding_bottom(10.0)),
        )
        .style(|s| s.flex_col().flex_basis(0).min_width(0).flex_grow(1.0)),
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

    //let server_status = status_signal.get().get(i).unwrap();//_or(&default_status);
    //let server_status = status_signal.unwrap_or(default_status);
    //let last_packet_time = server_status.last_packet_time.to_string();
    h_stack((
        v_stack((
            label(move || {name.clone()}).style(|s| s.font_size(20.0)),
            label(move || {
                status_signal.get().get(i)
                    .map(|s| s.last_packet_time.to_string())
                    .unwrap_or_else(|| "0".to_string())
            }).style(|s| s.font_size(20.0)),
            label(move || {
                status_signal.get().get(i)
                    .map(|s| if s.is_alive { "Alive".to_string() } else { "Not Alive".to_string() })
                    .unwrap_or_else(|| "Not Alive".to_string())
            }).style(move |s| {
                let is_alive = status_signal.get().get(i)
                    .map(|s| s.is_alive)
                    .unwrap_or(false);
                s.color(if is_alive { Color::from_rgb8(0, 255, 0) } else { Color::from_rgb8(255, 0, 0) })
            }),
        )),
        v_stack((
            h_stack((
                label(||"IP Address"),
                text_input(ip_address),
            )),
            h_stack((
                label(||"Port"),
                text_input(port),
            )),
        ))
        .style(|s| s.gap(5.0)),
        v_stack((
            button("Start Server").action(|| {
                println!("Start Server clicked!");
            }),
            button("Stop Server").action(|| {
                println!("Stop Server clicked!");
            }),
        )),
    ))
    .style(|s| s.padding(10.0).gap(10.0))
}

fn server_stack(status_signal: ReadSignal<Vec<ServerStatusInfo>>) -> impl IntoView {
    let last_packet_time_label = label(move || {
        status_signal.get().first().as_ref()
            .map(|s| s.last_packet_time.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    });

    unsafe {
        v_stack((
            list(
                SERVER_CONFIG.server
                    .iter()
                    .enumerate()
                    .map(|(i, _server)| {
                        //let status = status_signal.get().get(i).cloned();
                        server_view(i, status_signal)
                    })
            ),
            last_packet_time_label
        ))
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

    let menu_bar = container(
        label(||"File")
        .popout_menu(
        ||{window_menu()})
    );

    let view = v_stack((
        menu_bar,
        tab_navigation_view(status_signal),
        //server_stack,
    ))
    .style(|s| s.width_full().height_full().flex_col().align_content(AlignContent::FlexStart));

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