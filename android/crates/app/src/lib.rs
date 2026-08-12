//! PianoTuner: an Android piano tuner built on `tuner-core`.
//!
//! On Android the library is loaded as a `cdylib` by `NativeActivity` and
//! `android_main` is the entry point. The same modules build on the host, where
//! [`run_desktop`] opens a normal window, which keeps the UI and state logic
//! testable without a device.

pub mod audio;
pub mod state;
pub mod storage;
pub mod ui;

pub use state::{AppState, Screen};
pub use ui::PianoTunerApp;

/// Window / app title.
pub const APP_NAME: &str = "PianoTuner";

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    use android_activity::AndroidApp;
    use winit::platform::android::EventLoopBuilderExtAndroid as _;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("PianoTuner"),
    );

    fn library_dir(app: &AndroidApp) -> std::path::PathBuf {
        app.external_data_path()
            .or_else(|| app.internal_data_path())
            .unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"))
            .join("tunings")
    }

    storage::set_library_dir(library_dir(&app));

    let options = eframe::NativeOptions {
        // eframe 0.27 takes the `AndroidApp` handle through the winit event
        // loop builder rather than through `NativeOptions` directly.
        event_loop_builder: Some(Box::new(move |builder| {
            builder.with_android_app(app);
        })),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Box::new(PianoTunerApp::new(cc))),
    ) {
        log::error!("eframe error: {e}");
    }
}

/// Run the tuner in a desktop window. Useful for development on the host.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Box::new(PianoTunerApp::new(cc))),
    )
}
