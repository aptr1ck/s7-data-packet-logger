use floem::action::set_window_scale;
use floem::reactive::{use_context, provide_context};
use floem::views::Decorators;
use floem::{
    event::{Event, EventListener},
    kurbo::{Point, Size},
    reactive::{UpdaterEffect, RwSignal, SignalGet, SignalUpdate, SignalWith},
    window::WindowConfig,
    Application, IntoView,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::constants::*;

// Theme name signal type for providing context
#[derive(Clone, Copy)]
pub struct ThemeNameSig(pub RwSignal<String>);


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppCommand {
    Quit,
    Minimize,
    Maximize,
    //NewFile,
    //OpenFile,
    //OpenFileFolder,
    //SaveFile,
    //SaveFileAs,
}

type CommandHandler = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct CommandRegistry {
    handlers: HashMap<AppCommand, CommandHandler>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, command: AppCommand, handler: CommandHandler) {
        self.handlers.insert(command, handler);
    }
    
    pub fn execute(&self, command: AppCommand) {
        if let Some(handler) = self.handlers.get(&command) {
            handler();
        }
    }
}

fn default_syntect_theme() -> String {
    "Default".to_string()
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct AppConfig {
    pub position: Point,
    pub size: Size,
    #[serde(default = "default_syntect_theme")]
    pub syntect_theme_name: String,
    pub window_scale: f64,
    pub is_maximised: bool,
    pub sidebar_width: f64,
}

impl std::default::Default for AppConfig {
    fn default() -> Self {
        Self {
            position: Point { x: 500.0, y: 500.0 },
            size: Size {
                width: 350.0,
                height: 650.0,
            },
            /*app_theme: AppThemeState {
                system: floem::window::Theme::Dark,
                theme: AppTheme::FollowSystem,
            },*/
            syntect_theme_name: default_syntect_theme(),
            window_scale: 1.,
            is_maximised: false,
            sidebar_width: 300.0,
        }
    }
}

pub fn launch_with_track<V: IntoView + 'static>(app_view: impl FnOnce() -> V + 'static) {
    let config: AppConfig = confy::load(APPNAME, "floem-defaults").unwrap_or_default();

    let app = Application::new();

    // modifying this will rewrite app config to disk
    let app_config = RwSignal::new(config);
    provide_context(app_config);

    // Theme signal: initialize from config instead of a hard-coded string
    let theme_name = RwSignal::new(app_config.with(|c| c.syntect_theme_name.clone()));
    provide_context(ThemeNameSig(theme_name));

    // Whenever theme_name changes, mirror it back into app_config
    let app_config_for_theme = app_config;
    let ThemeNameSig(theme_sig) = use_context::<ThemeNameSig>().expect("ThemeNameSig missing");
    UpdaterEffect::new(
        move || theme_sig.get(),
        move |theme_sig| app_config_for_theme.update(|c| c.syntect_theme_name = theme_sig.clone()),
    );

    // Register Commands
    let mut registry: CommandRegistry = CommandRegistry::new();
    registry.register(AppCommand::Quit, Arc::new(|| {
        floem::quit_app();
    }));
    registry.register(AppCommand::Minimize, Arc::new(|| {
        floem::action::minimize_window();
    }));
    registry.register(AppCommand::Maximize, {
        Arc::new(move || {
            floem::action::toggle_window_maximized();
        })
    });

    //let registry_local = registry.clone();
    provide_context(RwSignal::new(registry));

    // todo: debounce this
    UpdaterEffect::new(
        move || app_config.get(),
        |config| {
            let _ = confy::store(APPNAME, "floem-defaults", config);
        },
    );

    let window_config = WindowConfig::default()
        .size(app_config.with(|ac| ac.size))
        .min_size(Size::new(800.0, 300.0))
        .position(app_config.with(|ac| ac.position))
        /*.undecorated(true)*/
        .show_titlebar(false)
        .resizable(true)
        .undecorated_shadow(true);

    app.window(
        move |_| {
            set_window_scale(app_config.with(|c| c.window_scale));

            // If config says the window was maximised last time, restore it on launch.
            if app_config.with(|c| c.is_maximised) {
                // toggle to maximize the newly created window
                floem::action::toggle_window_maximized();
            }

            app_view()
                .on_event_stop(EventListener::WindowMoved, move |event| {
                    if let Event::WindowMoved(position) = event {
                        // only store position when not maximised
                        app_config.update(|val| {
                            if !val.is_maximised {
                                val.position = *position;
                            }
                        })
                    }
                })
                .on_event_stop(EventListener::WindowResized, move |event| {
                    if let Event::WindowResized(size) = event {
                        // only store size when not maximised
                        app_config.update(|val| {
                            if !val.is_maximised {
                                val.size = *size;
                            }
                        })
                    }
                })
                // update config when user/OS maximises/restores the window
                .on_event_stop(EventListener::WindowMaximizeChanged, {
                    let app_config = app_config.clone();
                    move |event| {
                        if let Event::WindowMaximizeChanged(is_maximised) = event {
                            app_config.update(|c| c.is_maximised = *is_maximised);
                        }
                    }
                })
        },
        Some(window_config),
    )
    .run();
}
