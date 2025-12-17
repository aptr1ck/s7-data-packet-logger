use std::time::Duration;
use floem::action::drag_resize_window;
use floem::reactive::SignalRead;
use tokio::sync::mpsc;
use chrono::Local;
use chrono::TimeZone;
use im::Vector;
use floem::{
    action::{exec_after, set_window_menu},
    event::{Event, EventListener}, 
    kurbo::Point,
    menu::{Menu, SubMenu},
    peniko, prelude::*, 
    peniko::kurbo::Rect,
    reactive::{UpdaterEffect, use_context, provide_context, RwSignal, ReadSignal, SignalGet, SignalUpdate, WriteSignal}, 
    style::{CursorStyle, Style}, 
    text::Weight, 
    views::{button, container, h_stack, label, scroll, v_stack, Decorators}, 
    window::{ResizeDirection},
    IntoView, View, 
};
use std::io::Cursor;
use syntect::highlighting::{
    /*FontStyle, HighlightIterator,*/ Color as SynColor, /*HighlightState, Highlighter, RangedHighlightIterator,*/ Theme, ThemeSet
};
use syntect::parsing::Scope; //{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet, };
use syntect_assets::assets::HighlightingAssets;
use crate::app_config::{AppCommand, /*AppConfig,*/ ThemeNameSig};
use crate::comms::{ServerEntry, ServerStatus, ServerCommand, generate_server_id};
use crate::constants::*;
use crate::filehandling::file_tail;
use crate::{SERVER_CONFIG, ServerStatusInfo, mpsc::Receiver};
use crate::utils::*;

const LOG_LINES: usize = 500; // Number of lines to show in the log viewer
const TABBAR_HEIGHT: f64 = 37.0;
const CONTENT_PADDING: f64 = 10.0;
const BORDER_PADDING: f64 = 3.0;
const RESIZE_BORDER: f64 = 5.0; // Pixels from the edge that trigger resizing

#[derive(Clone, Copy, PartialEq)]
enum ResizeEdge {
    None,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum Tab {
    Servers,
    Log,
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Tab::Servers => write!(f, "Connections"),
            Tab::Log => write!(f, "Log"),
        }
    }
}

