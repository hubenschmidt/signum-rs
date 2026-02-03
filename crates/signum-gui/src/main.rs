//! signum-gui: DAW GUI application

mod app;
mod panels;

use app::SignumApp;
use eframe::NativeOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("signum=debug".parse().unwrap())
            .add_directive("wgpu=warn".parse().unwrap())
            .add_directive("eframe=warn".parse().unwrap()))
        .init();

    tracing::info!("Starting Signum DAW");

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Signum DAW",
        options,
        Box::new(|cc| Ok(Box::new(SignumApp::new(cc)))),
    )
}
