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
        });
        Ok(())
    }

    pub fn nueva(&mut self) {
        self.empujar(Pestana {
            ruta: None,
            contenido: String::new(),
            modificado: false,
            lenguaje_manual: None,
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

        let respuesta = egui::ScrollArea::both()
            .id_salt("editor")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut pestana.contenido)
                        .id(id_texto)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(40)
                        .layouter(&mut layouter),
                )
            });
        if respuesta.inner.changed() {
            pestana.modificado = true;
        }
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
