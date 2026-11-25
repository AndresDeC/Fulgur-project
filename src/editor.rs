// Editor de código: pestañas estilo VS Code, resaltado de sintaxis,
// selector de lenguaje y guardado.

use crate::tema;
use eframe::egui;
use std::path::PathBuf;

/// Lenguajes disponibles en el selector (etiqueta visible, extensión para syntect).
pub const LENGUAJES: &[(&str, &str)] = &[
    ("Texto plano", "txt"),
    ("Rust", "rs"),
    ("Python", "py"),
    ("JavaScript", "js"),
    ("TypeScript", "ts"),
    ("HTML", "html"),
    ("CSS", "css"),
    ("JSON", "json"),
    ("Markdown", "md"),
    ("TOML", "toml"),
    ("YAML", "yaml"),
    ("XML", "xml"),
    ("C", "c"),
    ("C++", "cpp"),
    ("C#", "cs"),
    ("Java", "java"),
    ("Go", "go"),
    ("PHP", "php"),
    ("Ruby", "rb"),
    ("Lua", "lua"),
    ("Shell", "sh"),
    ("SQL", "sql"),
];

/// Etiqueta legible de un código de lenguaje ("rs" → "Rust").
pub fn etiqueta_lenguaje(codigo: &str) -> String {
    LENGUAJES
        .iter()
        .find(|(_, c)| *c == codigo)
        .map(|(etiqueta, _)| (*etiqueta).to_owned())
        .unwrap_or_else(|| codigo.to_uppercase())
}

pub struct Pestana {
    pub ruta: Option<PathBuf>,
    pub contenido: String,
    pub modificado: bool,
    /// Lenguaje elegido a mano (para archivos sin título o para forzarlo).
    pub lenguaje_manual: Option<String>,
    /// Diagnósticos del contenido actual (de tree-sitter o del LSP).
    pub diagnosticos: Vec<crate::diagnostico::Diagnostico>,
    /// true cuando el contenido o el lenguaje cambió y hay que reanalizar.
    pub diag_sucio: bool,
    /// true si un servidor LSP gestiona este archivo; entonces el editor no
    /// usa tree-sitter y los diagnósticos los pone la app desde el LSP.
    pub usa_lsp: bool,
}

impl Pestana {
    pub fn nombre(&self) -> String {
        self.ruta
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sin título".to_owned())
    }

    /// Lenguaje para el resaltado: el elegido a mano gana a la extensión.
    pub fn lenguaje(&self) -> String {
        if let Some(l) = &self.lenguaje_manual {
            return l.clone();
        }
        self.ruta
            .as_ref()
            .and_then(|r| r.extension())
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "txt".to_owned())
    }
}

#[derive(Default)]
pub struct Editor {
    pub pestanas: Vec<Pestana>,
    pub activa: usize,
    /// Contador para dar id único y estable a cada pestaña nueva.
    siguiente_id: usize,
    ids: Vec<usize>,
}

impl Editor {
    fn empujar(&mut self, pestana: Pestana) {
        self.pestanas.push(pestana);
        self.ids.push(self.siguiente_id);
        self.siguiente_id += 1;
        self.activa = self.pestanas.len() - 1;
    }

    /// Id del TextEdit de la pestaña activa — lo usa el menú Edición para
    /// devolverle el foco antes de inyectar deshacer/copiar/pegar.
    pub fn id_editor_activo(&self) -> Option<egui::Id> {
        self.ids
            .get(self.activa)
            .map(|n| egui::Id::new(("texto_editor", *n)))
    }

    pub fn abrir(&mut self, ruta: PathBuf) -> Result<(), String> {
        if let Some(i) = self
            .pestanas
            .iter()
            .position(|p| p.ruta.as_deref() == Some(ruta.as_path()))
        {
            self.activa = i;
            return Ok(());
        }
        let contenido = std::fs::read_to_string(&ruta)
            .map_err(|e| format!("No se pudo abrir {}: {e}", ruta.display()))?;
        self.empujar(Pestana {
            ruta: Some(ruta),
            contenido,
            modificado: false,
            lenguaje_manual: None,
            diagnosticos: Vec::new(),
            diag_sucio: true,
            usa_lsp: false,
        });
        Ok(())
    }

