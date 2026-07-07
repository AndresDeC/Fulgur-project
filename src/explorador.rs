// Explorador de archivos estilo VS Code: árbol real con crear, renombrar
// y eliminar archivos/carpetas (botones de cabecera y menú de clic derecho).

use crate::tema;
use eframe::egui;
use std::path::{Path, PathBuf};

const CARPETAS_IGNORADAS: &[&str] = &["target", ".git", "node_modules", ".vs", "__pycache__"];

/// Eventos que el explorador comunica a la app (afectan al editor).
pub enum Accion {
    Abrir(PathBuf),
    /// Ruta eliminada (archivo o carpeta) — cerrar pestañas afectadas.
    Eliminado(PathBuf),
    /// (ruta vieja, ruta nueva) — actualizar pestañas abiertas.
    Renombrado(PathBuf, PathBuf),
}

/// Creación en curso de un archivo/carpeta (campo de texto inline).
struct Creacion {
    dir: PathBuf,
    es_carpeta: bool,
    nombre: String,
    enfocar: bool,
}

struct Renombre {
    ruta: PathBuf,
    nombre: String,
    enfocar: bool,
}

#[derive(Default)]
pub struct Explorador {
    pub raiz: Option<PathBuf>,
    seleccionado: Option<PathBuf>,
    creando: Option<Creacion>,
    renombrando: Option<Renombre>,
    eliminando: Option<PathBuf>,
    error: Option<String>,
}

impl Explorador {
    /// Directorio destino para "nuevo archivo/carpeta" desde la cabecera:
    /// la carpeta seleccionada, la carpeta del archivo seleccionado, o la raíz.
    fn dir_destino(&self) -> Option<PathBuf> {
        let raiz = self.raiz.clone()?;
        match &self.seleccionado {
            Some(s) if s.is_dir() => Some(s.clone()),
            Some(s) => s.parent().map(Path::to_path_buf).or(Some(raiz)),
            None => Some(raiz),
        }
    }

