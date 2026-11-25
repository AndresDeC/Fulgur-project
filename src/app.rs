// La aplicación Fulgur: layout estilo VS Code.
//
//  ┌──────────────────────────────────────────────┐
//  │ menú: Archivo Edición Ver Terminal Ayuda     │
//  ├──┬─────────────┬─────────────────────────────┤
//  │ac│  lateral    │  pestañas + editor          │
//  │ti│ explorador/ │                             │
//  │vi│ git/        │                             │
//  │da│ extensiones ├─────────────────────────────┤
//  │d │             │  TERMINAL · IA              │
//  ├──┴─────────────┴─────────────────────────────┤
//  │ barra de estado                              │
//  └──────────────────────────────────────────────┘

use crate::editor::{self, Editor};
use crate::explorador::{Accion, Explorador};
use crate::git::Git;
use crate::ia::Ia;
use crate::lsp::GestorLsp;
use crate::mercado::Mercado;
use crate::tema;
use crate::terminal::Terminal;
use eframe::egui;

#[derive(PartialEq, Clone, Copy)]
enum VistaLateral {
    Explorador,
    Git,
    Mercado,
}

#[derive(PartialEq, Clone, Copy)]
enum PanelInferior {
    Terminal,
    Ia,
}

pub struct Fulgur {
    explorador: Explorador,
    editor: Editor,
    terminal: Terminal,
    ia: Ia,
    mercado: Mercado,
    git: Git,
    lsp: GestorLsp,
    vista_lateral: VistaLateral,
    mostrar_lateral: bool,
    panel_inferior: PanelInferior,
    mostrar_inferior: bool,
    mostrar_acerca: bool,
    estado: String,
    tema_inicial_aplicado: bool,
}

impl Default for Fulgur {
    fn default() -> Self {
        Self {
            explorador: Explorador::default(),
            editor: Editor::default(),
            terminal: Terminal::default(),
            ia: Ia::default(),
            mercado: Mercado::default(),
            git: Git::default(),
            lsp: GestorLsp::default(),
            vista_lateral: VistaLateral::Explorador,
            mostrar_lateral: true,
            panel_inferior: PanelInferior::Terminal,
            mostrar_inferior: true,
            mostrar_acerca: false,
            estado: "Listo".to_owned(),
            tema_inicial_aplicado: false,
        }
    }
}

impl eframe::App for Fulgur {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !self.tema_inicial_aplicado {
            // Tema integrado guardado.
            tema::aplicar_indice(ui.ctx(), tema::cargar_guardado());
            self.tema_inicial_aplicado = true;
        }

        self.atajos(ui.ctx());
        self.actualizar_lsp(ui.ctx());

        // --- Barra de menú ---
        egui::Panel::top("menu")
            .frame(
                egui::Frame::new()
                    .fill(tema::p().barra_menu)
                    .inner_margin(egui::Margin::symmetric(6, 4)),
            )
            .show_separator_line(false)
            .show_inside(ui, |ui| self.barra_menu(ui));

