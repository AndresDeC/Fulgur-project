// Mercado de extensiones descentralizado.
//
// No hay un servidor central: el usuario añade "fuentes" (la URL o ruta local
// de un `indice.json`) y Fulgur lee de todas ellas. Cualquiera puede publicar
// un registro propio.
//
// Seguridad:
//  - cada entrada del índice declara el SHA-256 del paquete; si al descargarlo
//    no coincide, la instalación se rechaza.
//  - las extensiones son datos declarativos (JSON: temas, snippets…),
//    nunca código que se ejecute.

use eframe::egui;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Una extensión anunciada en el índice de una fuente.
#[derive(Clone, Serialize, Deserialize)]
pub struct EntradaIndice {
    pub id: String,
    pub nombre: String,
    pub version: String,
    #[serde(default)]
    pub descripcion: String,
    #[serde(default)]
    pub autor: String,
    /// "tema" o "snippets".
    pub tipo: String,
    /// URL absoluta, o ruta relativa al propio índice.
    pub url: String,
    /// SHA-256 (hex) del paquete. Obligatorio: sin él no se instala.
    pub sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Indice {
    pub nombre: String,
    pub extensiones: Vec<EntradaIndice>,
}

/// El paquete instalado: el JSON completo de la extensión.
#[derive(Clone, Serialize, Deserialize)]
pub struct Extension {
    pub id: String,
    pub nombre: String,
    pub version: String,
    pub tipo: String,
    #[serde(default)]
    pub datos: serde_json::Value,
}

/// Colores de una extensión de tipo "tema".
#[derive(Deserialize)]
struct DatosTema {
    oscuro: bool,
    fondo: [u8; 3],
    panel: [u8; 3],
    texto: [u8; 3],
    acento: [u8; 3],
}

#[derive(Deserialize)]
struct Snippet {
    atajo: String,
    #[serde(default)]
    descripcion: String,
    cuerpo: String,
}

/// Configuración que persiste en ~/.fulgur/config.json.
#[derive(Default, Serialize, Deserialize)]
struct Config {
    fuentes: Vec<String>,
    tema_aplicado: Option<String>,
}

enum Msg {
    Indice(String, Result<Indice, String>),
    Instalada(Result<Extension, String>),
}

pub struct Mercado {
    pub fuentes: Vec<String>,
    nueva_fuente: String,
    /// Índices ya descargados: (fuente, índice).
    indices: Vec<(String, Indice)>,
    pub instaladas: Vec<Extension>,
    pub tema_aplicado: Option<String>,
    /// Nº de descargas en curso.
    ocupado: usize,
    pub estado: String,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

/// Carpeta de datos del usuario: ~/.fulgur
fn dir_fulgur() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fulgur")
}

fn dir_extensiones() -> PathBuf {
    dir_fulgur().join("extensiones")
}

fn ruta_config() -> PathBuf {
    dir_fulgur().join("config.json")
}

impl Default for Mercado {
    fn default() -> Self {
        let (tx, rx) = channel();

        // Carga la configuración y las extensiones ya instaladas.
        let config: Config = std::fs::read_to_string(ruta_config())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let mut instaladas = Vec::new();
        if let Ok(entradas) = std::fs::read_dir(dir_extensiones()) {
            for entrada in entradas.flatten() {
                if let Ok(texto) = std::fs::read_to_string(entrada.path()) {
                    if let Ok(ext) = serde_json::from_str::<Extension>(&texto) {
                        instaladas.push(ext);
                    }
                }
            }
        }

        Self {
            fuentes: config.fuentes,
            nueva_fuente: String::new(),
            indices: Vec::new(),
            instaladas,
            tema_aplicado: config.tema_aplicado,
            ocupado: 0,
            estado: String::new(),
            tx,
            rx,
        }
    }
}

impl Mercado {
    fn guardar_config(&self) {
        let _ = std::fs::create_dir_all(dir_fulgur());
        let config = Config {
            fuentes: self.fuentes.clone(),
            tema_aplicado: self.tema_aplicado.clone(),
        };
        if let Ok(texto) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(ruta_config(), texto);
        }
    }

    /// Lee (o descarga) el contenido apuntado por `url`, que puede ser
    /// absoluta (http/https), o relativa a la fuente `base`.
    fn obtener(base: &str, url: &str) -> Result<Vec<u8>, String> {
        let destino = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_owned()
        } else if base.starts_with("http://") || base.starts_with("https://") {
            // URL relativa al índice remoto: se sustituye el último segmento.
            match base.rfind('/') {
                Some(i) => format!("{}/{}", &base[..i], url),
                None => url.to_owned(),
            }
        } else {
            // Fuente local: ruta relativa al archivo de índice.
            let ruta = Path::new(base)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(url);
            return std::fs::read(&ruta)
                .map_err(|e| format!("No se pudo leer {}: {e}", ruta.display()));
        };