// Build a ThemeSet from the embedded assets
lazy_static::lazy_static! {
    pub static ref THEMES: ThemeSet = {
        let _assets = HighlightingAssets::from_binary();
        let mut ts = ThemeSet::new();    

        // --- Add embedded .tmTheme files here ---
        // Use absolute path from crate root to be resilient regardless of module location
        let embedded: &[(&str, &[u8])] = &[
            // TODO: Monokai, Dracula, Cobalt
            ( "Default", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Default.tmTheme")), ),
            ( "Everforest Dark", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Everforest Dark.tmTheme")), ),
            ( "Everforest Light", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Everforest Light.tmTheme")), ),
            ( "Fairyfloss", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Fairyfloss.tmTheme")), ),
            ( "Lucky Charms", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Lucky Charms.tmTheme")), ),
            ( "Rosé Pine Dawn", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/RosePineDawn.tmTheme")), ),
            ( "Rosé Pine Moon", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/RosePineMoon.tmTheme")), ),
            ( "Solarized Dark", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/SolarizedDark.tmTheme")), ),
            ( "Solarized Light", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/SolarizedLight.tmTheme")), ),
            ( "Zenburn", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/themes/Zenburn.tmTheme")), ),
        ];

        for (name, bytes) in embedded {
            let mut cursor = Cursor::new(*bytes);
            let theme = ThemeSet::load_from_reader(&mut cursor)
                .expect("Failed to load embedded .tmTheme");
            ts.themes.insert((*name).to_string(), theme);
        }
        ts
    };
}

fn get_resize_edge(pos: Point, width: f64, height: f64) -> ResizeEdge {
    let left = pos.x <= RESIZE_BORDER;
    let right = pos.x >= width - RESIZE_BORDER;
    let top = pos.y <= RESIZE_BORDER;
    let bottom = pos.y >= height - RESIZE_BORDER;

    match (left, right, top, bottom) {
        (true, false, true, false) => ResizeEdge::TopLeft,
        (false, true, true, false) => ResizeEdge::TopRight,
        (true, false, false, true) => ResizeEdge::BottomLeft,
        (false, true, false, true) => ResizeEdge::BottomRight,
        (true, false, false, false) => ResizeEdge::Left,
        (false, true, false, false) => ResizeEdge::Right,
        (false, false, true, false) => ResizeEdge::Top,
        (false, false, false, true) => ResizeEdge::Bottom,
        _ => ResizeEdge::None,
    }
}

// Core helper: find the last matching theme rule for a given *stack of scopes*
// and return (foreground, background) if any. Uses TextMate’s “last rule wins”
// behavior for ties in specificity.
fn colors_for_scopes(
    theme: &Theme,
    scopes: &[Scope],
) -> Option<(Option<SynColor>, Option<SynColor>)> {
    let mut fg: Option<SynColor> = None;
    let mut bg: Option<SynColor> = None;

    for item in &theme.scopes {
        if item.scope.does_match(scopes).is_some() {
            if let Some(c) = item.style.foreground { fg = Some(c); }
            if let Some(c) = item.style.background { bg = Some(c); }
        }
    }
    if fg.is_some() || bg.is_some() { Some((fg, bg)) } else { None }
}

// Convenience: build a single-element scope slice from a selector string.
fn colors_for_scope_selector(
    theme: &Theme,
    selector: &str,
) -> Option<(Option<SynColor>, Option<SynColor>)> {
    let scope = Scope::new(selector).ok()?;
    colors_for_scopes(theme, std::slice::from_ref(&scope))
}

fn get_current_theme() -> &'static Theme {
    // Try to read the ThemeNameSig from context. If it's not available (for
    // example during early startup), fall back to the embedded "Default"
    // theme instead of panicking.
    if let Some(ThemeNameSig(theme_name_sig)) = use_context::<ThemeNameSig>() {
        THEMES
            .themes
            .get(theme_name_sig.get().as_str())
            .unwrap_or_else(|| THEMES.themes.get("Default").unwrap())
    } else {
        // No theme context provided yet — use Default.
        THEMES.themes.get("Default").unwrap()
    }
}

struct ThemeColors {
    fg: peniko::Color,
    ac: peniko::Color,
    bg: peniko::Color,
    bg1: peniko::Color,
    bg2: peniko::Color,
    bgh: peniko::Color,
    red: peniko::Color,
    green: peniko::Color,
}

fn get_theme_colors() -> ThemeColors {
    let theme = get_current_theme();
    let fg_color = theme.settings.foreground.unwrap_or(syntect::highlighting::Color::BLACK);
    let fg = peniko::Color::from_rgba8(fg_color.r, fg_color.g, fg_color.b, fg_color.a);

    let active_colour = theme.settings.background.unwrap_or(syntect::highlighting::Color::WHITE);
    let ac = peniko::Color::from_rgba8(active_colour.r, active_colour.g, active_colour.b, active_colour.a);

    let bg_color = theme.settings.background.unwrap_or(syntect::highlighting::Color::WHITE);
    let bg = peniko::Color::from_rgba8(bg_color.r, bg_color.g, bg_color.b, bg_color.a);

    let c = colors_for_scope_selector(theme, "ui.sidebar")
            .and_then(|(_, bg)| bg)  // prefer a theme-provided background for headings
            .unwrap_or(syntect::highlighting::Color { r: 128, g: 128, b: 128, a: 40 });
    let bg1 = peniko::Color::from_rgba8(c.r, c.g, c.b, c.a);

    let c = colors_for_scope_selector(theme, "ui.selected")
            .and_then(|(_, bg)| bg)  // prefer a theme-provided background for headings
            .unwrap_or(syntect::highlighting::Color { r: 128, g: 128, b: 128, a: 40 });
    let bg2 = peniko::Color::from_rgba8(c.r, c.g, c.b, c.a);

    let c = colors_for_scope_selector(theme, "ui.hover")
            .and_then(|(_, bg)| bg)  // prefer a theme-provided background for headings
            .unwrap_or(syntect::highlighting::Color { r: 128, g: 128, b: 128, a: 40 });
    let bgh = peniko::Color::from_rgba8(c.r, c.g, c.b, c.a);

    let c = colors_for_scope_selector(theme, "green")
                        .and_then(|(fg, _)| fg)  // prefer a theme-provided background for headings
                        .unwrap_or(syntect::highlighting::Color { r: 128, g: 128, b: 128, a: 40 });
    let green = peniko::Color::from_rgba8(c.r, c.g, c.b, c.a);
    
    let c = colors_for_scope_selector(theme, "red")
                        .and_then(|(fg, _)| fg)  // prefer a theme-provided background for headings
                        .unwrap_or(syntect::highlighting::Color { r: 128, g: 128, b: 128, a: 40 });
    let red = peniko::Color::from_rgba8(c.r, c.g, c.b, c.a);

    ThemeColors { fg, ac, bg, bg1, bg2, bgh, red, green }
}

unsafe fn menu_item_style() -> floem::style::Style {
    let colors = get_theme_colors();
    
    floem::style::Style::new()
        .background(peniko::Color::TRANSPARENT)
        .color(colors.fg)
        .font_size(FONT_SIZE_MENU)
        .height(MENU_HEIGHT)
        .padding_horiz(CONTENT_PADDING)
        .items_center()
        .selectable(false)
}

unsafe fn window_control_buttons_style() -> floem::style::Style {
    let colors = get_theme_colors();

    floem::style::Style::new()
    .background(peniko::Color::TRANSPARENT)
        .color(colors.fg)
        .font_size(FONT_SIZE_WIN_CONTROLS)
        .height(MENU_HEIGHT)
        .padding_horiz(CONTENT_PADDING)
        .width(50.0)
        .items_center()
        .selectable(false)
        .border(0)
        .font_family("Segoe MDL2 Assets".to_string())
        .hover(|s| s.background(colors.bg1).color(colors.fg))
        .focus(|s| s.background(peniko::Color::TRANSPARENT).color(colors.fg).hover(|s| s.background(peniko::Color::TRANSPARENT).color(colors.fg)))
        .selected(|s| s.background(peniko::Color::TRANSPARENT).color(colors.fg).hover(|s| s.background(peniko::Color::TRANSPARENT).color(colors.fg)))
        //.pressed???
}

fn button_style() -> Style {
    let colors = get_theme_colors();

    Style::new()
        .color(colors.fg)
        .background(colors.bg)
        .border_color(colors.fg)
        .hover(|s| s.background(colors.bgh).border_color(colors.fg).hover(|s| s.background(colors.bgh).border_color(colors.fg)))
        .focus(|s| s.background(colors.bg1).border_color(colors.fg).hover(|s| s.background(colors.bgh).border_color(colors.fg)))
        .active(|s| s.background(colors.bg1).border_color(colors.fg).hover(|s| s.background(colors.bgh).border_color(colors.fg)))
        .selected(|s| s.background(colors.bg1).border_color(colors.fg).hover(|s| s.background(colors.bgh).border_color(colors.fg)))
}

fn input_style() -> Style {
    let colors = get_theme_colors();

    Style::new()
        .color(colors.fg)
        .background(colors.ac)
        .border_color(colors.fg)
        .hover(|s| s.background(colors.ac).border_color(colors.fg))
        .selected(|s| s.background(colors.ac).border_color(colors.fg))
        .focus(|s| s.background(colors.ac).border_color(colors.fg).hover(|s| s.background(colors.ac).border_color(colors.fg)))
        .active(|s| s.background(colors.ac).border_color(colors.fg))
}

fn checkbox_style() -> Style {
    let colors = get_theme_colors();
    
    Style::new()
        .color(colors.fg)
        .background(colors.bg)
        .border_color(colors.fg)
        .hover(|s| s.background(colors.bg).border_color(colors.fg))
        .selected(|s| s.background(colors.bg).border_color(colors.fg))
        .focus(|s| s.background(colors.ac).border_color(colors.fg).hover(|s| s.background(colors.ac).border_color(colors.fg)))
        .active(|s| s.background(colors.ac).border_color(colors.fg))
}

fn tab_button(
    this_tab: Tab,
    tabs: ReadSignal<Vector<Tab>>,
    active_tab: RwSignal<usize>,
) -> impl IntoView {
    label(move || this_tab)
        .on_click_stop(move |_| {
            active_tab.update(|v: &mut usize| {
                *v = tabs
                    .get_untracked()
                    .iter()
                    .position(|it| *it == this_tab)
                    .unwrap();
            });
        })
        .style(move |s| {
            let colors = get_theme_colors();
            s.width(100)
                .height_full()
                .items_center()
                .justify_center()
                .color(colors.fg)
                .background(colors.bg)
                .focusable(true)
                .hover(|s| s.font_weight(Weight::BOLD).color(colors.fg).cursor(CursorStyle::Pointer))
                .apply_if(
                    active_tab.get()
                        == tabs
                            .get_untracked()
                            .iter()
                            .position(|it| *it == this_tab)
                            .unwrap(),
                    |s| s.font_weight(Weight::BOLD).color(colors.fg).background(colors.bg1),
                )
        })
}

fn tab_content(
    tab: Tab, 
    status_signal: ReadSignal<ServerStatus>, 
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    server_config_signal: RwSignal<Vec<ServerEntry>>,
) -> impl IntoView {
    match tab {
        Tab::Servers => container(server_stack(status_signal, command_tx, server_config_signal))
            .style(|s| {
                let colors = get_theme_colors();
                s.width_full().flex_grow(1.0).background(colors.bg)
            }),
        Tab::Log => container(scroll(log_view(status_signal)))
            .style(|s| {
                let colors = get_theme_colors();
                s.width_full().flex_grow(1.0).background(colors.bg1)
            }),
    }
}

fn log_view(status_signal: ReadSignal<ServerStatus>) -> impl IntoView {
    let log_lines_signal = RwSignal::new(Vec::<String>::new());
    
    // Create an effect to update the log
    UpdaterEffect::new(
        move || status_signal.get(),
        move |status_signal| {
            let log_content = file_tail("log.txt", LOG_LINES)
                .unwrap_or_else(|_| String::from("Failed to read log file."));
            let lines: Vec<String> = log_content.lines().map(|l| l.to_string()).collect();
            log_lines_signal.set(lines);
        },
    );

    let lines_read = log_lines_signal.read_only();
    // Render each log line as its own row using dyn_stack
    dyn_stack(
        move || {
            let v = lines_read.get();
            (0..v.len()).collect::<Vec<usize>>()
        },
        |idx| *idx,
        move |idx| {
            let lines_read = lines_read.clone();
            label(move || lines_read.get().get(idx).cloned().unwrap_or_default())
                .style(|s| {
                    let colors = get_theme_colors();
                    s.width_full()
                        .font_family("monospace".to_string())
                        .font_size(13.0)
                        .color(colors.fg)
                        .background(colors.bg1)
                })
        },
    )
    .style(|s| {
        let colors = get_theme_colors();
        s.flex_col()
            .width_full()
            .padding(CONTENT_PADDING)
            .background(colors.bg1)
    }).scroll()
}

fn tab_navigation_view(
    status_signal: ReadSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/, 
    command_tx: mpsc::UnboundedSender<ServerCommand>
) -> impl IntoView {
    let tabs = vec![Tab::Servers, Tab::Log]
        .into_iter()
        .collect::<Vector<Tab>>();
    let tabs = RwSignal::new(tabs);
    let active_tab = RwSignal::new(0);

    // Create the server config signals
    let server_config_signal = create_server_config_signal();

    let tabs_bar = h_stack((
        tab_button(Tab::Servers, tabs.read_only(), active_tab),
        tab_button(Tab::Log, tabs.read_only(), active_tab),
    ))
    .style(move |s| {
        let colors = get_theme_colors();
        s.flex_row()
            .width_full()
            .height(TABBAR_HEIGHT)
            .min_height(TABBAR_HEIGHT)
            .col_gap(2)
            .background(colors.bg)
            .items_center()
    });

    let main_content = 
        tab(
            move || Some(active_tab.get()),
            move || tabs.get(),
            |it| *it,
            move |it| tab_content(it, status_signal, command_tx.clone(), server_config_signal).style(|s| s.width_full()),
        )
        .style(|s| s.width_full().flex_grow(1.0).min_height(0.0));

    let navigation_view = v_stack((tabs_bar, main_content))
        .style(|s| {
            let colors = get_theme_colors();
            s.size_full().background(colors.bg1)
        }
    );
    navigation_view
}

fn server_view(
    server: ServerEntry,
    current_index: usize,
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    on_remove: impl Fn() + 'static + Clone
) -> impl IntoView {
    let status_signal = use_context::<ReadSignal<ServerStatus>>().expect("Server status signal not found in context");
    let name = RwSignal::new(server.name.clone());
    let ip_address = RwSignal::new(server.ip_address.clone());
    let port = RwSignal::new(server.port.to_string());
    let server_id = server.id.clone();
    let autostart = RwSignal::new(server.autostart);

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

        // Get an approximate width for the IP address input
    let sample = "255.255.255.255";
    let font_size = 13.0;
    // factor 0.6 is a reasonable approximation of average glyph width / font_size
    let approx_char_width = font_size * 0.6;
    let min_ip_width = (sample.len() as f64) * approx_char_width;

    h_stack((
        // Basic server info
        v_stack((
            text_input(name).style(move |s| {
                let colors = get_theme_colors();
                s.font_size(20.0)
                                                        .background(Color::TRANSPARENT)
                                                        .color(colors.fg)
                                                        .hover(|s| s.background(Color::TRANSPARENT))
                                                        .padding(0.0)
                                                        .border(0.0)
                                                        .min_width(150.0)
            }),
            label(move || {
                let status = status_signal.get();
                // Find status by server ID instead of index
                status.server.iter()
                    .find(|s| s.matches_server_id(&server_id_1))
                    .map(|s| {
                        if s.peer_ip == [0; 16] {
                            "".to_string()
                        } else {
                            status.get_ip_string(s.idx)
                        }
                    })
                    .unwrap_or_else(|| "b.o.r.k".to_string())
            }).style(move |s| {
                let colors = get_theme_colors();
                s.font_size(12.0).color(colors.fg)
            }),
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
            }).style(move |s| {
                let colors = get_theme_colors();
                s.font_size(12.0)
                                    .font_style(floem::text::Style::Italic)
                                    .color(colors.fg)
                }),
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
                    let colors = get_theme_colors();
                    let server_id = server_id_3.clone();
                    let is_running = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| s.is_running)
                        .unwrap_or(false);
                    s.color(if is_running { colors.green } else { colors.red })
                        .background(colors.bg)
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
                    let colors = get_theme_colors();
                    let server_id = server_id_5.clone();
                    let is_connected = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| s.is_connected)
                        .unwrap_or(false);
                    s.color(if is_connected { colors.green } else { colors.red })
                        .background(colors.bg)
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
                    let colors = get_theme_colors();
                    let server_id = server_id_7.clone();
                    let is_alive = status_signal.get().server.iter()
                        .find(|s| s.matches_server_id(&server_id))
                        .map(|s| s.is_alive)
                        .unwrap_or(false);
                    s.color(if is_alive { colors.green } else { colors.red })
                        .background(colors.bg)
                        .border_radius(5.0)
                        .padding(5.0)
                }),
            )).style(|s| s.gap(5.0)),
        )).style(|s| s.flex_grow(1.0).gap(8.0).items_start()),
        // Input fields for IP and Port
        v_stack((
            h_stack((
                label(||"Local IP Address on PLC Network"),
                text_input(ip_address).style(move |_| input_style().min_width(min_ip_width)),
            )).style(move |s| {
                let colors = get_theme_colors();
                s.justify_end().gap(CONTENT_PADDING).items_center().color(colors.fg)
            }),
            h_stack((
                label(||"Port"),
                text_input(port).style(move |_| input_style().min_width(min_ip_width)),
            )).style(move |s| {
                let colors = get_theme_colors();
                s.justify_end().gap(CONTENT_PADDING).items_center().color(colors.fg)
            }),
            h_stack((
                label(||"Auto start").style(move |s| {
                    let colors = get_theme_colors();
                    s.color(colors.fg)
                }),
                checkbox(move || autostart.get())
                .style(|_| checkbox_style())
                .on_update(move |is_checked| {
                    autostart.set(is_checked);
                })
            )).style(|s| s.items_center().gap(CONTENT_PADDING)),
            button("Save")
                .action(move || {
                    let new_ip = ip_address.get().clone();
                    let new_port = port.get().parse::<u16>().unwrap_or(0);
                    let new_name = name.get().clone();
                    let new_autostart = autostart.get();

                    unsafe {
                        // Find and update the server by ID
                        if let Some(server) = SERVER_CONFIG.server.iter_mut().find(|s| s.id == server_id_save) {
                            server.ip_address = new_ip.clone();
                            server.port = new_port;
                            server.name = new_name.clone();
                            server.autostart = new_autostart;
                            
                            if let Err(e) = crate::xmlhandling::save_config("config.xml") {
                                log(&format!("Failed to save config: {}", e));
                            } else {
                                log("Config saved.");
                            }
                            if DEBUG { println!("Saved server {:?}: {}:{}", server_id_save, new_ip, new_port); }
                        }
                    }
                }).style(|_| button_style().width(100.0).height(30.0)),
        )).style(|s| s.gap(5.0).items_end()),
        v_stack((
            {
                button("Start Server").action(move || {
                    let _ = start_command_tx.send(ServerCommand::Start(current_index));
                }).style(|_| button_style().height_full())
            },
            {
                button("Stop Server").action(move || {
                    let _ = stop_command_tx.send(ServerCommand::Stop(current_index));
                }).style(|_| button_style().height_full())
            },
            {
                let on_remove = on_remove.clone();
                button("Remove Server").action(move || {
                    on_remove();
                }).style(move |_| button_style().height_full())
            },
        )).style(|s| s.gap(5.0)),
    ))
    .style(move |s| {
        let colors = get_theme_colors();
        s.background(colors.bg2)
                        .padding(CONTENT_PADDING)
                        .gap(CONTENT_PADDING)
                        .width_full()
                        .flex_row()
        })
}

// Updated server_stack function with proper keying
fn server_stack(
    _status_signal: ReadSignal<ServerStatus>, 
    command_tx: mpsc::UnboundedSender<ServerCommand>,
    server_config_signal: RwSignal<Vec<ServerEntry>>,
    //set_server_config_signal: WriteSignal<Vec<ServerEntry>>
) -> impl IntoView {
    let dyn_stack_command_tx = command_tx.clone();
    let remove_command_tx = command_tx.clone();

    // Only sync when the number of servers changes, not on every status update
    UpdaterEffect::new(
        move || unsafe {SERVER_CONFIG.server.clone()},
        move |current_config| {
            server_config_signal.set(current_config);
        },
    );
    
    v_stack((
        /*container(*/scroll(
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
                                server_config_signal.update(|config| {
                                    config.retain(|s| s.id != server_id_for_removal);
                                });
                            }
                        }
                    )
                    .style(move |s| {
                        let colors = get_theme_colors();
                        s.background(colors.bgh)
                    })
                }
            ).style(|s| s.flex_col()
                                .width_full()
                                .min_height(0.0) // allow shrinking, and therefore constraining to parent height
                                .padding(BORDER_PADDING)
                                .gap(CONTENT_PADDING)
                                ),
        ).style(|s| s.flex_grow(1.0).width_full()),
        h_stack((
            label(||"").style(|s| s.width_full()), //Spacer
            button("Add Connection")
            .action(move || {
                let new_server = ServerEntry {
                    id: generate_server_id(),
                    name: "New Server".to_string(),
                    ip_address: "0.0.0.0".to_string(),
                    port: 2000,
                    autostart: false,
                };
                let _ = command_tx.send(ServerCommand::AddServer(new_server.clone()));
                server_config_signal.update(|config| {
                    config.push(new_server);
                });
            }).style(|_| button_style().height(30.0)),
            label(||"").style(|s| s.width_full()), //Spacer
        )).style(|s| s.items_start())//,
    )).style(move |s| {
        let colors = get_theme_colors();
        s.size_full().flex_col()
            .padding(BORDER_PADDING)
            .gap(CONTENT_PADDING)
            .background(colors.bg1)
    })
}

