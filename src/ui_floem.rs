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
    reactive::{create_effect, use_context, provide_context, create_signal, ReadSignal, SignalGet, SignalUpdate, WriteSignal}, 
    style::{AlignContent, CursorStyle, Position}, 
    text::Weight, views::{button, container, h_stack, label, scroll, v_stack, Decorators}, 
    window::{new_window, WindowConfig, WindowId}, 
    IntoView, View, ViewId
};
use crate::comms::ServerManager;
use crate::comms::{ServerEntry, ServerStatus, ServerCommand, generate_server_id};
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
                .height_full()
                .items_center()
                .justify_center()
                .color(solarized_base00())
                .background(solarized_base1_8())
                .hover(|s| s.font_weight(Weight::BOLD).color(solarized_base02()).cursor(CursorStyle::Pointer))
                .apply_if(
                    active_tab.get()
                        == tabs
                            .get_untracked()
                            .iter()
                            .position(|it| *it == this_tab)
                            .unwrap(),
                    |s| s.font_weight(Weight::BOLD).color(solarized_base01()).background(solarized_base2()),
                )
        })
}

fn tab_content(
    tab: Tab, status_signal: ReadSignal<ServerStatus>, 
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    server_config_signal: ReadSignal<Vec<ServerEntry>>,
    set_server_config_signal: WriteSignal<Vec<ServerEntry>>
) -> impl IntoView {
    match tab {
        Tab::Servers => container(server_stack(status_signal, command_tx, server_config_signal, set_server_config_signal)).style(|s| s.width_full().height_full()),
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
                    .class(LabelClass, |s| {
                        s.width_full()
                            .font_family("monospace".to_string()) 
                            .font_size(13.0)
                            .color(solarized_base01())
                            .background(solarized_base3())
                            .padding_horiz(CONTENT_PADDING/(2 as f64))
                            .padding_vert(1.0)
                    })
            })
    )
    .style(|s| {
        s.width_full()
            .height_full()
            .padding(CONTENT_PADDING/(2 as f64))
            .background(solarized_base2())
    })
}

