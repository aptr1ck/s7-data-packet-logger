use std::fs;
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
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
use crate::comms::ServerManager;
use crate::comms::{ServerEntry, ServerStatus, ServerCommand};
use crate::constants::*;
use crate::filehandling::file_tail;
use crate::{SERVER_CONFIG, ServerStatusInfo, mpsc::Receiver};
use crate::theme::*;
use crate::utils::*;

const LOG_LINES: usize = 500; // Number of lines to show in the log viewer
const TABBAR_HEIGHT: f64 = 37.0;
const CONTENT_PADDING: f64 = 10.0;
const BORDER_PADDING: f64 = 3.0;

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

fn tab_content(tab: Tab, status_signal: ReadSignal<ServerStatus>, command_tx: mpsc::UnboundedSender<ServerCommand>) -> impl IntoView {
    match tab {
        Tab::Servers => container(server_stack(status_signal, command_tx)).style(|s| s.width_full().height_full()),
        Tab::Log => container(log_view(status_signal)).style(|s| s.width_full().height_full()),
    }
}

fn log_view(status_signal: ReadSignal<ServerStatus>) -> impl IntoView {
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
                            .color(solarized_base0())
                            .padding_vert(1.0)
                            //.height(16.0) // Fixed height per line
                    })
            })
    )
    .style(|s| {
        s.width_full()
            .height_full()
            .padding(CONTENT_PADDING)
            .background(solarized_base3())
    })
}

fn tab_navigation_view(status_signal: ReadSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/, command_tx: mpsc::UnboundedSender<ServerCommand>) -> impl IntoView {
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
            .style(|s| s.padding_horiz(CONTENT_PADDING)
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
            .min_height(TABBAR_HEIGHT)
            .col_gap(5)
            .padding_left(CONTENT_PADDING as i32)
            .background(solarized_base3())
            .items_center()
    });

    //let status = status.clone();
    let main_content = container(
        scroll(
            tab(
                move || active_tab.get(),
                move || tabs.get(),
                |it| *it,
                move |it| container(tab_content(it, status_signal, command_tx.clone())).style(|s| s.width_full().height_full()),
            )
            .style(|s| s.width_full()
                                .flex_grow(1.0)),
        )
        .style(|s| s.width_full()
                            .flex_col()
                            .flex_grow(1.0)
                            ),
    )
    .style(|s| {
        s.items_start()
            .width_full()
            .height_full()
    });

    let settings_view = v_stack((tabs_bar, main_content))
        .style(|s| s.width_full()   
        .height_full()
        .padding(BORDER_PADDING)
        .gap(BORDER_PADDING)
    );
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