fn custom_window_menu() -> impl IntoView {
    // Theme Info
    let themes_list = &THEMES.themes;
    let ThemeNameSig(theme_name_sig) = use_context::<ThemeNameSig>().expect("ThemeNameSig not found");
    // Pull CommandRegistry from context (provided in app_config)
    let registry_sig = use_context::<RwSignal<crate::app_config::CommandRegistry>>()
        .expect("CommandRegistry not found");
    let registry = registry_sig.get_untracked();
    let button_registry = registry.clone();
    let registry_min = button_registry.clone();
    let registry_max = button_registry.clone();
    let registry_quit = button_registry.clone();
    
    //
    let menu = h_stack((
        label(|| "File")
            .popout_menu(move || {
                //let registry_new = registry.clone();
                let registry_quit = registry.clone();
                Menu::new()
                //.item("New", |i| i.action(move || {
                //    registry_new.execute(AppCommand::NewFile);
                //}))
                //.separator()
                .item("Exit", |i| i.action(move || {
                    registry_quit.execute(AppCommand::Quit);
                }))
            })
            .style(|_| unsafe{menu_item_style()}),
        label(|| "View")
            .popout_menu(move || {
                // Build a menu list of themes
                Menu::new()
                .submenu("Theme", |mut sm| {
                    let theme_names: Vec<String> = themes_list.keys().cloned().collect();
                    for name in theme_names {
                        let is_current = theme_name_sig.get() == name;
                        let label = if is_current { format!("• {}", name) } else { name.clone() };
                        let to_set = name.clone();
                        sm = sm.item(label, |i| i.action(move || theme_name_sig.set(to_set.clone())));
                    }
                    sm
                })
            })
        .style(|_| unsafe{menu_item_style()}),
        drag_window_area(container(label(|| "")).style(|s| s.flex_grow(1.0)))
            .style(|s| s.flex_grow(1.0)), // Spacer that grows to fill space   
        h_stack(( 
        button("\u{E949}").style(|_| unsafe{ window_control_buttons_style() }) // Minimize
        .action(move || {
            registry_min.execute(AppCommand::Minimize);
        }),
        button("\u{E739}").style(|_| unsafe{ window_control_buttons_style() }) // Maxmimise
        .action(move || {
            registry_max.execute(AppCommand::Maximize);
        }),
        button("\u{E106}").style(|_| unsafe{ window_control_buttons_style() }) // Close
        .action(move || {
            registry_quit.execute(AppCommand::Quit);
        }),
        )).style(|s| s.gap(0.0)),
    )).style(move |s| {
        let colors = get_theme_colors();
        s.background(colors.bg2)
            .height(MENU_HEIGHT)
            .padding_left(CONTENT_PADDING)
            .items_center()
            .gap(CONTENT_PADDING)}
        );

    menu
}

