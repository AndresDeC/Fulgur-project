// Asistente de IA integrado.
//
// Dos proveedores:
//  - Ollama:    IAs open source corriendo en tu máquina (gratis, sin clave).
//  - Anthropic: Claude vía API (de pago, requiere clave).
//
// Las peticiones se hacen en un hilo aparte para no congelar la UI.

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Proveedor {
    Ollama,
    Anthropic,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigIa {
    pub proveedor: Proveedor,
    pub url_ollama: String,
    pub modelo_ollama: String,
    pub clave_anthropic: String,
    pub modelo_anthropic: String,
}

impl Default for ConfigIa {
    fn default() -> Self {
        Self {
            proveedor: Proveedor::Ollama,
            url_ollama: "http://localhost:11434".to_owned(),
            modelo_ollama: "llama3.2".to_owned(),
            clave_anthropic: String::new(),
            modelo_anthropic: "claude-opus-4-8".to_owned(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Rol {
    Usuario,
    Asistente,
}

pub struct MensajeChat {
    pub rol: Rol,
    pub texto: String,
}

enum Msg {
    Respuesta(String),
    Error(String),
}

pub struct Ia {
    pub config: ConfigIa,
    pub mensajes: Vec<MensajeChat>,
    pub entrada: String,
    pub pensando: bool,
    pub mostrar_config: bool,
    /// Si está activo, se envía el archivo abierto en el editor como contexto.
    pub incluir_archivo: bool,
    rx: Option<Receiver<Msg>>,
}

fn ruta_config_ia() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fulgur")
        .join("ia.json")
}

impl Default for Ia {
    fn default() -> Self {
        let config: ConfigIa = std::fs::read_to_string(ruta_config_ia())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            config,
            mensajes: Vec::new(),
            entrada: String::new(),
            pensando: false,
            mostrar_config: false,
            incluir_archivo: false,
            rx: None,
        }
    }
}

impl Ia {
    fn guardar_config(&self) {
        let _ = std::fs::create_dir_all(ruta_config_ia().parent().unwrap());
        if let Ok(texto) = serde_json::to_string_pretty(&self.config) {
            let _ = std::fs::write(ruta_config_ia(), texto);
        }
    }

    /// `contexto` es el archivo activo del editor: (nombre, contenido).
    pub fn ui(&mut self, ui: &mut egui::Ui, contexto: Option<(String, String)>) {
        self.recibir_mensajes();

        // Barra superior del panel.
        ui.horizontal(|ui| {
            ui.label("🤖");
            let nombre = match self.config.proveedor {
                Proveedor::Ollama => format!("Ollama · {}", self.config.modelo_ollama),
                Proveedor::Anthropic => format!("Claude · {}", self.config.modelo_anthropic),
            };
            ui.label(egui::RichText::new(nombre).strong());
            ui.checkbox(&mut self.incluir_archivo, "incluir archivo activo")
                .on_hover_text("Envía el contenido de la pestaña abierta como contexto");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Configurar proveedor").clicked() {
                    self.mostrar_config = !self.mostrar_config;
                }
                if ui.button("🗑").on_hover_text("Borrar conversación").clicked() {
                    self.mensajes.clear();
                }
            });
        });

        if self.mostrar_config {
            self.ui_config(ui);
        }
        ui.separator();

        // Historial del chat.
        let alto_entrada = 30.0;
        egui::ScrollArea::vertical()
            .id_salt("chat_ia")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height(ui.available_height() - alto_entrada)
            .show(ui, |ui| {
                if self.mensajes.is_empty() {
                    ui.add_space(8.0);
                    ui.weak("Pregúntale algo a la IA sobre tu código. Configura el proveedor con ⚙.");
                }
                for mensaje in &self.mensajes {
                    let (etiqueta, color) = match mensaje.rol {
                        Rol::Usuario => ("Tú", egui::Color32::from_rgb(120, 170, 255)),
                        Rol::Asistente => ("IA", egui::Color32::from_rgb(140, 220, 160)),
                    };
                    ui.label(egui::RichText::new(etiqueta).strong().color(color));
                    ui.label(&mensaje.texto);
                    ui.add_space(6.0);
                }
                if self.pensando {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("pensando…");
                    });
                }
            });

        // Entrada del usuario.
        ui.horizontal(|ui| {
            let respuesta = ui.add_sized(
                egui::vec2(ui.available_width() - 70.0, 22.0),
                egui::TextEdit::singleline(&mut self.entrada).hint_text("Pregunta a la IA…"),
            );
            let enviar_click = ui.button("Enviar").clicked();
            let enviar_enter =
                respuesta.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (enviar_click || enviar_enter) && !self.pensando {
                self.enviar(ui.ctx().clone(), contexto);
                respuesta.request_focus();
            }
        });
    }

    fn ui_config(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Proveedor:");
            if ui
                .selectable_label(self.config.proveedor == Proveedor::Ollama, "Ollama (local)")
                .clicked()
            {
                self.config.proveedor = Proveedor::Ollama;
            }
            if ui
                .selectable_label(
                    self.config.proveedor == Proveedor::Anthropic,
                    "Anthropic (Claude)",
                )
                .clicked()
            {
                self.config.proveedor = Proveedor::Anthropic;
            }
        });
        match self.config.proveedor {
            Proveedor::Ollama => {
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut self.config.url_ollama);
                });
                ui.horizontal(|ui| {
                    ui.label("Modelo:");
                    ui.text_edit_singleline(&mut self.config.modelo_ollama);
                    ui.weak("(p. ej. llama3.2, qwen2.5-coder)");
                });
            }
            Proveedor::Anthropic => {
                ui.horizontal(|ui| {
                    ui.label("Clave API:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.clave_anthropic)
                            .password(true)
                            .hint_text("sk-ant-…"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Modelo:");
                    ui.text_edit_singleline(&mut self.config.modelo_anthropic);
                });
                ui.weak("La clave se guarda en ~/.fulgur/ia.json — no la compartas.");
            }
        }
        if ui.button("Guardar configuración").clicked() {
            self.guardar_config();
            self.mostrar_config = false;
        }
    }

    fn recibir_mensajes(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Respuesta(texto) => self.mensajes.push(MensajeChat {
                    rol: Rol::Asistente,
                    texto,
                }),
                Msg::Error(e) => self.mensajes.push(MensajeChat {
                    rol: Rol::Asistente,
                    texto: format!("⚠ {e}"),
                }),
            }
            self.pensando = false;
        }
    }

    fn enviar(&mut self, ctx: egui::Context, contexto: Option<(String, String)>) {
        let pregunta = self.entrada.trim().to_owned();
        if pregunta.is_empty() {
            return;
        }
        self.entrada.clear();

        // El primer mensaje puede llevar el archivo activo como contexto.
        let texto_envio = match (&contexto, self.incluir_archivo) {
            (Some((nombre, contenido)), true) => format!(
                "Contexto — archivo abierto «{nombre}»:\n```\n{contenido}\n```\n\nPregunta: {pregunta}"
            ),
            _ => pregunta.clone(),
        };
        self.mensajes.push(MensajeChat {
            rol: Rol::Usuario,
            texto: pregunta,
        });
        self.pensando = true;

        // Se envía el historial completo (con el contexto solo en el último turno).
        let mut historial: Vec<(bool, String)> = self.mensajes
            [..self.mensajes.len() - 1]
            .iter()
            .map(|m| (m.rol == Rol::Usuario, m.texto.clone()))
            .collect();
        historial.push((true, texto_envio));

        let config = self.config.clone();
        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let resultado = match config.proveedor {
                Proveedor::Ollama => pedir_ollama(&config, &historial),
                Proveedor::Anthropic => pedir_anthropic(&config, &historial),
            };
            let _ = tx.send(match resultado {
                Ok(texto) => Msg::Respuesta(texto),
                Err(e) => Msg::Error(e),
            });
            ctx.request_repaint();
        });
    }
}