fn tab_navigation_view(
    status_signal: ReadSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/, 
    command_tx: mpsc::UnboundedSender<ServerCommand>
) -> impl IntoView {
    let tabs = vec![Tab::Servers, Tab::Log]
        .into_iter()
        .collect::<Vector<Tab>>();
    let (tabs, _set_tabs) = create_signal(tabs);
    let (active_tab, set_active_tab) = create_signal(0);

    // Create the server config signals
    let (server_config_signal, set_server_config_signal) = create_server_config_signal();

    let tabs_bar = h_stack((
        tab_button(Tab::Servers, tabs, set_active_tab, active_tab),
        tab_button(Tab::Log, tabs, set_active_tab, active_tab),
    ))
    .style(|s| {
        s.flex_row()
            .width_full()
            .height(TABBAR_HEIGHT)
            .min_height(TABBAR_HEIGHT)
            .col_gap(2)
            //.padding_left(CONTENT_PADDING as i32)
            .background(solarized_base1_9())
            .items_center()
    });

    //let status = status.clone();
    let main_content = //scroll(
        //scroll(
            tab(
                move || active_tab.get(),
                move || tabs.get(),
                |it| *it,
                move |it| tab_content(it, status_signal, command_tx.clone(), server_config_signal, set_server_config_signal).style(|s| s.width_full().height_full()),
            )
            .style(|s| s.width_full()
                                .height_full()
                                .flex_grow(1.0))
        //)
        //.style(|s| s.width_full()
        //                    .flex_col()
        //                    .flex_grow(1.0)
        //                    ),
    /*)
    .style(|s| {
        s.items_start()
            .width_full()
            .height_full()
    })*/;

    let settings_view = v_stack((tabs_bar, main_content))
        .style(|s| s.width_full()   
        .height_full()
        //.gap(BORDER_PADDING)
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

fn server_view(
    server: ServerEntry,
    current_index: usize,
    //status_signal: ReadSignal<ServerStatus>, 
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    on_remove: impl Fn() + 'static + Clone
) -> impl IntoView {
    let status_signal = use_context::<ReadSignal<ServerStatus>>().expect("Server status signal not found in context");
    let name = RwSignal::new(server.name.clone());
    let ip_address = RwSignal::new(server.ip_address.clone());
    let port = RwSignal::new(server.port.to_string());
    let server_id = server.id.clone();
    
    // Clone a bunch of server IDs to avoid move errors.
    let server_id_1 = server_id.clone();
    let server_id_2 = server_id.clone();
    let server_id_3 = server_id.clone();
    let server_id_4 = server_id.clone();
    let server_id_5 = server_id.clone();
    let server_id_6 = server_id.clone();
    let server_id_7 = server_id.clone();
    let server_id_save = server_id.clone();

    let start_command_tx = command_tx.clone();
    let stop_command_tx = command_tx.clone();

    h_stack((
        // Basic server info
        v_stack((
            text_input(name).style(|s| s.font_size(20.0)
                                                        .background(Color::TRANSPARENT)
                                                        .color(solarized_base01())
                                                        .hover(|s| s.background(Color::TRANSPARENT))
                                                        .padding(0.0)
                                                        .border(0.0)),
            label(move || {
                let status = status_signal.get();
                // Find status by server ID instead of index
                status.server.iter()
                    .find(|s| s.matches_server_id(&server_id_1))
                    .map(|s| {
                        if s.peer_ip == [0; 16] {
                            "x.x.x.x".to_string()
                        } else {
                            status.get_ip_string(s.idx)
                        }
                    })
                    .unwrap_or_else(|| "b.o.r.k".to_string())
            }).style(|s| s.font_size(12.0).color(solarized_base01())),
            label(move || "").style(|s| s.font_size(6.0).flex_grow(1.0)), // Spacer
            label(move || {
                let server_id = server_id.clone();
                status_signal.get().server.iter()
                    .find(|s| s.matches_server_id(&server_id))
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
                    let server_id = server_id_2.clone();
                    let status = status_signal.get();
                    println!("Looking for server ID: {:?}", server_id);
                    println!("Available server statuses: {:?}", status.server.iter().map(|s| &s.server_id).collect::<Vec<_>>());
                    status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| if s.is_running { "Running".to_string() } else { "Stopped".to_string() })
                        .unwrap_or_else(|| "Unknown".to_string())
                }).style(move |s| {
                    let server_id = server_id_3.clone();
                    let is_running = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| s.is_running)
                        .unwrap_or(false);
                    s.color(if is_running { solarized_green() } else { solarized_red() })
                        .background(solarized_base2())
                        .border_radius(5.0)
                        .padding(5.0)
                }),
                label(move || {
                    let server_id = server_id_4.clone();
                    status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| if s.is_connected { "Connected".to_string() } else { "Disconnected".to_string() })
                        .unwrap_or_else(|| "Disconnected".to_string())
                }).style(move |s| {
                    let server_id = server_id_5.clone();
                    let is_connected = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| s.is_connected)
                        .unwrap_or(false);
                    s.color(if is_connected { solarized_green() } else { solarized_base0() })
                        .background(solarized_base2())
                        .border_radius(5.0)
                        .padding(5.0)
                }),
                label(move || {
                    let server_id = server_id_6.clone();
                    status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| if s.is_alive { "Alive".to_string() } else { "Not Alive".to_string() })
                        .unwrap_or_else(|| "Not Alive".to_string())
                }).style(move |s| {
                    let server_id = server_id_7.clone();
                    let is_alive = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
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
            )).style(|s| s.justify_end().gap(CONTENT_PADDING).items_center().color(solarized_base01())),
            h_stack((
                label(||"Port"),
                text_input(port),
            )).style(|s| s.justify_end().gap(CONTENT_PADDING).items_center().color(solarized_base01())),
            button("Save")
                .action(move || {
                    let new_ip = ip_address.get().clone();
                    let new_port = port.get().parse::<u16>().unwrap_or(0);
                    let new_name = name.get().clone();
                    
                    unsafe {
                        // Find and update the server by ID
                        if let Some(server) = SERVER_CONFIG.server.iter_mut().find(|s| s.id == server_id_save) {
                            server.ip_address = new_ip.clone();
                            server.port = new_port;
                            server.name = new_name.clone();
                            
                            if let Err(e) = crate::xmlhandling::save_config("config.xml") {
                                log(&format!("Failed to save config: {}", e));
                            } else {
                                log("Config saved.");
                            }
                            if DEBUG { println!("Saved server {:?}: {}:{}", server_id_save, new_ip, new_port); }
                        }
                    }
                }).style(|s| s.width(100.0).height(30.0).color(solarized_base01())),
        )).style(|s| s.gap(5.0).items_end()),
        v_stack((
            {
                button("Start Server").action(move || {
                    let _ = start_command_tx.send(ServerCommand::Start(current_index));
                }).style(|s| s.height_full().color(solarized_base01()))
            },
            {
                button("Stop Server").action(move || {
                    let _ = stop_command_tx.send(ServerCommand::Stop(current_index));
                }).style(|s| s.height_full().color(solarized_base01()))
            },
            {
                let on_remove = on_remove.clone();
                button("Remove Server").action(move || {
                    on_remove();
                }).style(|s| s.height_full().color(solarized_red()))
            },
        )).style(|s| s.gap(5.0)),
    ))
    .style(|s| s.background(solarized_base3())
                        .padding(CONTENT_PADDING)
                        .gap(BORDER_PADDING)
                        .width_full()
                        .flex_row())
}