    fn empezar_creacion(&mut self, dir: PathBuf, es_carpeta: bool) {
        self.creando = Some(Creacion {
            dir,
            es_carpeta,
            nombre: String::new(),
            enfocar: true,
        });
        self.renombrando = None;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Accion> {
        let mut accion: Option<Accion> = None;

        // --- Cabecera: EXPLORADOR + botones nuevo archivo / nueva carpeta ---
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("EXPLORADOR")
                    .size(11.0)
                    .color(tema::p().texto_tenue),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.raiz.is_some() {
                    if ui
                        .small_button("📁+")
                        .on_hover_text("Nueva carpeta")
                        .clicked()
                    {
                        if let Some(dir) = self.dir_destino() {
                            self.empezar_creacion(dir, true);
                        }
                    }
                    if ui
                        .small_button("📄+")
                        .on_hover_text("Nuevo archivo")
                        .clicked()
                    {
                        if let Some(dir) = self.dir_destino() {
                            self.empezar_creacion(dir, false);
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);

        match self.raiz.clone() {
            None => {
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    ui.weak("Todavía no has abierto\nninguna carpeta.");
                    ui.add_space(8.0);
                    if ui.button("Abrir carpeta").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            self.raiz = Some(dir);
                        }
                    }
                });
            }
            Some(raiz) => {
                let nombre = raiz
                    .file_name()
                    .map(|n| n.to_string_lossy().to_uppercase())
                    .unwrap_or_else(|| raiz.display().to_string());
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(nombre).size(11.5).strong());
                });
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        self.contenido_dir(ui, &raiz, &mut accion);
                        // Zona vacía debajo del árbol: clic derecho para crear en la raíz.
                        let restante = ui.available_size_before_wrap();
                        if restante.y > 0.0 {
                            let (_, resp) = ui.allocate_exact_size(
                                egui::vec2(restante.x, restante.y.max(40.0)),
                                egui::Sense::click(),
                            );
                            if resp.clicked() {
                                self.seleccionado = None;
                            }
                            resp.context_menu(|ui| {
                                if ui.button("Nuevo archivo…").clicked() {
                                    self.empezar_creacion(raiz.clone(), false);
                                    ui.close();
                                }
                                if ui.button("Nueva carpeta…").clicked() {
                                    self.empezar_creacion(raiz.clone(), true);
                                    ui.close();
                                }
                            });
                        }
                    });
            }
        }

        // Errores de operaciones de archivo (crear/renombrar/eliminar).
        if let Some(e) = self.error.clone() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(240, 110, 110), &e);
                if tema::boton_cerrar(ui, 14.0, tema::p().texto_tenue, false).clicked() {
                    self.error = None;
                }
            });
        }

        // Diálogo de confirmación de borrado.
        if let Some(a) = self.dialogo_eliminar(ui.ctx()) {
            accion = Some(a);
        }

        accion
    }

    /// Pinta el contenido de un directorio (recursivo).
    fn contenido_dir(&mut self, ui: &mut egui::Ui, dir: &Path, accion: &mut Option<Accion>) {
        // Campo inline "nuevo archivo/carpeta" si toca en este directorio.
        if self.creando.as_ref().is_some_and(|c| c.dir == dir) {
            if let Some(a) = self.campo_creacion(ui) {
                *accion = Some(a);
            }
        }

        let Ok(entradas) = std::fs::read_dir(dir) else {
            ui.weak("(sin acceso)");
            return;
        };
        let mut carpetas: Vec<PathBuf> = Vec::new();
        let mut archivos: Vec<PathBuf> = Vec::new();
        for entrada in entradas.flatten() {
            let ruta = entrada.path();
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            if ruta.is_dir() {
                if !CARPETAS_IGNORADAS.contains(&nombre.as_str()) {
                    carpetas.push(ruta);
                }
            } else {
                archivos.push(ruta);
            }
        }
        carpetas.sort();
        archivos.sort();

        for carpeta in carpetas {
            self.nodo_carpeta(ui, &carpeta, accion);
        }
        for archivo in archivos {
            self.nodo_archivo(ui, &archivo, accion);
        }
    }

    fn nodo_carpeta(&mut self, ui: &mut egui::Ui, carpeta: &Path, accion: &mut Option<Accion>) {
        // Si se está renombrando esta carpeta, mostrar el campo de texto en su lugar.
        if self.renombrando.as_ref().is_some_and(|r| r.ruta == carpeta) {
            if let Some(a) = self.campo_renombre(ui) {
                *accion = Some(a);
            }
            return;
        }

        let nombre = carpeta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let respuesta = egui::CollapsingHeader::new(egui::RichText::new(nombre).size(13.0))
            .id_salt(carpeta)
            .show(ui, |ui| {
                self.contenido_dir(ui, carpeta, accion);
            });

        if respuesta.header_response.clicked() {
            self.seleccionado = Some(carpeta.to_path_buf());
        }
        let carpeta = carpeta.to_path_buf();
        respuesta.header_response.context_menu(|ui| {
            if ui.button("Nuevo archivo…").clicked() {
                self.empezar_creacion(carpeta.clone(), false);
                ui.close();
            }
            if ui.button("Nueva carpeta…").clicked() {
                self.empezar_creacion(carpeta.clone(), true);
                ui.close();
            }
            ui.separator();
            if ui.button("Renombrar…").clicked() {
                self.empezar_renombre(&carpeta);
                ui.close();
            }
            if ui.button("Eliminar").clicked() {
                self.eliminando = Some(carpeta.clone());
                ui.close();
            }
        });
    }

    fn nodo_archivo(&mut self, ui: &mut egui::Ui, archivo: &Path, accion: &mut Option<Accion>) {
        if self.renombrando.as_ref().is_some_and(|r| r.ruta == archivo) {
            if let Some(a) = self.campo_renombre(ui) {
                *accion = Some(a);
            }
            return;
        }

        let nombre = archivo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let seleccionado = self.seleccionado.as_deref() == Some(archivo);
        let respuesta = ui.selectable_label(
            seleccionado,
            egui::RichText::new(format!("  {nombre}")).size(13.0),
        );
        if respuesta.clicked() {
            self.seleccionado = Some(archivo.to_path_buf());
            *accion = Some(Accion::Abrir(archivo.to_path_buf()));
        }
        let archivo = archivo.to_path_buf();
        respuesta.context_menu(|ui| {
            if ui.button("Renombrar…").clicked() {
                self.empezar_renombre(&archivo);
                ui.close();
            }
            if ui.button("Eliminar").clicked() {
                self.eliminando = Some(archivo.clone());
                ui.close();
            }
        });
    }

    fn empezar_renombre(&mut self, ruta: &Path) {
        self.renombrando = Some(Renombre {
            ruta: ruta.to_path_buf(),
            nombre: ruta
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            enfocar: true,
        });
        self.creando = None;
    }

    /// Campo de texto inline para crear archivo/carpeta. Enter confirma, Esc cancela.
    fn campo_creacion(&mut self, ui: &mut egui::Ui) -> Option<Accion> {
        let Some(creando) = &mut self.creando else {
            return None;
        };
        let mut resultado = None;
        let mut cerrar = false;

        ui.horizontal(|ui| {
            ui.label(if creando.es_carpeta { "📁" } else { "📄" });
            let campo = ui.add(
                egui::TextEdit::singleline(&mut creando.nombre)
                    .hint_text(if creando.es_carpeta { "nombre de la carpeta" } else { "nombre del archivo" })
                    .desired_width(f32::INFINITY),
            );
            if creando.enfocar {
                campo.request_focus();
                creando.enfocar = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cerrar = true;
            } else if campo.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !creando.nombre.trim().is_empty() {
                    let destino = creando.dir.join(creando.nombre.trim());
                    if destino.exists() {
                        self.error = Some(format!("Ya existe «{}».", creando.nombre.trim()));
                    } else if creando.es_carpeta {
                        match std::fs::create_dir_all(&destino) {
                            Ok(()) => {}
                            Err(e) => self.error = Some(format!("No se pudo crear la carpeta: {e}")),
                        }
                    } else {
                        match std::fs::write(&destino, "") {
                            Ok(()) => {
                                self.seleccionado = Some(destino.clone());
                                resultado = Some(Accion::Abrir(destino));
                            }
                            Err(e) => self.error = Some(format!("No se pudo crear el archivo: {e}")),
                        }
                    }
                }
                cerrar = true;
            }
        });

        if cerrar {
            self.creando = None;
        }
        resultado
    }

    /// Campo de texto inline para renombrar. Enter confirma, Esc cancela.
    fn campo_renombre(&mut self, ui: &mut egui::Ui) -> Option<Accion> {
        let Some(renombre) = &mut self.renombrando else {
            return None;
        };
        let mut resultado = None;
        let mut cerrar = false;

        ui.horizontal(|ui| {
            let campo = ui.add(
                egui::TextEdit::singleline(&mut renombre.nombre).desired_width(f32::INFINITY),
            );
            if renombre.enfocar {
                campo.request_focus();
                renombre.enfocar = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cerrar = true;
            } else if campo.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !renombre.nombre.trim().is_empty() {
                    let nueva = renombre.ruta.with_file_name(renombre.nombre.trim());
                    if nueva != renombre.ruta {
                        if nueva.exists() {
                            self.error = Some(format!("Ya existe «{}».", renombre.nombre.trim()));
                        } else {
                            match std::fs::rename(&renombre.ruta, &nueva) {
                                Ok(()) => {
                                    self.seleccionado = Some(nueva.clone());
                                    resultado =
                                        Some(Accion::Renombrado(renombre.ruta.clone(), nueva));
                                }
                                Err(e) => {
                                    self.error = Some(format!("No se pudo renombrar: {e}"));
                                }
                            }
                        }
                    }
                }
                cerrar = true;
            }
        });

        if cerrar {
            self.renombrando = None;
        }
        resultado
    }

    /// Ventana de confirmación antes de eliminar.
    fn dialogo_eliminar(&mut self, ctx: &egui::Context) -> Option<Accion> {
        let ruta = self.eliminando.clone()?;
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut resultado = None;

        egui::Window::new("Confirmar eliminación")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let tipo = if ruta.is_dir() { "la carpeta" } else { "el archivo" };
                ui.label(format!("¿Eliminar {tipo} «{nombre}»?"));
                ui.weak("Esta acción no se puede deshacer.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new("Eliminar").color(egui::Color32::from_rgb(255, 120, 120)))
                        .clicked()
                    {
                        let r = if ruta.is_dir() {
                            std::fs::remove_dir_all(&ruta)
                        } else {
                            std::fs::remove_file(&ruta)
                        };
                        match r {
                            Ok(()) => resultado = Some(Accion::Eliminado(ruta.clone())),
                            Err(e) => self.error = Some(format!("No se pudo eliminar: {e}")),
                        }
                        self.eliminando = None;
                    }
                    if ui.button("Cancelar").clicked() {
                        self.eliminando = None;
                    }
                });
            });

        resultado
    }
}
