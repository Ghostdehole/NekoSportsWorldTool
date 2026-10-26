//! Standalone local route planner. Does not initialize the campus service client.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(feature = "gui")]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 820.0])
            .with_min_inner_size([360.0, 540.0])
            .with_title("Neko 路线工作台"),
        ..Default::default()
    };
    nekosportsworldtool::run_route_planner_gui(options)
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("路线工作台需要 GUI：cargo run --bin neko-route-workbench");
    std::process::exit(1);
}
