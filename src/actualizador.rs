// Auto-actualizador estilo VS Code.
//
// Al arrancar (solo en compilaciones release) consulta la última versión
// publicada en GitHub Releases y, si es más nueva que la que corre, muestra un
// aviso para descargarla y reiniciar. El binario se reemplaza a sí mismo con el
// truco clásico de Windows: no se puede sobrescribir un .exe en ejecución, pero
// sí renombrarlo (el que corre pasa a «fulgur-old.exe» y el nuevo ocupa su sitio).
//
// Toda la red va en hilos aparte para no congelar la UI.

use eframe::egui;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

const REPO: &str = "AndresDeC/Fulgur-project";
/// Sufijo del asset de Windows en cada release.
const SUFIJO_ASSET: &str = "windows-x64.exe";

#[derive(Clone)]
pub enum Estado {
    /// Nada que mostrar (al día, sin comprobar o aviso descartado).
    Oculto,
    Comprobando,
    /// Hay una versión nueva disponible para descargar.
    Disponible { version: String, url: String },
    Descargando,
    /// Nuevo binario ya colocado; falta reiniciar para aplicarlo.
    Listo,
    Error(String),
}

enum Msg {
    Estado(Estado),
}

pub struct Actualizador {
    estado: Estado,
    rx: Option<Receiver<Msg>>,
    /// Ruta del ejecutable que corre (para relanzarlo tras actualizar).
    exe: Option<PathBuf>,
}

impl Default for Actualizador {
    fn default() -> Self {
        // Al arrancar, limpiar el «fulgur-old.exe» que dejó una actualización previa.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let _ = std::fs::remove_file(dir.join("fulgur-old.exe"));
            }
        }
        Self {
            estado: Estado::Oculto,
            rx: None,
            exe: std::env::current_exe().ok(),
        }
    }
}