        // --- Barra de estado (abajo del todo) ---
        egui::Panel::bottom("estado")
            .frame(
                egui::Frame::new()
                    .fill(tema::p().barra_estado)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show_separator_line(false)
            .show_inside(ui, |ui| self.barra_estado(ui));

        // --- Barra de actividad (iconos, izquierda) ---
        egui::Panel::left("actividad")
            .exact_size(46.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(tema::p().barra_actividad))
            .show_separator_line(false)
            .show_inside(ui, |ui| self.barra_actividad(ui));

        // --- Barra lateral: explorador / git / mercado ---
        if self.mostrar_lateral {
            egui::Panel::left("lateral")
                .resizable(true)
                .default_size(250.0)
                .frame(egui::Frame::new().fill(tema::p().barra_lateral))
                .show_inside(ui, |ui| match self.vista_lateral {
                    VistaLateral::Explorador => {
                        if let Some(accion) = self.explorador.ui(ui) {
                            self.procesar_accion_explorador(accion);
                        }
                    }
                    VistaLateral::Git => self.git.ui(ui),
                    VistaLateral::Mercado => {
                        if let Some(snippet) = self.mercado.ui(ui) {
                            self.insertar_snippet(&snippet);
                        }
                    }
                });
        }

        // --- Panel inferior: TERMINAL · IA ---
        if self.mostrar_inferior {
            egui::Panel::bottom("panel_inferior")
                .resizable(true)
                .default_size(210.0)
                .frame(
                    egui::Frame::new()
                        .fill(tema::p().fondo_editor)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        if pestana_panel(ui, self.panel_inferior == PanelInferior::Terminal, "TERMINAL") {
                            self.panel_inferior = PanelInferior::Terminal;
                        }
                        if pestana_panel(ui, self.panel_inferior == PanelInferior::Ia, "IA") {
                            self.panel_inferior = PanelInferior::Ia;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if tema::boton_cerrar(ui, 15.0, tema::p().texto_tenue, false)
                                .on_hover_text("Ocultar panel")
                                .clicked()
                            {
                                self.mostrar_inferior = false;
                            }
                        });
                    });
                    ui.add_space(2.0);
                    match self.panel_inferior {
                        PanelInferior::Terminal => self.terminal.ui(ui),
                        PanelInferior::Ia => {
                            let contexto = self
                                .editor
                                .contexto_activo()
                                .map(|(n, c)| (n, c.to_owned()));
                            self.ia.ui(ui, contexto);
                        }
                    }
                });
        }

        // --- Centro: pestañas + editor ---
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(tema::p().fondo_editor))
            .show_inside(ui, |ui| {
                self.editor.ui(ui);
            });

        self.ventana_acerca(ui.ctx());
    }
}

/// Elige singular o plural según la cantidad.
fn plural<'a>(n: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if n == 1 { singular } else { plural }
}

/// Pestaña del panel inferior estilo VSC: texto pequeño en mayúsculas,
/// subrayado cuando está activa.
fn pestana_panel(ui: &mut egui::Ui, activa: bool, texto: &str) -> bool {
    let p = tema::p();
    let color = if activa { p.texto_fuerte } else { p.texto_tenue };
    let respuesta = ui.add(
        egui::Label::new(egui::RichText::new(texto).size(11.0).color(color))
            .sense(egui::Sense::click()),
    );
    if activa {
        let mut linea = respuesta.rect;
        linea.set_top(respuesta.rect.bottom() + 2.0);
        linea.set_height(1.0);
        ui.painter()
            .rect_filled(linea, egui::CornerRadius::ZERO, p.texto_fuerte);
    }
    respuesta.clicked()
}

