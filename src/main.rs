// Fulgur ⚡ — un IDE nativo en Rust, ligero y rápido.
//
// Módulos:
//  - app:        la ventana principal, menús y layout de paneles.
//  - explorador: árbol de archivos real.
//  - editor:     pestañas, resaltado de sintaxis y guardado.
//  - terminal:   terminal conectada a PowerShell.
//  - mercado:    mercado de extensiones descentralizado (SHA-256 verificado).
//  - ia:         asistente conectable a Ollama (local) o Anthropic (Claude).
//
// Esta línea evita que se abra una ventana negra de consola al lanzar la app en Windows.
#![windows_subsystem = "windows"]

mod actualizador;
mod app;
mod diagnostico;
mod editor;
mod explorador;
mod git;
mod ia;
mod lsp;
mod mercado;
mod tema;
mod terminal;

use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([760.0, 480.0])
        .with_title("Fulgur ⚡");
    // Icono de la ventana (y de la barra de tareas): el rayo de neón.
    if let Ok(icono) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo-256.png")) {
        viewport = viewport.with_icon(std::sync::Arc::new(icono));
    }

    let opciones = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Fulgur",
        opciones,
        Box::new(|_cc| Ok(Box::new(app::Fulgur::default()))),
    )
}
