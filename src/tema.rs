// Temas integrados de Fulgur (no dependen de extensiones):
//  - Normal: el gris estilo VS Code Dark+ de siempre.
//  - Oscuro: negro moderno con acentos neón.
//  - Claro:  el blanco que nadie usa más que los alocados.
//
// También vive aquí el botón de cerrar (✕) dibujado a mano, porque la fuente
// por defecto no trae ese glifo y salía un cuadrito.

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Tema {
    Normal,
    Oscuro,
    Claro,
}

pub struct Paleta {
    pub fondo_editor: Color32,
    pub barra_actividad: Color32,
    pub barra_lateral: Color32,
    pub barra_menu: Color32,
    pub barra_estado: Color32,
    pub texto_estado: Color32,
    pub tab_inactiva: Color32,
    pub acento: Color32,
    pub texto: Color32,
    pub texto_tenue: Color32,
    pub texto_fuerte: Color32,
    pub borde: Color32,
    pub seleccion: Color32,
    pub es_oscuro: bool,
}

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// El gris de VS Code Dark+ (el actual).
static NORMAL: Paleta = Paleta {
    fondo_editor: c(0x1e, 0x1e, 0x1e),
    barra_actividad: c(0x33, 0x33, 0x33),
    barra_lateral: c(0x25, 0x25, 0x26),
    barra_menu: c(0x3c, 0x3c, 0x3c),
    barra_estado: c(0x00, 0x7a, 0xcc),
    texto_estado: Color32::WHITE,
    tab_inactiva: c(0x2d, 0x2d, 0x2d),
    acento: c(0x00, 0x7a, 0xcc),
    texto: c(0xcc, 0xcc, 0xcc),
    texto_tenue: c(0x8c, 0x8c, 0x8c),
    texto_fuerte: Color32::WHITE,
    borde: c(0x2b, 0x2b, 0x2b),
    seleccion: c(0x09, 0x47, 0x71),
    es_oscuro: true,
};

/// Negro moderno con acentos neón.
static OSCURO: Paleta = Paleta {
    fondo_editor: c(0x0b, 0x0b, 0x0f),
    barra_actividad: c(0x08, 0x08, 0x0c),
    barra_lateral: c(0x0f, 0x0f, 0x16),
    barra_menu: c(0x12, 0x12, 0x1c),
    barra_estado: c(0x0b, 0x0b, 0x12),
    texto_estado: c(0x00, 0xe5, 0xff),
    tab_inactiva: c(0x11, 0x11, 0x1a),
    acento: c(0x00, 0xe5, 0xff),
    texto: c(0xc8, 0xcc, 0xdd),
    texto_tenue: c(0x5c, 0x62, 0x7c),
    texto_fuerte: c(0xea, 0xf6, 0xff),
    borde: c(0x1e, 0x1e, 0x2e),
    seleccion: c(0x00, 0x3a, 0x4e),
    es_oscuro: true,
};

/// Blanco estilo VS Code Light (la barra de actividad se queda oscura, como en VSC).
static CLARO: Paleta = Paleta {
    fondo_editor: Color32::WHITE,
    barra_actividad: c(0x2c, 0x2c, 0x2c),
    barra_lateral: c(0xf3, 0xf3, 0xf3),
    barra_menu: c(0xe4, 0xe4, 0xe4),
    barra_estado: c(0x00, 0x7a, 0xcc),
    texto_estado: Color32::WHITE,
    tab_inactiva: c(0xec, 0xec, 0xec),
    acento: c(0x00, 0x7a, 0xcc),
    texto: c(0x33, 0x33, 0x33),
    texto_tenue: c(0x71, 0x71, 0x71),
    texto_fuerte: Color32::BLACK,
    borde: c(0xd4, 0xd4, 0xd4),
    seleccion: c(0xad, 0xd6, 0xff),
    es_oscuro: false,
};

/// Tema activo (0=Normal, 1=Oscuro, 2=Claro).
static ACTUAL: AtomicU8 = AtomicU8::new(0);

/// La paleta del tema activo. Los módulos pintan siempre con `tema::p()`.
pub fn p() -> &'static Paleta {
    match ACTUAL.load(Ordering::Relaxed) {
        1 => &OSCURO,
        2 => &CLARO,
        _ => &NORMAL,
    }
}

pub fn actual() -> Tema {
    match ACTUAL.load(Ordering::Relaxed) {
        1 => Tema::Oscuro,
        2 => Tema::Claro,
        _ => Tema::Normal,
    }
}

fn ruta_config() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fulgur")
        .join("tema.json")
}

pub fn cargar_guardado() -> Tema {
    std::fs::read_to_string(ruta_config())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Tema::Normal)
}

/// Aplica un tema a toda la app y lo persiste para el próximo arranque.
pub fn aplicar(ctx: &egui::Context, tema: Tema) {
    ACTUAL.store(
        match tema {
            Tema::Normal => 0,
            Tema::Oscuro => 1,
            Tema::Claro => 2,
        },
        Ordering::Relaxed,
    );
    let p = p();
    let mut v = if p.es_oscuro {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.barra_lateral;
    v.window_fill = p.barra_lateral;
    v.extreme_bg_color = p.fondo_editor;
    v.selection.bg_fill = p.seleccion;
    v.hyperlink_color = p.acento;
    v.widgets.noninteractive.fg_stroke.color = p.texto;
    v.widgets.inactive.fg_stroke.color = p.texto;
    v.widgets.hovered.fg_stroke.color = p.texto_fuerte;
    v.widgets.active.fg_stroke.color = p.texto_fuerte;
    v.widgets.open.fg_stroke.color = p.texto_fuerte;
    v.widgets.noninteractive.bg_stroke.color = p.borde;
    ctx.set_visuals(v);

    let _ = std::fs::create_dir_all(ruta_config().parent().unwrap());
    if let Ok(texto) = serde_json::to_string(&tema) {
        let _ = std::fs::write(ruta_config(), texto);
    }
}

/// Botón de cerrar (✕) dibujado a mano — sale bien en cualquier fuente.
/// Si `modificado` es true, muestra el punto ● hasta que el ratón pase por encima.
pub fn boton_cerrar(ui: &mut egui::Ui, tam: f32, color: Color32, modificado: bool) -> egui::Response {
    let (rect, respuesta) = ui.allocate_exact_size(egui::vec2(tam, tam), egui::Sense::click());
    let centro = rect.center();
    let sobre = respuesta.hovered();

    if sobre {
        ui.painter().rect_filled(
            rect.expand(1.0),
            egui::CornerRadius::same(3),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }

    if modificado && !sobre {
        ui.painter().circle_filled(centro, tam * 0.20, color);
    } else {
        let r = tam * 0.26;
        let trazo = egui::Stroke::new(1.3, if sobre { p().texto_fuerte } else { color });
        ui.painter().line_segment(
            [centro + egui::vec2(-r, -r), centro + egui::vec2(r, r)],
            trazo,
        );
        ui.painter().line_segment(
            [centro + egui::vec2(-r, r), centro + egui::vec2(r, -r)],
            trazo,
        );
    }
    respuesta
}