/// Convierte el historial a los mensajes JSON que ambas APIs entienden.
fn mensajes_json(historial: &[(bool, String)]) -> Vec<serde_json::Value> {
    historial
        .iter()
        .map(|(es_usuario, texto)| {
            serde_json::json!({
                "role": if *es_usuario { "user" } else { "assistant" },
                "content": texto,
            })
        })
        .collect()
}

const PROMPT_SISTEMA: &str = "Eres el asistente de IA integrado en Fulgur, un IDE ligero. \
Ayudas con código: explicas, encuentras errores y propones mejoras. Responde en el idioma del usuario, \
de forma clara y concisa.";

/// API local de Ollama: POST /api/chat (sin streaming).
fn pedir_ollama(config: &ConfigIa, historial: &[(bool, String)]) -> Result<String, String> {
    let url = format!("{}/api/chat", config.url_ollama.trim_end_matches('/'));
    let mut mensajes = vec![serde_json::json!({"role": "system", "content": PROMPT_SISTEMA})];
    mensajes.extend(mensajes_json(historial));
    let cuerpo = serde_json::json!({
        "model": config.modelo_ollama,
        "messages": mensajes,
        "stream": false,
    });

    let respuesta = ureq::post(&url)
        .timeout(std::time::Duration::from_secs(300))
        .send_json(cuerpo)
        .map_err(|e| formatear_error_http("Ollama", e))?;
    let json: serde_json::Value = respuesta
        .into_json()
        .map_err(|e| format!("Respuesta inválida de Ollama: {e}"))?;
    json["message"]["content"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("Respuesta inesperada de Ollama: {json}"))
}

/// API de Anthropic: POST https://api.anthropic.com/v1/messages (sin streaming).
fn pedir_anthropic(config: &ConfigIa, historial: &[(bool, String)]) -> Result<String, String> {
    if config.clave_anthropic.trim().is_empty() {
        return Err("Falta la clave API de Anthropic. Configúrala con el botón ⚙.".to_owned());
    }
    let cuerpo = serde_json::json!({
        "model": config.modelo_anthropic,
        "max_tokens": 8192,
        "system": PROMPT_SISTEMA,
        "messages": mensajes_json(historial),
    });

    let respuesta = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", config.clave_anthropic.trim())
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(300))
        .send_json(cuerpo)
        .map_err(|e| formatear_error_http("Anthropic", e))?;
    let json: serde_json::Value = respuesta
        .into_json()
        .map_err(|e| format!("Respuesta inválida de Anthropic: {e}"))?;

    // La respuesta trae bloques de contenido; concatenamos los de texto.
    let bloques = json["content"]
        .as_array()
        .ok_or_else(|| format!("Respuesta inesperada de Anthropic: {json}"))?;
    let texto: String = bloques
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if texto.is_empty() && json["stop_reason"] == "refusal" {
        return Err("Claude rechazó la petición por motivos de seguridad.".to_owned());
    }
    Ok(texto)
}

fn formatear_error_http(proveedor: &str, e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(codigo, resp) => {
            let detalle = resp.into_string().unwrap_or_default();
            format!("{proveedor} devolvió HTTP {codigo}: {detalle}")
        }
        ureq::Error::Transport(t) => format!(
            "No se pudo conectar con {proveedor}: {t}. ¿Está el servicio disponible?"
        ),
    }
}
