// Control de código con Git, estilo VS Code:
// estado del repo, preparar (stage), confirmar (commit), push/pull e init.
//
// Cada operación corre `git` en un hilo aparte y refresca el estado al acabar.

use crate::tema;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

pub struct ArchivoGit {
    /// Letra de estado de `git status --porcelain`: M, A, D, R, ?
    pub estado: char,
    pub ruta: String,
}

/// Lo que el hilo devuelve a la UI tras cada operación.
struct Datos {
    es_repo: bool,
    rama: String,
    preparados: Vec<ArchivoGit>,
    cambios: Vec<ArchivoGit>,
    resultado: Option<Result<String, String>>,
}

#[derive(Default)]
pub struct Git {
    raiz: Option<PathBuf>,
    es_repo: bool,
    rama: String,
    preparados: Vec<ArchivoGit>,
    cambios: Vec<ArchivoGit>,
    mensaje: String,
    salida: String,
    ocupado: bool,
    cargado: bool,
    rx: Option<Receiver<Datos>>,
}

impl Git {
    /// Se llama al abrir una carpeta: apunta Git a ella y fuerza un refresco.
    pub fn establecer_raiz(&mut self, dir: PathBuf) {
        self.raiz = Some(dir);
        self.cargado = false;
        self.salida.clear();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.recibir();

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("CONTROL DE CÓDIGO")
                    .size(11.0)
                    .color(tema::p().texto_tenue),
            );
            if self.ocupado {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("🔄").on_hover_text("Refrescar estado").clicked() {
                    self.operar(ui.ctx().clone(), None);
                }
            });
        });
        ui.add_space(4.0);

        let Some(raiz) = self.raiz.clone() else {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.weak("Abre una carpeta para\nusar el control de código.");
            });
            return;
        };

        // Primer refresco automático al mostrar el panel.
        if !self.cargado && !self.ocupado {
            self.cargado = true;
            self.operar(ui.ctx().clone(), None);
        }

        if !self.es_repo {
            ui.vertical_centered(|ui| {
                ui.weak(format!(
                    "«{}» no es un repositorio Git.",
                    raiz.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                ));
                ui.add_space(6.0);
                if ui.button("Inicializar repositorio").clicked() {
                    self.operar(ui.ctx().clone(), Some(vec!["init".into()]));
                }
            });
            self.pie(ui);
            return;
        }

        // Rama actual + push/pull.
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("rama:").size(12.5).color(tema::p().texto_tenue));
            ui.label(
                egui::RichText::new(&self.rama)
                    .size(12.5)
                    .color(tema::p().acento),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⬇ Pull").clicked() {
                    self.operar(ui.ctx().clone(), Some(vec!["pull".into()]));
                }
                if ui.small_button("⬆ Push").clicked() {
                    self.operar(ui.ctx().clone(), Some(vec!["push".into()]));
                }
            });
        });
        ui.add_space(4.0);

        // Mensaje + botón de commit.
        ui.add(
            egui::TextEdit::multiline(&mut self.mensaje)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text("Mensaje de commit"),
        );
        let puede_commit = !self.preparados.is_empty() && !self.mensaje.trim().is_empty();
        if ui
            .add_enabled(
                puede_commit && !self.ocupado,
                egui::Button::new("✔ Confirmar (commit)"),
            )
            .on_hover_text("Confirma los archivos preparados")
            .clicked()
        {
            let msg = self.mensaje.trim().to_owned();
            self.mensaje.clear();
            self.operar(ui.ctx().clone(), Some(vec!["commit".into(), "-m".into(), msg]));
        }
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;

                // --- Preparados (staged) ---
                let mut quitar: Option<String> = None;
                if !self.preparados.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("PREPARADOS ({})", self.preparados.len()))
                            .size(10.5)
                            .color(tema::p().texto_tenue),
                    );
                    for archivo in &self.preparados {
                        ui.horizontal(|ui| {
                            if ui.small_button("−").on_hover_text("Quitar de preparados").clicked() {
                                quitar = Some(archivo.ruta.clone());
                            }
                            etiqueta_estado(ui, archivo);
                        });
                    }
                    ui.add_space(6.0);
                }

                // --- Cambios sin preparar ---
                let mut preparar: Option<String> = None;
                let mut preparar_todo = false;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("CAMBIOS ({})", self.cambios.len()))
                            .size(10.5)
                            .color(tema::p().texto_tenue),
                    );
                    if !self.cambios.is_empty() && ui.small_button("+ todo").on_hover_text("Preparar todos los cambios").clicked() {
                        preparar_todo = true;
                    }
                });
                if self.cambios.is_empty() && self.preparados.is_empty() {
                    ui.weak("Sin cambios. Todo limpio ✨");
                }
                for archivo in &self.cambios {
                    ui.horizontal(|ui| {
                        if ui.small_button("+").on_hover_text("Preparar (git add)").clicked() {
                            preparar = Some(archivo.ruta.clone());
                        }
                        etiqueta_estado(ui, archivo);
                    });
                }

                if let Some(ruta) = quitar {
                    self.operar(
                        ui.ctx().clone(),
                        Some(vec!["restore".into(), "--staged".into(), "--".into(), ruta]),
                    );
                }
                if preparar_todo {
                    self.operar(ui.ctx().clone(), Some(vec!["add".into(), "-A".into()]));
                } else if let Some(ruta) = preparar {
                    self.operar(ui.ctx().clone(), Some(vec!["add".into(), "--".into(), ruta]));
                }
            });

        self.pie(ui);
    }

    /// Última salida de git (resultado o error) al pie del panel.
    fn pie(&self, ui: &mut egui::Ui) {
        if !self.salida.is_empty() {
            ui.separator();
            ui.weak(&self.salida);
        }
    }

    fn recibir(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(datos) = rx.try_recv() {
            self.es_repo = datos.es_repo;
            self.rama = datos.rama;
            self.preparados = datos.preparados;
            self.cambios = datos.cambios;
            match datos.resultado {
                Some(Ok(salida)) => self.salida = if salida.is_empty() { "Hecho ✔".into() } else { salida },
                Some(Err(e)) => self.salida = e,
                None => {}
            }
            self.ocupado = false;
        }
    }

    /// Ejecuta un comando git (o solo refresca si `args` es None) en un hilo.
    fn operar(&mut self, ctx: egui::Context, args: Option<Vec<String>>) {
        let Some(raiz) = self.raiz.clone() else { return };
        if self.ocupado {
            return;
        }
        self.ocupado = true;
        let (tx, rx) = channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let resultado = args.map(|a| {
                let refs: Vec<&str> = a.iter().map(String::as_str).collect();
                correr_git(&raiz, &refs)
            });

            let es_repo = correr_git(&raiz, &["rev-parse", "--is-inside-work-tree"])
                .map(|s| s.trim() == "true")
                .unwrap_or(false);
            let mut rama = String::new();
            let mut preparados = Vec::new();
            let mut cambios = Vec::new();
            if es_repo {
                rama = correr_git(&raiz, &["branch", "--show-current"])
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_default();
                if rama.is_empty() {
                    rama = "(sin rama)".to_owned();
                }
                if let Ok(estado) = correr_git(&raiz, &["status", "--porcelain"]) {
                    for linea in estado.lines() {
                        if linea.len() < 4 {
                            continue;
                        }
                        let x = linea.chars().next().unwrap_or(' ');
                        let y = linea.chars().nth(1).unwrap_or(' ');
                        // En los renombrados ("R  viejo -> nuevo") mostramos el nuevo nombre.
                        let ruta = linea[3..]
                            .split(" -> ")
                            .last()
                            .unwrap_or("")
                            .trim_matches('"')
                            .to_owned();
                        if x != ' ' && x != '?' {
                            preparados.push(ArchivoGit { estado: x, ruta: ruta.clone() });
                        }
                        if y != ' ' || x == '?' {
                            cambios.push(ArchivoGit {
                                estado: if x == '?' { '?' } else { y },
                                ruta,
                            });
                        }
                    }
                }
            }

            let _ = tx.send(Datos {
                es_repo,
                rama,
                preparados,
                cambios,
                resultado,
            });
            ctx.request_repaint();
        });
    }
}

fn etiqueta_estado(ui: &mut egui::Ui, archivo: &ArchivoGit) {
    let color = match archivo.estado {
        'M' | 'R' => egui::Color32::from_rgb(0xe2, 0xc0, 0x8d), // modificado
        'A' | '?' => egui::Color32::from_rgb(0x81, 0xc9, 0x95), // nuevo
        'D' => egui::Color32::from_rgb(0xf0, 0x88, 0x88),       // borrado
        _ => tema::p().texto_tenue,
    };
    ui.label(egui::RichText::new(archivo.estado.to_string()).monospace().color(color));
    ui.label(egui::RichText::new(&archivo.ruta).size(12.5));
}

/// Corre git con los argumentos dados y devuelve stdout (o stderr como error).
fn correr_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let salida = cmd
        .output()
        .map_err(|e| format!("No se pudo ejecutar git (¿está instalado?): {e}"))?;
    let stdout = String::from_utf8_lossy(&salida.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&salida.stderr).trim().to_owned();
    if salida.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}