// Updated server_stack function with proper keying
fn server_stack(
    status_signal: ReadSignal<ServerStatus>, 
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    server_config_signal: ReadSignal<Vec<ServerEntry>>,
    set_server_config_signal: WriteSignal<Vec<ServerEntry>>
) -> impl IntoView {
    let dyn_stack_command_tx = command_tx.clone();
    let remove_command_tx = command_tx.clone();
    
    // Create an effect to sync server_config_signal with the actual SERVER_CONFIG
    /*create_effect(move |_| {
        let _status = status_signal.get();
        let current_config = unsafe { SERVER_CONFIG.server.clone() };
        let signal_config = server_config_signal.get();
        
        if current_config.len() != signal_config.len() {
            set_server_config_signal.set(current_config);
        } else {
            let configs_different = current_config.iter().zip(signal_config.iter())
                .any(|(a, b)| a.id != b.id || a.name != b.name || a.ip_address != b.ip_address || a.port != b.port);
            
            if configs_different {
                set_server_config_signal.set(current_config);
            }
        }
    });*/
    // Only sync when the number of servers changes, not on every status update
    create_effect(move |prev_len| {
        let current_config = unsafe { SERVER_CONFIG.server.clone() };
        let current_len = current_config.len();
        
        if prev_len.map_or(true, |prev| prev != current_len) {
            set_server_config_signal.set(current_config);
            current_len
        } else {
            prev_len.unwrap_or(current_len)
        }
    });
    
    v_stack((
        dyn_stack(
            // Use server entries with their IDs as the data source
            move || server_config_signal.get(),
            // Key by server ID instead of index
            |server| server.id.clone(),
            move |server| {
                // Find the current index of this server in the config
                let server_id = server.id.clone();
                let current_index = server_config_signal.get()
                    .iter()
                    .position(|s| s.id == server_id)
                    .unwrap_or(0);
                
                let server_command_tx = dyn_stack_command_tx.clone();
                let remove_server_command_tx = remove_command_tx.clone();
                let server_id_for_removal = server_id.clone();
                
                server_view(
                    server.clone(), 
                    current_index,
                    //status_signal, 
                    server_command_tx, 
                    move || {
                        // Remove by server ID, not index
                        let config = server_config_signal.get();
                        if let Some(index_to_remove) = config.iter().position(|s| s.id == server_id_for_removal) {
                            let _ = remove_server_command_tx.send(ServerCommand::RemoveServer(index_to_remove));
                            
                            // Update the signal immediately
                            set_server_config_signal.update(|config| {
                                config.retain(|s| s.id != server_id_for_removal);
                            });
                        }
                    }
                )
                .style(|s| s.background(solarized_base3()))
            }
        ).style(|s| s.flex_col()
                            .width_full()
                            .height_full()
                            .padding(BORDER_PADDING)
                            .gap(CONTENT_PADDING)
                            ),
        h_stack((
            label(||"").style(|s| s.width_full()), //Spacer
            button("Add Server")
            .action(move || {
                let new_server = ServerEntry {
                    id: generate_server_id(),
                    name: "New Server".to_string(),
                    ip_address: "0.0.0.0".to_string(),
                    port: 2000,
                    autostart: false,
                };
                
                let _ = command_tx.send(ServerCommand::AddServer(new_server.clone()));
                set_server_config_signal.update(|config| {
                    config.push(new_server);
                });
            }),
            label(||"").style(|s| s.width_full()), //Spacer
        )),
        label(||"").style(|s| s.height_full()), // Veritcal Spacer
    )).style(|s| s.width_full()
                            .height_full()
                            .padding(BORDER_PADDING)
                            .gap(CONTENT_PADDING))
}

// Create a signal for the server configuration (for reactive UI)
fn create_server_config_signal() -> (ReadSignal<Vec<ServerEntry>>, WriteSignal<Vec<ServerEntry>>) {
    // Initialize with the current config
    let initial_config = unsafe { SERVER_CONFIG.server.clone() };
    create_signal(initial_config)
}

pub fn app_view(rx: Receiver<ServerStatusInfo>, command_tx: tokio::sync::mpsc::UnboundedSender<ServerCommand> ) -> impl IntoView {
    let (status_signal, set_status_signal) = create_signal(ServerStatus/*Vec::<ServerStatusInfo>*/::new());
    provide_context(status_signal);

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

    /*create_effect(move |_| {
        let status = status_signal.get();
        let current_config = server_config_signal.get();
        
        // If the number of servers in status doesn't match config, sync them
        if status.server.len() != current_config.len() {
            // Update the config signal to match the actual server status
            let new_config: Vec<ServerEntry> = (0..status.server.len())
                .map(|i| {
                    unsafe {
                        SERVER_CONFIG.server.get(i).cloned().unwrap_or(ServerEntry {
                            name: format!("Server {}", i),
                            ip_address: "0.0.0.0".to_string(),
                            port: 2000,
                            autostart: false,
                        })
                    }
                })
                .collect();
            
            set_server_config_signal.set(new_config);
        }
    });*/

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