impl Fulgur {
    fn barra_actividad(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            self.icono_actividad(ui, VistaLateral::Explorador, "📁", "Explorador");
            ui.add_space(4.0);
            self.icono_actividad(ui, VistaLateral::Git, "🔀", "Control de código (Git)");
            ui.add_space(4.0);
            self.icono_actividad(ui, VistaLateral::Mercado, "🧩", "Extensiones");
        });
    }

    fn icono_actividad(
        &mut self,
        ui: &mut egui::Ui,
        vista: VistaLateral,
        icono: &str,
        nombre: &str,
    ) {
        // La barra de actividad es oscura en los tres temas (como en VSC),
        // así que los iconos van siempre en blanco/gris.
        let activo = self.mostrar_lateral && self.vista_lateral == vista;
        let color = if activo {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(0x8c, 0x8c, 0x8c)
        };
        let boton = ui.add(
            egui::Button::new(egui::RichText::new(icono).size(20.0).color(color)).frame(false),
        );
        if activo {
            let r = boton.rect;
            let linea = egui::Rect::from_min_size(
                egui::pos2(ui.max_rect().left(), r.top()),
                egui::vec2(2.0, r.height()),
            );
            ui.painter()
                .rect_filled(linea, egui::CornerRadius::ZERO, tema::p().acento);
        }
        if boton.on_hover_text(nombre).clicked() {
            if activo {
                self.mostrar_lateral = false;
            } else {
                self.vista_lateral = vista;
                self.mostrar_lateral = true;
            }
        }
    }

    fn barra_menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(egui::RichText::new("⚡").color(tema::p().acento));

            ui.menu_button("Archivo", |ui| {
                if ui.button("Nuevo archivo         Ctrl+N").clicked() {
                    self.editor.nueva();
                    ui.close();
                }
                if ui.button("Abrir archivo…        Ctrl+O").clicked() {
                    if let Some(ruta) = rfd::FileDialog::new().pick_file() {
                        self.abrir_archivo(ruta);
                    }
                    ui.close();
                }
                if ui.button("Abrir carpeta…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.abrir_carpeta(dir);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Guardar               Ctrl+S").clicked() {
                    self.guardar();
                    ui.close();
                }
                if ui.button("Guardar como…").clicked() {
                    match self.editor.guardar_como_activa() {
                        Ok(Some(ruta)) => self.estado = format!("Guardado: {}", ruta.display()),
                        Ok(None) => {}
                        Err(e) => self.estado = e,
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Cerrar pestaña        Ctrl+W").clicked() {
                    self.editor.cerrar_activa();
                    ui.close();
                }
                if ui.button("Salir").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Edición", |ui| {
                if ui.button("Deshacer              Ctrl+Z").clicked() {
                    self.tecla_al_editor(ui.ctx(), egui::Key::Z);
                    ui.close();
                }
                if ui.button("Rehacer               Ctrl+Y").clicked() {
                    self.tecla_al_editor(ui.ctx(), egui::Key::Y);
                    ui.close();
                }
                ui.separator();
                if ui.button("Cortar                Ctrl+X").clicked() {
                    self.evento_al_editor(ui.ctx(), egui::Event::Cut);
                    ui.close();
                }
                if ui.button("Copiar                Ctrl+C").clicked() {
                    self.evento_al_editor(ui.ctx(), egui::Event::Copy);
                    ui.close();
                }
                if ui.button("Pegar                 Ctrl+V").clicked() {
                    // El portapapeles se lee con arboard porque egui solo
                    // recibe "pegar" desde el atajo de teclado del sistema.
                    if let Ok(texto) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                        self.evento_al_editor(ui.ctx(), egui::Event::Paste(texto));
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Seleccionar todo      Ctrl+A").clicked() {
                    self.tecla_al_editor(ui.ctx(), egui::Key::A);
                    ui.close();
                }
            });

            ui.menu_button("Ver", |ui| {
                ui.menu_button("Tema", |ui| {
                    ui.set_min_width(160.0);
                    let actual = tema::indice_actual();
                    for (i, paleta) in tema::temas().iter().enumerate() {
                        if ui.selectable_label(actual == i, paleta.nombre).clicked() {
                            tema::aplicar_indice(ui.ctx(), i);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                ui.checkbox(&mut self.mostrar_lateral, "Barra lateral");
                ui.checkbox(&mut self.mostrar_inferior, "Panel inferior");
            });

            ui.menu_button("Terminal", |ui| {
                if ui.button("Mostrar terminal").clicked() {
                    self.mostrar_inferior = true;
                    self.panel_inferior = PanelInferior::Terminal;
                    ui.close();
                }
                if ui.button("Mostrar IA").clicked() {
                    self.mostrar_inferior = true;
                    self.panel_inferior = PanelInferior::Ia;
                    ui.close();
                }
            });

            ui.menu_button("Ayuda", |ui| {
                if ui.button("Acerca de Fulgur").clicked() {
                    self.mostrar_acerca = true;
                    ui.close();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(raiz) = &self.explorador.raiz {
                    ui.label(
                        egui::RichText::new(raiz.display().to_string())
                            .size(11.5)
                            .color(tema::p().texto_tenue),
                    );
                }
            });
        });
    }

    /// Devuelve el foco al editor e inyecta una pulsación Ctrl+tecla,
    /// para que Deshacer/Rehacer del menú funcionen sobre el texto.
    fn tecla_al_editor(&self, ctx: &egui::Context, tecla: egui::Key) {
        self.evento_al_editor(
            ctx,
            egui::Event::Key {
                key: tecla,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
        );
    }

    fn evento_al_editor(&self, ctx: &egui::Context, evento: egui::Event) {
        if let Some(id) = self.editor.id_editor_activo() {
            ctx.memory_mut(|m| m.request_focus(id));
            ctx.input_mut(|i| i.events.push(evento));
        }
    }

    fn barra_estado(&mut self, ui: &mut egui::Ui) {
        let p = tema::p();
        ui.horizontal(|ui| {
            let estado = |t: &str| egui::RichText::new(t).size(11.5).color(p.texto_estado);
            ui.label(estado(&self.estado));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(estado("Fulgur ⚡"));
                ui.separator();
                ui.label(estado(&format!("🧩 {}", self.mercado.instaladas.len())));
                ui.separator();

                // Selector de lenguaje de la pestaña activa, como en VSC.
                if let Some(pestana) = self.editor.pestana_activa() {
                    let actual = pestana.lenguaje();
                    ui.menu_button(estado(&editor::etiqueta_lenguaje(&actual)), |ui| {
                        ui.set_min_width(160.0);
                        for (etiqueta, codigo) in editor::LENGUAJES {
                            let elegido = actual == *codigo;
                            if ui.selectable_label(elegido, *etiqueta).clicked() {
                                if let Some(pe) = self.editor.pestana_activa_mut() {
                                    pe.lenguaje_manual = Some((*codigo).to_owned());
                                    pe.diag_sucio = true;
                                }
                                ui.close();
                            }
                        }
                    });
                }

                // Diagnósticos de la pestaña activa (LSP o tree-sitter).
                if let Some((analizado, errores, avisos)) = self.editor.analisis_activo() {
                    if analizado {
                        ui.separator();
                        let (texto, color) = if errores > 0 {
                            (
                                format!("⚠ {errores} {}", plural(errores, "error", "errores")),
                                egui::Color32::from_rgb(0xff, 0x8a, 0x8a),
                            )
                        } else if avisos > 0 {
                            (
                                format!("{avisos} {}", plural(avisos, "aviso", "avisos")),
                                egui::Color32::from_rgb(0xff, 0xc1, 0x07),
                            )
                        } else {
                            (
                                "Sin problemas".to_owned(),
                                egui::Color32::from_rgb(0x8a, 0xe0, 0x9a),
                            )
                        };
                        // Detalle para el tooltip: qué falla y en qué línea.
                        let detalle: Vec<String> = self
                            .editor
                            .diagnosticos_activos()
                            .iter()
                            .take(14)
                            .map(|d| format!("Línea {}: {}", d.linea, d.mensaje))
                            .collect();
                        let fuente = if self.editor.activa_usa_lsp() {
                            "Análisis semántico del servidor de lenguaje:"
                        } else {
                            "Detectado por tree-sitter, sin compilar:"
                        };
                        let resp = ui.label(egui::RichText::new(texto).size(11.5).color(color));
                        if !detalle.is_empty() {
                            resp.on_hover_ui(|ui| {
                                ui.weak(fuente);
                                for linea in &detalle {
                                    ui.label(linea);
                                }
                            });
                        }
                    }
                }

                // Indicador del servidor LSP activo.
                if self.editor.activa_usa_lsp() {
                    if let Some(lang) = self.editor.pestana_activa().map(|p| p.lenguaje()) {
                        if let Some((nombre, _)) = crate::lsp::servidor_para(&lang) {
                            ui.separator();
                            ui.label(estado(nombre))
                                .on_hover_text("Servidor de lenguaje (LSP) activo");
                        }
                    }
                }
            });
        });
    }

    fn ventana_acerca(&mut self, ctx: &egui::Context) {
        if !self.mostrar_acerca {
            return;
        }
        egui::Window::new("Acerca de Fulgur")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Fulgur ⚡").size(24.0));
                    ui.label(format!("versión {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(6.0);
                    ui.weak("IDE nativo, ultraligero, escrito en Rust.");
                    ui.weak("Editor · Terminal · Git · IA · Extensiones");
                    ui.add_space(8.0);
                    if ui.button("Cerrar").clicked() {
                        self.mostrar_acerca = false;
                    }
                });
            });
    }

    fn atajos(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.guardar();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.editor.nueva();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            if let Some(ruta) = rfd::FileDialog::new().pick_file() {
                self.abrir_archivo(ruta);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
            self.editor.cerrar_activa();
        }
    }

    /// Mantiene al servidor LSP al día con el archivo activo y vuelca sus
    /// diagnósticos en la pestaña. Se ejecuta una vez por fotograma.
    fn actualizar_lsp(&mut self, ctx: &egui::Context) {
        self.lsp.recibir();

        let Some(pestana) = self.editor.pestana_activa_mut() else {
            return;
        };
        // Solo archivos guardados (con ruta) y con servidor para su lenguaje.
        let Some(ruta) = pestana.ruta.clone() else {
            pestana.usa_lsp = false;
            return;
        };
        let lenguaje = pestana.lenguaje();
        if crate::lsp::servidor_para(&lenguaje).is_none() {
            pestana.usa_lsp = false;
            return;
        }

        let cambiado = pestana.diag_sucio;
        let gestionado =
            self.lsp
                .sincronizar(&ruta, &lenguaje, &pestana.contenido, cambiado, ctx);
        pestana.usa_lsp = gestionado;
        if gestionado {
            // La app es dueña de los diagnósticos de este archivo (no tree-sitter).
            pestana.diag_sucio = false;
            if let Some(diags) = self.lsp.diagnosticos(&ruta, &lenguaje, &pestana.contenido) {
                pestana.diagnosticos = diags;
            }
        }
    }

    fn procesar_accion_explorador(&mut self, accion: Accion) {
        match accion {
            Accion::Abrir(ruta) => self.abrir_archivo(ruta),
            Accion::Eliminado(ruta) => {
                self.editor.cerrar_por_ruta(&ruta);
                self.estado = format!("Eliminado: {}", ruta.display());
            }
            Accion::Renombrado(vieja, nueva) => {
                self.editor.renombrar_ruta(&vieja, &nueva);
                self.estado = format!("Renombrado a: {}", nueva.display());
            }
        }
    }

    fn abrir_archivo(&mut self, ruta: std::path::PathBuf) {
        match self.editor.abrir(ruta.clone()) {
            Ok(()) => self.estado = format!("Abierto: {}", ruta.display()),
            Err(e) => self.estado = e,
        }
    }

    fn abrir_carpeta(&mut self, dir: std::path::PathBuf) {
        self.explorador.raiz = Some(dir.clone());
        self.terminal.cambiar_cwd(dir.clone());
        self.git.establecer_raiz(dir.clone());
        self.lsp.establecer_raiz(dir.clone());
        self.vista_lateral = VistaLateral::Explorador;
        self.mostrar_lateral = true;
        self.estado = format!("Carpeta abierta: {}", dir.display());
    }

    fn guardar(&mut self) {
        match self.editor.guardar_activa() {
            Ok(Some(ruta)) => self.estado = format!("Guardado: {}", ruta.display()),
            Ok(None) => {}
            Err(e) => self.estado = e,
        }
    }

    fn insertar_snippet(&mut self, snippet: &str) {
        if self.editor.pestanas.is_empty() {
            self.editor.nueva();
        }
        let activa = self.editor.activa;
        if let Some(pestana) = self.editor.pestanas.get_mut(activa) {
            if !pestana.contenido.ends_with('\n') && !pestana.contenido.is_empty() {
                pestana.contenido.push('\n');
            }
            pestana.contenido.push_str(snippet);
            pestana.contenido.push('\n');
            pestana.modificado = true;
            self.estado = "Snippet insertado al final del archivo".to_owned();
        }
    }
}
