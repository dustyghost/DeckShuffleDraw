mod app;
mod image_loader;
mod settings;

use app::CardApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 1200.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Deck Shuffle Draw",
        options,
        Box::new(|cc| Ok(Box::new(CardApp::new(cc)))),
    )
}