    pub fn nueva(&mut self) {
        self.empujar(Pestana {
            ruta: None,
            contenido: String::new(),
            modificado: false,
            lenguaje_manual: None,
            diagnosticos: Vec::new(),
            diag_sucio: true,
            usa_lsp: false,
        });
    }

    pub fn guardar_activa(&mut self) -> Result<Option<PathBuf>, String> {
        let Some(pestana) = self.pestanas.get_mut(self.activa) else {
            return Ok(None);
        };
        let ruta = match &pestana.ruta {
            Some(r) => r.clone(),
            None => match rfd::FileDialog::new().save_file() {
                Some(r) => {
                    pestana.ruta = Some(r.clone());
                    r
                }
                None => return Ok(None),
            },
        };
        std::fs::write(&ruta, &pestana.contenido)
            .map_err(|e| format!("No se pudo guardar {}: {e}", ruta.display()))?;
        pestana.modificado = false;
        Ok(Some(ruta))
    }

    /// "Guardar como…": siempre pregunta la ruta.
    pub fn guardar_como_activa(&mut self) -> Result<Option<PathBuf>, String> {
        let Some(pestana) = self.pestanas.get_mut(self.activa) else {
            return Ok(None);
        };
        let Some(ruta) = rfd::FileDialog::new().save_file() else {
            return Ok(None);
        };
        std::fs::write(&ruta, &pestana.contenido)
            .map_err(|e| format!("No se pudo guardar {}: {e}", ruta.display()))?;
        pestana.ruta = Some(ruta.clone());
        pestana.modificado = false;
        Ok(Some(ruta))
    }

    pub fn cerrar_activa(&mut self) {
        if !self.pestanas.is_empty() {
            self.cerrar(self.activa);
        }
    }

    fn cerrar(&mut self, i: usize) {
        self.pestanas.remove(i);
        self.ids.remove(i);
        if self.activa >= i && self.activa > 0 {
            self.activa -= 1;
        }
    }

    pub fn contexto_activo(&self) -> Option<(String, &str)> {
        self.pestanas
            .get(self.activa)
            .map(|p| (p.nombre(), p.contenido.as_str()))
    }

    pub fn pestana_activa_mut(&mut self) -> Option<&mut Pestana> {
        self.pestanas.get_mut(self.activa)
    }

    pub fn pestana_activa(&self) -> Option<&Pestana> {
        self.pestanas.get(self.activa)
    }

    /// Para la barra de estado: (hay análisis, nº de errores, nº de avisos).
    /// Hay análisis si el lenguaje lo soporta tree-sitter o lo gestiona el LSP.
    pub fn analisis_activo(&self) -> Option<(bool, usize, usize)> {
        use crate::diagnostico::Severidad;
        self.pestana_activa().map(|p| {
            let analizado = crate::diagnostico::soportado(&p.lenguaje()) || p.usa_lsp;
            let errores = p
                .diagnosticos
                .iter()
                .filter(|d| d.severidad == Severidad::Error)
                .count();
            (analizado, errores, p.diagnosticos.len() - errores)
        })
    }

    /// true si el archivo activo lo gestiona un servidor LSP.
    pub fn activa_usa_lsp(&self) -> bool {
        self.pestana_activa().is_some_and(|p| p.usa_lsp)
    }

    /// Los errores de sintaxis de la pestaña activa (para el tooltip de detalle).
    pub fn diagnosticos_activos(&self) -> &[crate::diagnostico::Diagnostico] {
        self.pestana_activa()
            .map(|p| p.diagnosticos.as_slice())
            .unwrap_or(&[])
    }

    /// Cierra las pestañas cuyo archivo (o carpeta contenedora) fue eliminado.
    pub fn cerrar_por_ruta(&mut self, eliminada: &std::path::Path) {
        let conservar: Vec<bool> = self
            .pestanas
            .iter()
            .map(|p| {
                p.ruta
                    .as_ref()
                    .is_none_or(|r| r != eliminada && !r.starts_with(eliminada))
            })
            .collect();
        let mut i = 0;
        self.pestanas.retain(|_| {
            let k = conservar[i];
            i += 1;
            k
        });
        let mut i = 0;
        self.ids.retain(|_| {
            let k = conservar[i];
            i += 1;
            k
        });
        self.activa = self.activa.min(self.pestanas.len().saturating_sub(1));
    }

