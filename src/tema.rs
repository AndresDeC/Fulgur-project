// Temas integrados de Fulgur (no dependen de extensiones).
//
// Los temas viven en la tabla `TEMAS`: añadir uno nuevo es añadir una `Paleta`
// más al array, nada más. El tema activo se guarda por su `id` en
// ~/.fulgur/tema.json y se recupera al arrancar.
//
// Aquí vive también el botón de cerrar (✕) dibujado a mano, porque la fuente
// por defecto no trae ese glifo y salía un cuadrito.

use eframe::egui::{self, Color32};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Paleta {
    /// Identificador estable para persistir (no cambiar entre versiones).
    pub id: &'static str,
    /// Nombre visible en el menú Ver → Tema.
    pub nombre: &'static str,
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

/// Todos los temas integrados. El primero es el de por defecto.
static TEMAS: &[Paleta] = &[
    // Gris neutro estilo VS Code Dark+. El clásico de siempre.
    Paleta {
        id: "grafito",
        nombre: "Grafito (gris)",
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
    },
    // Negro profundo con acentos de cian neón. El favorito del usuario.
    Paleta {
        id: "neon-black",
        nombre: "Neon Black",
        fondo_editor: c(0x0a, 0x0a, 0x0f),
        barra_actividad: c(0x06, 0x06, 0x09),
        barra_lateral: c(0x0c, 0x0c, 0x12),
        barra_menu: c(0x10, 0x10, 0x18),
        barra_estado: c(0x0a, 0x0a, 0x12),
        texto_estado: c(0x00, 0xe5, 0xff),
        tab_inactiva: c(0x10, 0x10, 0x19),
        acento: c(0x00, 0xe5, 0xff),
        texto: c(0xc6, 0xcb, 0xde),
        texto_tenue: c(0x56, 0x5c, 0x75),
        texto_fuerte: c(0xea, 0xf6, 0xff),
        borde: c(0x1b, 0x1b, 0x29),
        seleccion: c(0x07, 0x3a, 0x4e),
        es_oscuro: true,
    },
    // Índigo profundo, suave para la vista. Inspirado en Tokyo Night.
    Paleta {
        id: "tokyo-night",
        nombre: "Tokyo Night",
        fondo_editor: c(0x1a, 0x1b, 0x26),
        barra_actividad: c(0x14, 0x15, 0x1e),
        barra_lateral: c(0x16, 0x16, 0x20),
        barra_menu: c(0x1f, 0x20, 0x30),
        barra_estado: c(0x16, 0x16, 0x1e),
        texto_estado: c(0x7a, 0xa2, 0xf7),
        tab_inactiva: c(0x1e, 0x20, 0x30),
        acento: c(0x7a, 0xa2, 0xf7),
        texto: c(0xa9, 0xb1, 0xd6),
        texto_tenue: c(0x56, 0x5f, 0x89),
        texto_fuerte: c(0xc0, 0xca, 0xf5),
        borde: c(0x2a, 0x2e, 0x42),
        seleccion: c(0x28, 0x34, 0x57),
        es_oscuro: true,
    },
    // Gris cálido azulado, clásico y descansado. Inspirado en One Dark.
    Paleta {
        id: "one-dark",
        nombre: "One Dark",
        fondo_editor: c(0x28, 0x2c, 0x34),
        barra_actividad: c(0x1e, 0x22, 0x28),
        barra_lateral: c(0x21, 0x25, 0x2b),
        barra_menu: c(0x2c, 0x31, 0x3a),
        barra_estado: c(0x21, 0x25, 0x2b),
        texto_estado: c(0x61, 0xaf, 0xef),
        tab_inactiva: c(0x2b, 0x30, 0x38),
        acento: c(0x61, 0xaf, 0xef),
        texto: c(0xab, 0xb2, 0xbf),
        texto_tenue: c(0x63, 0x6d, 0x83),
        texto_fuerte: Color32::WHITE,
        borde: c(0x3a, 0x3f, 0x4b),
        seleccion: c(0x3e, 0x44, 0x51),
        es_oscuro: true,
    },
    // Blanco limpio y profesional. Inspirado en GitHub Light.
    Paleta {
        id: "papel",
        nombre: "Papel (claro)",
        fondo_editor: Color32::WHITE,
        barra_actividad: c(0x24, 0x29, 0x2f),
        barra_lateral: c(0xf6, 0xf8, 0xfa),
        barra_menu: c(0xef, 0xf1, 0xf3),
        barra_estado: c(0x09, 0x69, 0xda),
        texto_estado: Color32::WHITE,
        tab_inactiva: c(0xea, 0xee, 0xf2),
        acento: c(0x09, 0x69, 0xda),
        texto: c(0x1f, 0x23, 0x28),
        texto_tenue: c(0x65, 0x6d, 0x76),
        texto_fuerte: Color32::BLACK,
        borde: c(0xd0, 0xd7, 0xde),
        seleccion: c(0xb6, 0xda, 0xff),
        es_oscuro: false,
    },
    // Claro cálido de tonos sepia. Inspirado en Solarized Light.
    Paleta {
        id: "solar",
        nombre: "Solar (sepia)",
        fondo_editor: c(0xfd, 0xf6, 0xe3),
        barra_actividad: c(0x07, 0x36, 0x42),
        barra_lateral: c(0xee, 0xe8, 0xd5),
        barra_menu: c(0xe6, 0xdf, 0xc8),
        barra_estado: c(0x26, 0x8b, 0xd2),
        texto_estado: c(0xfd, 0xf6, 0xe3),
        tab_inactiva: c(0xe9, 0xe2, 0xcc),
        acento: c(0x26, 0x8b, 0xd2),
        texto: c(0x58, 0x6e, 0x75),
        texto_tenue: c(0x93, 0xa1, 0xa1),
        texto_fuerte: c(0x07, 0x36, 0x42),
        borde: c(0xdd, 0xd6, 0xc1),
        seleccion: c(0xd7, 0xcf, 0xb8),
        es_oscuro: false,
    },
];

/// Índice del tema activo dentro de `TEMAS`.
static ACTUAL: AtomicUsize = AtomicUsize::new(0);

/// Todos los temas disponibles (para construir el menú).
pub fn temas() -> &'static [Paleta] {
    TEMAS
}

/// La paleta del tema activo. Los módulos pintan siempre con `tema::p()`.
pub fn p() -> &'static Paleta {
    let i = ACTUAL.load(Ordering::Relaxed).min(TEMAS.len() - 1);
    &TEMAS[i]
}

/// Índice del tema activo (para resaltar el elegido en el menú).
pub fn indice_actual() -> usize {
    ACTUAL.load(Ordering::Relaxed).min(TEMAS.len() - 1)
}

fn ruta_config() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fulgur")
        .join("tema.json")
}

/// Lee el tema guardado y devuelve su índice (0 si no hay o no se reconoce).
pub fn cargar_guardado() -> usize {
    let Ok(id) = std::fs::read_to_string(ruta_config()) else {
        return 0;
    };
    let id = id.trim();
    TEMAS
        .iter()
        .position(|t| t.id == id)
        .unwrap_or(0)
}

/// Aplica un tema por su índice a toda la app y lo persiste para el próximo arranque.
pub fn aplicar_indice(ctx: &egui::Context, indice: usize) {
    let indice = indice.min(TEMAS.len() - 1);
    ACTUAL.store(indice, Ordering::Relaxed);
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
    let _ = std::fs::write(ruta_config(), p.id);
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