impl Actualizador {
    /// Lanza una comprobación de versión en segundo plano.
    pub fn comprobar(&mut self, ctx: &egui::Context) {
        if matches!(self.estado, Estado::Comprobando | Estado::Descargando) {
            return;
        }
        self.estado = Estado::Comprobando;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Estado(consultar()));
            ctx.request_repaint();
        });
    }

    /// Descarga la versión nueva y la deja instalada (pendiente de reinicio).
    fn instalar(&mut self, url: String, ctx: &egui::Context) {
        let Some(exe) = self.exe.clone() else {
            self.estado = Estado::Error("No sé dónde está el ejecutable".to_owned());
            return;
        };
        self.estado = Estado::Descargando;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Estado(descargar_y_reemplazar(&url, &exe)));
            ctx.request_repaint();
        });
    }

    /// Recoge el resultado de los hilos. Se llama una vez por fotograma.
    pub fn recibir(&mut self) {
        let Some(rx) = &self.rx else { return };
        if let Ok(Msg::Estado(estado)) = rx.try_recv() {
            self.estado = estado;
            self.rx = None;
        }
    }

    /// ¿Hay algo que mostrar en la barra de aviso?
    pub fn hay_aviso(&self) -> bool {
        !matches!(self.estado, Estado::Oculto | Estado::Comprobando)
    }

    /// Dibuja la barra de aviso (estilo notificación de VS Code) y gestiona los
    /// botones. Se llama dentro de un panel propio.
    pub fn ui_barra(&mut self, ui: &mut egui::Ui) {
        let p = crate::tema::p();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            match self.estado.clone() {
                Estado::Disponible { version, url } => {
                    ui.label(
                        egui::RichText::new(format!("⬆ Versión {version} disponible"))
                            .color(p.acento)
                            .strong(),
                    );
                    if ui.button("Actualizar").clicked() {
                        let ctx = ui.ctx().clone();
                        self.instalar(url, &ctx);
                    }
                    if ui.button("Ahora no").clicked() {
                        self.estado = Estado::Oculto;
                    }
                }
                Estado::Descargando => {
                    ui.spinner();
                    ui.label(egui::RichText::new("Descargando actualización…").color(p.texto));
                }
                Estado::Listo => {
                    ui.label(
                        egui::RichText::new("Actualización lista — reinicia para aplicarla")
                            .color(egui::Color32::from_rgb(0x8a, 0xe0, 0x9a))
                            .strong(),
                    );
                    if ui.button("Reiniciar ahora").clicked() {
                        self.reiniciar(ui.ctx());
                    }
                    if ui.button("Al cerrar").clicked() {
                        self.estado = Estado::Oculto;
                    }
                }
                Estado::Error(e) => {
                    ui.label(
                        egui::RichText::new(format!("⚠ No se pudo actualizar: {e}"))
                            .color(egui::Color32::from_rgb(0xff, 0x8a, 0x8a)),
                    );
                    if ui.button("Cerrar").clicked() {
                        self.estado = Estado::Oculto;
                    }
                }
                Estado::Oculto | Estado::Comprobando => {}
            }
        });
    }

    /// Relanza el nuevo ejecutable y cierra el actual.
    fn reiniciar(&mut self, ctx: &egui::Context) {
        if let Some(exe) = &self.exe {
            let _ = std::process::Command::new(exe).spawn();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// Consulta la API de GitHub por la última release versionada (las `latest`
/// rolling son prerelease y la API las ignora en este endpoint).
fn consultar() -> Estado {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let respuesta = ureq::get(&url)
        .set("User-Agent", "Fulgur-Updater")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .call();
    let json: serde_json::Value = match respuesta {
        Ok(r) => match r.into_json() {
            Ok(j) => j,
            Err(e) => return Estado::Error(format!("respuesta inválida: {e}")),
        },
        Err(e) => return Estado::Error(format!("sin conexión con GitHub: {e}")),
    };

    let tag = json["tag_name"].as_str().unwrap_or("");
    if !es_mas_nueva(tag, env!("CARGO_PKG_VERSION")) {
        return Estado::Oculto; // ya estamos al día.
    }
    match json["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| nombre_asset_valido(x)))
        .and_then(|x| x["browser_download_url"].as_str())
    {
        Some(url) => Estado::Disponible {
            version: tag.to_owned(),
            url: url.to_owned(),
        },
        None => Estado::Error("la versión nueva no trae binario de Windows".to_owned()),
    }
}

fn nombre_asset_valido(asset: &serde_json::Value) -> bool {
    asset["name"]
        .as_str()
        .is_some_and(|n| n.ends_with(SUFIJO_ASSET))
}

/// Descarga el nuevo .exe y lo pone en el sitio del actual (que pasa a .old).
fn descargar_y_reemplazar(url: &str, exe: &std::path::Path) -> Estado {
    let dir = match exe.parent() {
        Some(d) => d.to_path_buf(),
        None => return Estado::Error("ruta del ejecutable inválida".to_owned()),
    };
    let nuevo = dir.join("fulgur-nuevo.exe");
    let viejo = dir.join("fulgur-old.exe");

    // Descargar a un archivo temporal.
    let resultado = (|| -> std::io::Result<()> {
        let resp = ureq::get(url)
            .set("User-Agent", "Fulgur-Updater")
            .timeout(std::time::Duration::from_secs(600))
            .call()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut lector = resp.into_reader();
        let mut archivo = std::fs::File::create(&nuevo)?;
        let mut buf = Vec::new();
        lector.read_to_end(&mut buf)?;
        std::io::Write::write_all(&mut archivo, &buf)?;
        Ok(())
    })();
    if let Err(e) = resultado {
        let _ = std::fs::remove_file(&nuevo);
        return Estado::Error(format!("descarga fallida: {e}"));
    }

    // Intercambio atómico por renombrados (permitido con el .exe en ejecución).
    let _ = std::fs::remove_file(&viejo);
    if let Err(e) = std::fs::rename(exe, &viejo) {
        let _ = std::fs::remove_file(&nuevo);
        return Estado::Error(format!("no se pudo apartar el ejecutable actual: {e}"));
    }
    if let Err(e) = std::fs::rename(&nuevo, exe) {
        // Revertir para no dejar la app sin ejecutable.
        let _ = std::fs::rename(&viejo, exe);
        return Estado::Error(format!("no se pudo colocar el nuevo ejecutable: {e}"));
    }
    Estado::Listo
}

/// Convierte "v1.2.3" o "1.2.3" en (1, 2, 3) para comparar.
fn version_tupla(s: &str) -> (u32, u32, u32) {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.split('.').map(|x| x.trim().parse().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// ¿La versión `remota` es más nueva que la `actual`?
fn es_mas_nueva(remota: &str, actual: &str) -> bool {
    version_tupla(remota) > version_tupla(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compara_versiones() {
        assert!(es_mas_nueva("v0.3.1", "0.3.0"));
        assert!(es_mas_nueva("v1.0.0", "0.9.9"));
        assert!(es_mas_nueva("0.4.0", "0.3.9"));
        assert!(!es_mas_nueva("v0.3.0", "0.3.0")); // igual: no es más nueva.
        assert!(!es_mas_nueva("v0.2.9", "0.3.0")); // más vieja.
    }

    #[test]
    fn parsea_con_y_sin_v() {
        assert_eq!(version_tupla("v1.2.3"), (1, 2, 3));
        assert_eq!(version_tupla("1.2.3"), (1, 2, 3));
        assert_eq!(version_tupla("v2.0"), (2, 0, 0));
    }

    // Consulta real a la API de GitHub: no debe fallar (red + JSON + forma de la
    // respuesta). Marcada #[ignore] porque necesita internet.
    //   cargo test --bin fulgur -- --ignored consulta_real
    #[test]
    #[ignore]
    fn consulta_real_no_falla() {
        assert!(
            !matches!(consultar(), Estado::Error(_)),
            "la consulta a GitHub devolvió error"
        );
    }

    // Descarga real del asset de v0.3.0 y prueba el intercambio de ficheros sobre
    // un ejecutable de mentira. Lenta (baja ~43 MB), por eso #[ignore]:
    //   cargo test --bin fulgur -- --ignored descarga_y_reemplazo
    #[test]
    #[ignore]
    fn descarga_y_reemplazo_real() {
        let dir = std::env::temp_dir().join(format!("fulgur_upd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe_falso = dir.join("fulgur.exe");
        std::fs::write(&exe_falso, b"soy el binario viejo").unwrap();

        let url = "https://github.com/AndresDeC/Fulgur-project/releases/download/v0.3.0/fulgur-v0.3.0-windows-x64.exe";
        let estado = descargar_y_reemplazar(url, &exe_falso);

        assert!(matches!(estado, Estado::Listo), "no quedó Listo");
        // El exe ahora pesa lo que el binario real (mucho más que el placeholder).
        let tam = std::fs::metadata(&exe_falso).unwrap().len();
        assert!(tam > 1_000_000, "el exe no se reemplazó (tam={tam})");
        // Y se guardó la copia de seguridad del viejo.
        assert!(dir.join("fulgur-old.exe").exists(), "falta el .old");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reconoce_el_asset_de_windows() {
        let a = serde_json::json!({ "name": "fulgur-v0.3.1-windows-x64.exe" });
        assert!(nombre_asset_valido(&a));
        let b = serde_json::json!({ "name": "fuente.tar.gz" });
        assert!(!nombre_asset_valido(&b));
    }
}