    /// Actualiza rutas tras un renombrado (archivo o carpeta contenedora).
    pub fn renombrar_ruta(&mut self, vieja: &std::path::Path, nueva: &std::path::Path) {
        for p in &mut self.pestanas {
            if let Some(r) = &p.ruta {
                if r == vieja {
                    p.ruta = Some(nueva.to_path_buf());
                } else if let Ok(resto) = r.strip_prefix(vieja) {
                    p.ruta = Some(nueva.join(resto));
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.pestanas.is_empty() {
            self.pantalla_bienvenida(ui);
            return;
        }
        self.activa = self.activa.min(self.pestanas.len() - 1);

        self.barra_pestanas(ui);

        // Aviso tipo VSC en archivos sin título: elegir lenguaje dentro del editor.
        if self.pestanas[self.activa].ruta.is_none() {
            self.aviso_lenguaje(ui);
        }

        // --- El editor con resaltado de sintaxis ---
        let lenguaje = self.pestanas[self.activa].lenguaje();
        let id_texto = self.id_editor_activo().expect("hay pestaña activa");
        let tema_codigo =
            egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
        let pestana = &mut self.pestanas[self.activa];

        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, ancho: f32| {
            let mut trabajo = egui_extras::syntax_highlighting::highlight(
                ui.ctx(),
                ui.style(),
                &tema_codigo,
                buf.as_str(),
                &lenguaje,
            );
            trabajo.wrap.max_width = ancho;
            ui.ctx().fonts_mut(|f| f.layout_job(trabajo))
        };

        egui::ScrollArea::both()
            .id_salt("editor")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let salida = egui::TextEdit::multiline(&mut pestana.contenido)
                    .id(id_texto)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(40)
                    .layouter(&mut layouter)
                    .show(ui);

                if salida.response.changed() {
                    pestana.modificado = true;
                    pestana.diag_sucio = true;
                }
                // Reanalizar con tree-sitter solo cuando cambió el contenido y
                // ningún servidor LSP gestiona el archivo (si lo hay, sus
                // diagnósticos semánticos los pone la app y son mejores).
                if !pestana.usa_lsp && pestana.diag_sucio {
                    pestana.diagnosticos =
                        crate::diagnostico::analizar(&lenguaje, &pestana.contenido);
                    pestana.diag_sucio = false;
                }
                // Subrayar en rojo los errores de sintaxis encontrados.
                dibujar_subrayados(
                    ui.painter(),
                    &salida.galley,
                    salida.galley_pos,
                    &pestana.diagnosticos,
                );
            });
    }

    /// Franja "Selecciona un lenguaje" para archivos sin título, como en VSC.
    fn aviso_lenguaje(&mut self, ui: &mut egui::Ui) {
        let actual = self.pestanas[self.activa].lenguaje();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.weak("Archivo sin título —");
            ui.menu_button(
                egui::RichText::new(format!("Seleccionar lenguaje: {} ▾", etiqueta_lenguaje(&actual)))
                    .color(tema::p().acento)
                    .size(12.5),
                |ui| {
                    ui.set_min_width(170.0);
                    for (etiqueta, codigo) in LENGUAJES {
                        if ui
                            .selectable_label(actual == *codigo, *etiqueta)
                            .clicked()
                        {
                            self.pestanas[self.activa].lenguaje_manual =
                                Some((*codigo).to_owned());
                            self.pestanas[self.activa].diag_sucio = true;
                            ui.close();
                        }
                    }
                },
            );
            ui.weak("o guarda el archivo con extensión (Ctrl+S).");
        });
        ui.add_space(2.0);
    }

    /// Barra de pestañas estilo VS Code: pestaña activa resaltada con línea de
    /// acento arriba, y ✕ para cerrar (● cuando hay cambios sin guardar).
    fn barra_pestanas(&mut self, ui: &mut egui::Ui) {
        let mut cerrar: Option<usize> = None;
        let mut activar: Option<usize> = None;
        let p = tema::p();

        egui::Frame::new().fill(p.tab_inactiva).show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("pestanas")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
                    ui.horizontal(|ui| {
                        for (i, pestana) in self.pestanas.iter().enumerate() {
                            let activa = i == self.activa;
                            let fondo = if activa { p.fondo_editor } else { p.tab_inactiva };
                            let marco = egui::Frame::new()
                                .fill(fondo)
                                .inner_margin(egui::Margin::symmetric(10, 6));
                            let contenido = marco.show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 7.0;
                                let color = if activa { p.texto_fuerte } else { p.texto_tenue };
                                let titulo = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(pestana.nombre())
                                            .size(12.5)
                                            .color(color),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if titulo.clicked() {
                                    activar = Some(i);
                                }
                                if tema::boton_cerrar(ui, 14.0, color, pestana.modificado)
                                    .on_hover_text("Cerrar")
                                    .clicked()
                                {
                                    cerrar = Some(i);
                                }
                            });
                            // Línea de acento sobre la pestaña activa (como VSC).
                            if activa {
                                let mut linea = contenido.response.rect;
                                linea.set_height(2.0);
                                ui.painter().rect_filled(
                                    linea,
                                    egui::CornerRadius::ZERO,
                                    p.acento,
                                );
                            }
                        }
                    });
                });
        });

        if let Some(i) = activar {
            self.activa = i;
        }
        if let Some(i) = cerrar {
            self.cerrar(i);
        }
    }

    fn pantalla_bienvenida(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    egui::RichText::new("Fulgur ⚡")
                        .size(30.0)
                        .color(tema::p().texto_tenue),
                );
                ui.add_space(14.0);
                ui.weak("Abrir archivo             Ctrl+O");
                ui.weak("Archivo nuevo            Ctrl+N");
                ui.weak("Guardar                  Ctrl+S");
                ui.add_space(8.0);
                ui.weak("Abre una carpeta desde el explorador para empezar.");
            });
        });
    }
}

