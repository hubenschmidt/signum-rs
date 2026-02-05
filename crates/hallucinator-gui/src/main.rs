//! hallucinator-gui: DAW GUI application

mod app;
pub mod clipboard;
mod panels;

use app::HallucinatorApp;
use eframe::NativeOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(target_os = "linux")]
mod x11_init {
    use std::ffi::c_int;
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[repr(C)]
    pub struct Display {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct XErrorEvent {
        pub type_: c_int,
        pub display: *mut Display,
        pub resourceid: u64,
        pub serial: u64,
        pub error_code: u8,
        pub request_code: u8,
        pub minor_code: u8,
    }

    type XErrorHandler = extern "C" fn(*mut Display, *mut XErrorEvent) -> c_int;

    #[link(name = "X11")]
    unsafe extern "C" {
        pub fn XInitThreads() -> c_int;
        pub fn XSetErrorHandler(handler: XErrorHandler) -> *mut c_void;
    }

    static X_ERROR_LOGGED: AtomicBool = AtomicBool::new(false);

    extern "C" fn x11_error_handler(_display: *mut Display, event: *mut XErrorEvent) -> c_int {
        if X_ERROR_LOGGED.swap(true, Ordering::Relaxed) {
            return 0; // Already logged one error, skip
        }
        let event = unsafe { &*event };
        eprintln!(
            "[X11 Error] code={} request={} resource=0x{:x} (non-fatal, continuing)",
            event.error_code, event.request_code, event.resourceid
        );
        X_ERROR_LOGGED.store(false, Ordering::Relaxed);
        0
    }

    pub fn init() {
        unsafe {
            let result = XInitThreads();
            if result == 0 {
                eprintln!("Warning: XInitThreads() failed");
            }
            XSetErrorHandler(x11_error_handler);
        }
    }
}

fn main() -> eframe::Result<()> {
    // Initialize X11 threading BEFORE any X11 operations (critical for VST3 plugins)
    #[cfg(target_os = "linux")]
    x11_init::init();

    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("hallucinator=debug".parse().unwrap())
            .add_directive("wgpu=warn".parse().unwrap())
            .add_directive("eframe=warn".parse().unwrap()))
        .init();

    tracing::info!("Starting Hallucinator");

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Hallucinator",
        options,
        Box::new(|cc| Ok(Box::new(HallucinatorApp::new(cc)))),
    )
}
