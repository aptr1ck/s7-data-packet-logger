use floem::{
    prelude::*,
    event::{Event, EventListener},
    keyboard::{Key, NamedKey},
    kurbo::Size,
    views::{button, container, h_stack, label, v_stack,},
    style::{AlignContent},
    window::{new_window, WindowConfig},
    IntoView
};

/*fn list_item<V: IntoView + 'static>(name: String, view_fn: impl Fn() -> V) -> impl IntoView {
    h_stack((
        label(move || name.clone()).style(|s| s),
        container(view_fn()).style(|s| s.width_full().justify_content(AlignContent::End)),
    ))
    .style(|s| s.width(200))
}*/

pub fn app_view() -> impl IntoView {
    let view = h_stack((
        button("Test").action(|| {
            println!("Button clicked!");
        }),
    ));

    let id = view.id();
    view.on_event_stop(EventListener::KeyUp, move |e| {
        if let Event::KeyUp(e) = e {
            if e.key.logical_key == Key::Named(NamedKey::F11) {
                id.inspect();
            }
        }
    })
    .window_title(|| String::from("Layout examples"))
}