/// Subraya con una línea ondulada roja cada error de sintaxis del archivo.
/// `origen` es dónde quedó dibujado el texto en pantalla (`galley_pos`), al que
/// hay que sumar las posiciones locales que devuelve el galley.
fn dibujar_subrayados(
    painter: &egui::Painter,
    galley: &egui::Galley,
    origen: egui::Pos2,
    diags: &[crate::diagnostico::Diagnostico],
) {
    use crate::diagnostico::Severidad;
    use egui::text::CCursor;
    for d in diags {
        let color = match d.severidad {
            Severidad::Error => egui::Color32::from_rgb(0xff, 0x55, 0x55),
            Severidad::Advertencia => egui::Color32::from_rgb(0xff, 0xc1, 0x07),
            Severidad::Info => egui::Color32::from_rgb(0x4f, 0xc3, 0xf7),
        };
        let r0 = galley.pos_from_cursor(CCursor::new(d.inicio_char));
        let r1 = galley.pos_from_cursor(CCursor::new(d.fin_char));
        let y = origen.y + r0.bottom() - 0.5;
        let x0 = origen.x + r0.left();
        // Error en una sola línea: subrayamos hasta su final. Si abarca varias,
        // subrayamos solo hasta el borde derecho de la primera línea.
        let misma_fila = (r1.top() - r0.top()).abs() < 1.0;
        let x1 = if misma_fila {
            origen.x + r1.left()
        } else {
            origen.x + galley.rect.right()
        };
        let x1 = x1.max(x0 + 6.0); // ancho mínimo: los tokens MISSING miden 0.
        subrayar_ondulado(painter, x0, x1, y, color);
    }
}

/// Dibuja una línea en zigzag (el clásico "garabato" de error) entre x0 y x1.
fn subrayar_ondulado(painter: &egui::Painter, x0: f32, x1: f32, y: f32, color: egui::Color32) {
    let paso = 2.5;
    let amplitud = 1.5;
    let mut puntos = Vec::new();
    let mut x = x0;
    let mut abajo = false;
    while x <= x1 {
        puntos.push(egui::pos2(x, y + if abajo { amplitud } else { 0.0 }));
        x += paso;
        abajo = !abajo;
    }
    if puntos.len() >= 2 {
        painter.add(egui::Shape::line(puntos, egui::Stroke::new(1.0, color)));
    }
}