// Create a signal for the server configuration (for reactive UI)
fn create_server_config_signal() -> RwSignal<Vec<ServerEntry>> {//ReadSignal<Vec<ServerEntry>>, WriteSignal<Vec<ServerEntry>>) {
    // Initialize with the current config
    let initial_config = unsafe { SERVER_CONFIG.server.clone() };
    RwSignal::new(initial_config)
}

pub fn app_view(rx: Receiver<ServerStatusInfo>, command_tx: tokio::sync::mpsc::UnboundedSender<ServerCommand> ) -> impl IntoView {
    let status_signal = RwSignal::new(ServerStatus/*Vec::<ServerStatusInfo>*/::new());
    provide_context(status_signal.read_only());
    
    // Convert rx to Arc<Mutex<>> so we can share it between contexts
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let rx_clone = rx.clone();

    // Create a simple polling function
    fn schedule_poll(
        rx: std::sync::Arc<std::sync::Mutex<Receiver<ServerStatusInfo>>>,
        set_status_signal: RwSignal<ServerStatus>/*Vec<ServerStatusInfo>>*/
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
    schedule_poll(rx_clone, status_signal);

    let view = stack((
        v_stack((
            custom_window_menu(),
            tab_navigation_view(status_signal.read_only(), command_tx)
                .style(|s| {
                    let colors = get_theme_colors();
                    s.width_full().flex_grow(1.0).min_height(0.0).background(colors.bg1)
                }),
        )).style(|s| {
            let colors = get_theme_colors();
            s.border(RESIZE_HANDLE_SIZE / 2.0).size_full().background(colors.bg)
        }),
        // Right edge resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_right(0.0)
            .inset_top(0.0)
            .width(RESIZE_HANDLE_SIZE)
            .height_full()
            .cursor(CursorStyle::EResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::East);
        }),
        // Bottom edge resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_bottom(0.0)
            .inset_left(0.0)
            .height(RESIZE_HANDLE_SIZE)
            .width_full()
            .cursor(CursorStyle::SResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::South);
        }),
        // Left edge resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_top(0.0)
            .inset_left(0.0)
            .width(RESIZE_HANDLE_SIZE)
            .height_full()
            .cursor(CursorStyle::WResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::West);
        }),
        // Top edge resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_top(0.0)
            .inset_left(0.0)
            .height(RESIZE_HANDLE_SIZE)
            .width_full()
            .cursor(CursorStyle::NResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::North);
        }),
        // Bottom left corner resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_bottom(0.0)
            .inset_left(0.0)
            .height(RESIZE_HANDLE_SIZE * 2.0)
            .width(RESIZE_HANDLE_SIZE * 2.0)
            .cursor(CursorStyle::SwResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::SouthWest);
        }),
        // Bottom right corner resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_bottom(0.0)
            .inset_right(0.0)
            .height(RESIZE_HANDLE_SIZE * 2.0)
            .width(RESIZE_HANDLE_SIZE * 2.0)
            .cursor(CursorStyle::SeResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::SouthEast);
        }),
        // Top left corner resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_top(0.0)
            .inset_left(0.0)
            .height(RESIZE_HANDLE_SIZE * 2.0)
            .width(RESIZE_HANDLE_SIZE * 2.0)
            .cursor(CursorStyle::NwResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::NorthWest);
        }),
        // Top right corner resize handle
        empty().style(move |s| {
            s.position(floem::style::Position::Absolute)
            .inset_top(0.0)
            .inset_right(0.0)
            .height(RESIZE_HANDLE_SIZE * 2.0)
            .width(RESIZE_HANDLE_SIZE * 2.0)
            .cursor(CursorStyle::NeResize)
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Start Resize
            drag_resize_window(ResizeDirection::NorthEast);
        }),
    )).style(|s| {
            let colors = get_theme_colors();
            s.size_full().background(colors.bg2)
    });

    let id = view.id();
    //let window_id = 0 as WindowId;//floem::window::WindowContext;
    
    view.on_event_stop(EventListener::KeyUp, move |e| {
        if let Event::Key(KeyboardEvent {
            state: KeyState::Up,
            code,
            key,
            ..
        }) = e 
        {
            if *key == Key::Named(NamedKey::F11) {
                id.inspect();
            }
        }
    })
    .window_title(|| String::from(APPNAME))
}