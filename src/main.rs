mod app;
mod recorder;
mod settings;
mod store;
mod transcriber;

use chrono::{Local, Months};
use gpui_kit::{component::TitleBar, *};

use crate::{app::TranscriptsView, settings::Settings, store::Store};

/// Tokio runtime for rig/reqwest, which GPUI's executor does not provide.
pub struct TokioRuntime(pub tokio::runtime::Runtime);

impl Global for TokioRuntime {}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to start tokio runtime");

    let mut store = Store::load(Store::default_path()).unwrap_or_else(|e| {
        eprintln!("could not load transcripts, starting empty: {e:#}");
        Store::load(std::env::temp_dir().join("rs-speech-to-text-fallback.json"))
            .expect("fallback store")
    });

    let cutoff = Local::now()
        .checked_sub_months(Months::new(3))
        .unwrap_or_else(Local::now);
    if store.prune_older_than(cutoff) > 0
        && let Err(e) = store.save()
    {
        eprintln!("could not save pruned transcripts: {e:#}");
    }

    let settings_path = Settings::default_path();
    let settings = Settings::load(&settings_path).unwrap_or_else(|e| {
        eprintln!("could not load settings, using defaults: {e:#}");
        Settings::default()
    });

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_global(TokioRuntime(runtime));

            let bounds = Bounds::centered(None, size(px(520.), px(720.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Matches StartupWMClass in packaging/rs-speech-to-text.desktop.
                app_id: Some("rs-speech-to-text".into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("Speech to Text".into()),
                    ..TitleBar::title_bar_options()
                }),
                // On Wayland the window is client-decorated, so TitleBar must own dragging.
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |_, cx| {
                cx.new(|_| TranscriptsView::new(store, settings, settings_path))
            })
            .expect("failed to open window");
            cx.activate(true);
        });
}