        if destino.starts_with("http://") || destino.starts_with("https://") {
            let respuesta = ureq::get(&destino)
                .timeout(std::time::Duration::from_secs(15))
                .call()
                .map_err(|e| format!("Error de red en {destino}: {e}"))?;
            let mut bytes = Vec::new();
            use std::io::Read;
            respuesta
                .into_reader()
                .take(20 * 1024 * 1024) // límite de 20 MB por paquete
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Error leyendo {destino}: {e}"))?;
            Ok(bytes)
        } else {
            std::fs::read(&destino).map_err(|e| format!("No se pudo leer {destino}: {e}"))
        }
    }

    /// Descarga los índices de todas las fuentes en hilos aparte.
    fn actualizar_indices(&mut self, ctx: &egui::Context) {
        self.indices.clear();
        for fuente in self.fuentes.clone() {
            self.ocupado += 1;
            let tx = self.tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let resultado = Self::obtener(&fuente, &fuente).and_then(|bytes| {
                    serde_json::from_slice::<Indice>(&bytes)
                        .map_err(|e| format!("Índice inválido en {fuente}: {e}"))
                });
                let _ = tx.send(Msg::Indice(fuente, resultado));
                ctx.request_repaint();
            });
        }
    }

    /// Descarga un paquete, verifica su SHA-256 y lo instala.
    fn instalar(&mut self, fuente: String, entrada: EntradaIndice, ctx: &egui::Context) {
        self.ocupado += 1;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let resultado = (|| {
                let bytes = Self::obtener(&fuente, &entrada.url)?;

                // --- Verificación de integridad ---
                let hash = hex(&Sha256::digest(&bytes));
                if !hash.eq_ignore_ascii_case(entrada.sha256.trim()) {
                    return Err(format!(
                        "¡SHA-256 no coincide para «{}»! El paquete pudo ser manipulado. Instalación rechazada.\n  esperado: {}\n  recibido: {hash}",
                        entrada.nombre, entrada.sha256
                    ));
                }

                let ext: Extension = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Paquete inválido: {e}"))?;
                let _ = std::fs::create_dir_all(dir_extensiones());
                let destino = dir_extensiones().join(format!("{}.json", ext.id));
                std::fs::write(&destino, &bytes)
                    .map_err(|e| format!("No se pudo guardar la extensión: {e}"))?;
                Ok(ext)
            })();
            let _ = tx.send(Msg::Instalada(resultado));
            ctx.request_repaint();
        });
    }

    fn desinstalar(&mut self, id: &str, ctx: &egui::Context) {
        let _ = std::fs::remove_file(dir_extensiones().join(format!("{id}.json")));
        self.instaladas.retain(|e| e.id != id);
        if self.tema_aplicado.as_deref() == Some(id) {
            self.tema_aplicado = None;
            ctx.set_visuals(egui::Visuals::dark());
        }
        self.guardar_config();
        self.estado = format!("Extensión «{id}» desinstalada.");
    }

    fn recibir_mensajes(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.ocupado = self.ocupado.saturating_sub(1);
            match msg {
                Msg::Indice(fuente, Ok(indice)) => {
                    self.estado = format!(
                        "Fuente «{}»: {} extensiones.",
                        indice.nombre,
                        indice.extensiones.len()
                    );
                    self.indices.push((fuente, indice));
                }
                Msg::Indice(_, Err(e)) | Msg::Instalada(Err(e)) => self.estado = e,
                Msg::Instalada(Ok(ext)) => {
                    self.estado = format!("«{}» v{} instalada ✔", ext.nombre, ext.version);
                    self.instaladas.retain(|e| e.id != ext.id);
                    self.instaladas.push(ext);
                }
            }
        }
    }

    /// Devuelve `Some(texto)` si el usuario pidió insertar un snippet en el editor.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<String> {
        self.recibir_mensajes();
        let mut insertar: Option<String> = None;

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("EXTENSIONES")
                    .size(11.0)
                    .color(crate::tema::p().texto_tenue),
            );
            if self.ocupado > 0 {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄").on_hover_text("Actualizar todas las fuentes").clicked() {
                    let ctx = ui.ctx().clone();
                    self.actualizar_indices(&ctx);
                }
            });
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("mercado")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // --- Fuentes (registros descentralizados) ---
                ui.label(egui::RichText::new("Fuentes").strong());
                ui.weak("URL o ruta local de un indice.json. Cualquiera puede publicar el suyo.");
                let mut quitar: Option<usize> = None;
                for (i, fuente) in self.fuentes.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button("🗑").clicked() {
                            quitar = Some(i);
                        }
                        ui.small(fuente);
                    });
                }
                if let Some(i) = quitar {
                    self.fuentes.remove(i);
                    self.guardar_config();
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.nueva_fuente)
                            .hint_text("https://… o C:\\ruta\\indice.json")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    if ui.button("Añadir").clicked() && !self.nueva_fuente.trim().is_empty() {
                        self.fuentes.push(self.nueva_fuente.trim().to_owned());
                        self.nueva_fuente.clear();
                        self.guardar_config();
                        let ctx = ui.ctx().clone();
                        self.actualizar_indices(&ctx);
                    }
                });

                ui.add_space(8.0);
                ui.separator();

                // --- Catálogo de las fuentes ---
                ui.label(egui::RichText::new("Disponibles").strong());
                if self.indices.is_empty() {
                    ui.weak("Añade una fuente y pulsa 🔄 para ver su catálogo.");
                }
                let mut instalar_pedido: Option<(String, EntradaIndice)> = None;
                for (fuente, indice) in &self.indices {
                    ui.label(format!("📦 {}", indice.nombre));
                    for entrada in &indice.extensiones {
                        let ya = self.instaladas.iter().any(|e| e.id == entrada.id);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(icono_tipo(&entrada.tipo));
                            ui.vertical(|ui| {
                                ui.label(format!("{} v{} — {}", entrada.nombre, entrada.version, entrada.autor));
                                if !entrada.descripcion.is_empty() {
                                    ui.weak(&entrada.descripcion);
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ya {
                                        ui.weak("instalada ✔");
                                    } else if ui.button("Instalar").clicked() {
                                        instalar_pedido =
                                            Some((fuente.clone(), entrada.clone()));
                                    }
                                },
                            );
                        });
                    }
                }
                if let Some((fuente, entrada)) = instalar_pedido {
                    let ctx = ui.ctx().clone();
                    self.instalar(fuente, entrada, &ctx);
                }

                ui.add_space(8.0);
                ui.separator();

                // --- Instaladas ---
                ui.label(egui::RichText::new("Instaladas").strong());
                if self.instaladas.is_empty() {
                    ui.weak("Todavía no hay extensiones instaladas.");
                }
                let mut desinstalar_pedido: Option<String> = None;
                let mut aplicar_pedido: Option<usize> = None;
                for (i, ext) in self.instaladas.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(icono_tipo(&ext.tipo));
                        ui.label(format!("{} v{}", ext.nombre, ext.version));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").on_hover_text("Desinstalar").clicked() {
                                desinstalar_pedido = Some(ext.id.clone());
                            }
                            if ext.tipo == "tema" {
                                let activo = self.tema_aplicado.as_deref() == Some(&ext.id);
                                if activo {
                                    ui.weak("activo");
                                } else if ui.small_button("Aplicar").clicked() {
                                    aplicar_pedido = Some(i);
                                }
                            }
                        });
                    });
                    // Los snippets se listan con su botón de inserción.
                    if ext.tipo == "snippets" {
                        if let Ok(snippets) = serde_json::from_value::<Vec<Snippet>>(
                            ext.datos.get("snippets").cloned().unwrap_or_default(),
                        ) {
                            for s in snippets {
                                ui.horizontal(|ui| {
                                    ui.add_space(18.0);
                                    if ui.small_button("Insertar").clicked() {
                                        insertar = Some(s.cuerpo.clone());
                                    }
                                    ui.monospace(&s.atajo);
                                    ui.weak(&s.descripcion);
                                });
                            }
                        }
                    }
                }
                if let Some(id) = desinstalar_pedido {
                    let ctx = ui.ctx().clone();
                    self.desinstalar(&id, &ctx);
                }
                if let Some(i) = aplicar_pedido {
                    let ext = self.instaladas[i].clone();
                    aplicar_tema(ui.ctx(), &ext);
                    self.tema_aplicado = Some(ext.id.clone());
                    self.guardar_config();
                    self.estado = format!("Tema «{}» aplicado.", ext.nombre);
                }

                if !self.estado.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.weak(&self.estado);
                }
            });

        insertar
    }
}

fn icono_tipo(tipo: &str) -> &'static str {
    match tipo {
        "tema" => "🎨",
        "snippets" => "✂",
        _ => "🧩",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Convierte los datos de una extensión "tema" en colores reales de egui.
fn aplicar_tema(ctx: &egui::Context, ext: &Extension) {
    let Ok(tema) = serde_json::from_value::<DatosTema>(ext.datos.clone()) else {
        return;
    };
    let c = |rgb: [u8; 3]| egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    let mut visuales = if tema.oscuro {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuales.panel_fill = c(tema.panel);
    visuales.window_fill = c(tema.panel);
    visuales.extreme_bg_color = c(tema.fondo);
    visuales.override_text_color = Some(c(tema.texto));
    visuales.selection.bg_fill = c(tema.acento).gamma_multiply(0.55);
    visuales.hyperlink_color = c(tema.acento);
    ctx.set_visuals(visuales);
}
