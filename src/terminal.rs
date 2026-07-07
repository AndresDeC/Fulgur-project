// Terminal integrada: ejecuta comandos reales en PowerShell.
//
// Cada comando se lanza en un hilo aparte y su salida llega en vivo
// a través de un canal; la UI nunca se bloquea.

use eframe::egui;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Mensajes que el hilo de ejecución envía a la UI.
enum Msg {
    Salida(String),
    Error(String),
    Fin(i32),
}

/// Una línea del historial de la terminal.
pub struct Linea {
    pub es_error: bool,
    pub texto: String,
}

pub struct Terminal {
    pub entrada: String,
    pub lineas: Vec<Linea>,
    /// Directorio de trabajo actual (persiste entre comandos; `cd` lo cambia).
    pub cwd: PathBuf,
    pub ejecutando: bool,
    historial: Vec<String>,
    idx_historial: Option<usize>,
    rx: Option<Receiver<Msg>>,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            entrada: String::new(),
            lineas: vec![Linea {
                es_error: false,
                texto: "Fulgur ⚡ terminal — PowerShell conectado. Escribe un comando y pulsa Enter.".to_owned(),
            }],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            ejecutando: false,
            historial: Vec::new(),
            idx_historial: None,
            rx: None,
        }
    }
}

impl Terminal {
    /// Cambia el directorio de trabajo (se llama al abrir una carpeta en el explorador).
    pub fn cambiar_cwd(&mut self, dir: PathBuf) {
        self.lineas.push(Linea {
            es_error: false,
            texto: format!("(directorio de trabajo: {})", dir.display()),
        });
        self.cwd = dir;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Recoge la salida que hayan producido los hilos desde el último fotograma.
        self.recibir_mensajes();

        // Zona de salida.
        let alto_entrada = 28.0;
        egui::ScrollArea::vertical()
            .id_salt("salida_terminal")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height(ui.available_height() - alto_entrada)
            .show(ui, |ui| {
                for linea in &self.lineas {
                    let texto = egui::RichText::new(&linea.texto).monospace();
                    if linea.es_error {
                        ui.label(texto.color(egui::Color32::from_rgb(240, 100, 100)));
                    } else {
                        ui.label(texto);
                    }
                }
                if self.ejecutando {
                    ui.spinner();
                }
            });

        // Línea de entrada.
        ui.horizontal(|ui| {
            ui.monospace(format!(
                "PS {}>",
                self.cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.cwd.display().to_string())
            ));
            let respuesta = ui.add_sized(
                egui::vec2(ui.available_width(), 20.0),
                egui::TextEdit::singleline(&mut self.entrada)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("comando…"),
            );

            // Historial con flechas arriba/abajo.
            if respuesta.has_focus() && !self.historial.is_empty() {
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    let idx = match self.idx_historial {
                        Some(0) => 0,
                        Some(i) => i - 1,
                        None => self.historial.len() - 1,
                    };
                    self.idx_historial = Some(idx);
                    self.entrada = self.historial[idx].clone();
                } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    if let Some(i) = self.idx_historial {
                        if i + 1 < self.historial.len() {
                            self.idx_historial = Some(i + 1);
                            self.entrada = self.historial[i + 1].clone();
                        } else {
                            self.idx_historial = None;
                            self.entrada.clear();
                        }
                    }
                }
            }

            if respuesta.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.ejecutar(ui.ctx().clone());
                respuesta.request_focus();
            }
        });
    }

    fn recibir_mensajes(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut terminado = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Salida(t) => self.lineas.push(Linea { es_error: false, texto: t }),
                Msg::Error(t) => self.lineas.push(Linea { es_error: true, texto: t }),
                Msg::Fin(codigo) => {
                    if codigo != 0 {
                        self.lineas.push(Linea {
                            es_error: true,
                            texto: format!("(el comando terminó con código {codigo})"),
                        });
                    }
                    terminado = true;
                }
            }
        }
        if terminado {
            self.ejecutando = false;
            self.rx = None;
        }
    }

    fn ejecutar(&mut self, ctx: egui::Context) {
        let comando = self.entrada.trim().to_owned();
        self.entrada.clear();
        if comando.is_empty() || self.ejecutando {
            return;
        }
        self.historial.push(comando.clone());
        self.idx_historial = None;
        self.lineas.push(Linea {
            es_error: false,
            texto: format!("PS {}> {}", self.cwd.display(), comando),
        });

        // Comandos internos que gestionamos nosotros para que el estado persista.
        if comando == "cls" || comando == "clear" {
            self.lineas.clear();
            return;
        }
        if comando == "cd" {
            let cwd = self.cwd.display().to_string();
            self.lineas.push(Linea { es_error: false, texto: cwd });
            return;
        }
        if let Some(destino) = comando.strip_prefix("cd ") {
            let destino = destino.trim().trim_matches('"').trim_matches('\'');
            let nueva = if std::path::Path::new(destino).is_absolute() {
                PathBuf::from(destino)
            } else {
                self.cwd.join(destino)
            };
            match nueva.canonicalize() {
                Ok(nueva) if nueva.is_dir() => self.cwd = nueva,
                _ => self.lineas.push(Linea {
                    es_error: true,
                    texto: format!("cd: no existe el directorio «{destino}»"),
                }),
            }
            return;
        }

        // Comando real: se lanza PowerShell en un hilo aparte.
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.ejecutando = true;
        let cwd = self.cwd.clone();
        std::thread::spawn(move || Self::hilo_comando(comando, cwd, tx, ctx));
    }

    fn hilo_comando(comando: String, cwd: PathBuf, tx: Sender<Msg>, ctx: egui::Context) {
        // Forzamos UTF-8 para que acentos y símbolos lleguen bien a la UI.
        let script = format!(
            "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $OutputEncoding=[System.Text.Encoding]::UTF8; {comando}"
        );
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Evita que aparezca una ventana de consola fantasma al ejecutar cada comando.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        match cmd.spawn() {
            Ok(mut hijo) => {
                let stdout = hijo.stdout.take().expect("stdout piped");
                let stderr = hijo.stderr.take().expect("stderr piped");

                // stderr se lee en su propio hilo para que ninguno de los dos búferes se llene y bloquee al hijo.
                let tx_err = tx.clone();
                let ctx_err = ctx.clone();
                let hilo_err = std::thread::spawn(move || {
                    for linea in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let _ = tx_err.send(Msg::Error(linea));
                        ctx_err.request_repaint();
                    }
                });

                for linea in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = tx.send(Msg::Salida(linea));
                    ctx.request_repaint();
                }
                let _ = hilo_err.join();
                let codigo = hijo.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                let _ = tx.send(Msg::Fin(codigo));
            }
            Err(e) => {
                let _ = tx.send(Msg::Error(format!("No se pudo lanzar PowerShell: {e}")));
                let _ = tx.send(Msg::Fin(-1));
            }
        }
        ctx.request_repaint();
    }
}