fn server_view(i: usize, status_signal: ReadSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/, command_tx: mpsc::UnboundedSender<ServerCommand>) -> impl IntoView {
    // Server Config Data
    let server = unsafe { &SERVER_CONFIG.server[i] };
    let name = RwSignal::new(server.name.clone());
    let ip_address = RwSignal::new(server.ip_address.clone());
    let port = RwSignal::new(server.port.clone().to_string());
    //let peer_ip = /*unsafe { SERVER_STATUS.get_ip_string(i) };*/status_signal.get().get_ip_string(i);//.unwrap_or_else(|| "x.x.x.x".to_string());
    // Create clones for each button that needs to use command_tx
    let start_command_tx = command_tx.clone();
    let stop_command_tx = command_tx.clone();

    // Main h-stack for server view
    h_stack((
        // Basic server info
        v_stack((
            //label(move || {name.clone()}).style(|s| s.font_size(20.0)),
            text_input(name).style(|s| s.font_size(20.0)
                                                        .background(Color::TRANSPARENT)
                                                        .hover(|s| s.background(Color::TRANSPARENT))
                                                        .padding(0.0)
                                                        .border(0.0)),
            label(move || {status_signal.get().get_ip_string(i)}).style(|s| s.font_size(12.0)),
            label(move || "").style(|s| s.font_size(6.0).flex_grow(1.0)), // Spacer
            label(move || {
                status_signal.get().server.get(i)
                    .map(|s| {
                        if s.last_packet_time == 0 {
                            "Not connected".to_string()
                        } else {
                            let secs = (s.last_packet_time / 1000) as i64;
                            let nsec = ((s.last_packet_time % 1000) * 1_000_000) as u32;
                            let datetime = Local.timestamp_opt(secs, nsec).unwrap();
                            datetime.format("Last packet at %Y-%m-%d %H:%M:%S").to_string()
                        }
                    })
                    .unwrap_or_else(|| "No server data".to_string())
            }).style(|s| s.font_size(12.0)
                                    .font_style(floem::text::Style::Italic)
                                    .color(solarized_base1())
                                ),
            h_stack((
                label(move || {
                    status_signal.get().server.get(i)
                        .map(|s| if s.is_running { "Running".to_string() } else { "Stopped".to_string() })
                        .unwrap_or_else(|| "Bork?".to_string())
                }).style(move |s| {
                    let is_running = status_signal.get().server.get(i)
                        .map(|s| s.is_running)
                        .unwrap_or(false);
                    s.color(if is_running { solarized_green() } else { solarized_red() })
                        .background(solarized_base2())
                        .border_radius(5.0)
                        .padding(5.0)
                }),
                label(move || {
                    status_signal.get().server.get(i)
                        .map(|s| if s.is_connected { "Connected".to_string() } else { "Disconnected".to_string() })
                        .unwrap_or_else(|| "Disconnected".to_string())
                }).style(move |s| {
                    let is_connected = status_signal.get().server.get(i)
                        .map(|s| s.is_connected)
                        .unwrap_or(false);
                    s.color(if is_connected { solarized_green() } else { solarized_base0() })
                        .background(solarized_base2())
                        .border_radius(5.0)
                        .padding(5.0)
                }),
                label(move || {
                    status_signal.get().server.get(i)
                        .map(|s| if s.is_alive { "Alive".to_string() } else { "Not Alive".to_string() })
                        .unwrap_or_else(|| "Not Alive".to_string())
                }).style(move |s| {
                    let is_alive = status_signal.get().server.get(i)
                        .map(|s| s.is_alive)
                        .unwrap_or(false);
                    s.color(if is_alive { solarized_green() } else { solarized_base0() })
                        .background(solarized_base2())
                        .border_radius(5.0)
                        .padding(5.0)
                }),
            )).style(|s| s.gap(5.0)),
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
            button("Save")
                .action(move || {
                    // Save the server configuration
                    let new_ip = ip_address.get().clone();
                    let new_port = port.get().parse::<u16>().unwrap_or(0);
                    let new_name = name.get().clone();
                    unsafe {
                        SERVER_CONFIG.server[i].ip_address = new_ip.clone();
                        SERVER_CONFIG.server[i].port = new_port;
                        SERVER_CONFIG.server[i].name = new_name.clone();
                        // Save config
                        if let Err(e) = crate::xmlhandling::save_config("config.xml") {//, &config) {
                            log(&format!("Failed to save config: {}", e));
                        } else {
                            log("Config saved.");
                        }
                        println!("Saved server {}: {}:{}", i, new_ip, new_port);
                    }
                }).style(|s| s.width(100.0).height(30.0)),
        )).style(|s| s.gap(5.0).items_end()),
        v_stack((
            {
                //let command_tx = command_tx.clone();
                button("Start Server").action(move || {
                    let _ = start_command_tx.send(ServerCommand::Start(i));
                }).style(|s| s.height_full())
            },
            {
                //let command_tx = command_tx.clone();
                button("Stop Server").action(move || {
                    let _ = stop_command_tx.send(ServerCommand::Stop(i));
                }).style(|s| s.height_full())
            },
        )).style(|s| s.gap(5.0)),
    ))
    .style(|s| s.background(solarized_base3())
                        .padding(CONTENT_PADDING)
                        .gap(BORDER_PADDING)
                        .width_full()
                        .flex_row())
}

fn server_stack(status_signal: ReadSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/, command_tx: mpsc::UnboundedSender<ServerCommand>) -> impl IntoView {
    unsafe {
        dyn_stack(
            move || (0..SERVER_CONFIG.server.len()).collect::<Vec<_>>(),
            |i| *i,
            move |i| server_view(i, status_signal, command_tx.clone())
            //.style(|s| s.background(solarized_base3()))
        ).style(|s| s.width_full()
                            .height_full()
                            .gap(CONTENT_PADDING)
                            )
    }
}

pub fn app_view(rx: Receiver<ServerStatusInfo>, command_tx: tokio::sync::mpsc::UnboundedSender<ServerCommand> ) -> impl IntoView {
    let (status_signal, set_status_signal) = create_signal(ServerStatus/*Vec::<ServerStatusInfo>*/::new());

    // Convert rx to Arc<Mutex<>> so we can share it between contexts
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let rx_clone = rx.clone();

    // Create a simple polling function
    fn schedule_poll(
        rx: std::sync::Arc<std::sync::Mutex<Receiver<ServerStatusInfo>>>,
        set_status_signal: WriteSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/
    ) {
        if let Ok(rx_guard) = rx.try_lock() {
            while let Ok(status) = rx_guard.try_recv() {
                set_status_signal.update(|statuses| {
                    // Update existing status or add new one
                    if let Some(existing) = statuses.server.iter_mut().find(|s| s.idx == status.idx) {
                        *existing = status;
                    } else {
                        statuses.server.push(status);
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
        tab_navigation_view(status_signal, command_tx),
        //server_stack,
    ))
    .style(|s| s.width_full()
                        .height_full()
                        .flex_col()
                        .background(solarized_base2())
                    );

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