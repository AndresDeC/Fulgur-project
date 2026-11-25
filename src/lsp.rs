// Cliente LSP (Language Server Protocol) mínimo.
//
// Habla con servidores de lenguaje externos (rust-analyzer, gopls…) por JSON-RPC
// sobre stdin/stdout. A diferencia de tree-sitter —que solo ve la sintaxis— el
// LSP entiende el proyecto entero: tipos, imports, el borrow checker… así que
// da errores y avisos *semánticos*, los mismos que verías al compilar.
//
// El servidor corre en su propio proceso (su RAM es aparte y solo cuando editas
// ese lenguaje). Toda la E/S va en hilos para no congelar la UI; los diagnósticos
// que el servidor empuja llegan por un canal y se recogen cada fotograma.

use crate::diagnostico::{Diagnostico, Severidad};
use eframe::egui;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Servidor de lenguaje para un código de Fulgur: (nombre, comando ejecutable).
/// Ampliar = añadir un brazo (y tener el servidor instalado en el PATH).
pub fn servidor_para(lenguaje: &str) -> Option<(&'static str, &'static str)> {
    match lenguaje {
        "rs" => Some(("rust-analyzer", "rust-analyzer")),
        "go" => Some(("gopls", "gopls")),
        _ => None,
    }
}

/// El `languageId` que espera el LSP para cada lenguaje.
fn lenguaje_lsp(lenguaje: &str) -> &'static str {
    match lenguaje {
        "rs" => "rust",
        "go" => "go",
        _ => "plaintext",
    }
}

/// Un diagnóstico tal como lo manda el LSP: posiciones en (línea, carácter) con
/// la codificación negociada. Se traduce a `Diagnostico` al mostrarlo, cuando ya
/// tenemos el texto del archivo a mano.
struct DiagLsp {
    l0: usize,
    c0: usize,
    l1: usize,
    c1: usize,
    severidad: Severidad,
    mensaje: String,
}

/// Mensaje del hilo lector hacia la UI.
enum Entrante {
    Diagnosticos { clave: String, diags: Vec<DiagLsp> },
}

/// Un servidor de lenguaje en marcha.
pub struct ClienteLsp {
    _hijo: Child,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Entrante>,
    /// true si la codificación de posiciones negociada es UTF-16 (el defecto LSP).
    utf16: bool,
    /// Documentos abiertos: clave normalizada → versión actual.
    abiertos: HashMap<String, i64>,
    /// Último lote de diagnósticos por documento.
    diagnosticos: HashMap<String, Vec<DiagLsp>>,
}

impl ClienteLsp {
    /// Lanza el servidor en `raiz` y completa el handshake. `None` si el binario
    /// no está instalado o el handshake falla.
    pub fn iniciar(comando: &str, raiz: &Path, ctx: egui::Context) -> Option<Self> {
        let mut orden = Command::new(comando);
        orden
            .current_dir(raiz)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            orden.creation_flags(CREATE_NO_WINDOW);
        }
        let mut hijo = orden.spawn().ok()?;
        let stdin = hijo.stdin.take()?;
        let stdout = hijo.stdout.take()?;

        // Hilo escritor: serializa el acceso a stdin del servidor.
        let (tx, rx_salida) = channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut w = stdin;
            while let Ok(bytes) = rx_salida.recv() {
                if w.write_all(&bytes).is_err() || w.flush().is_err() {
                    break;
                }
            }
        });

        // initialize → esperar respuesta → initialized.
        let uri_raiz = ruta_a_uri(raiz);
        let nombre = raiz
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "workspace".to_owned());
        let init = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": uri_raiz,
                "workspaceFolders": [{ "uri": uri_raiz, "name": nombre }],
                "capabilities": {
                    "general": { "positionEncodings": ["utf-8", "utf-16"] },
                    "textDocument": {
                        "synchronization": { "dynamicRegistration": false },
                        "publishDiagnostics": { "relatedInformation": false }
                    }
                }
            }
        });
        tx.send(enmarcar(&init)).ok()?;

        let mut lector = BufReader::new(stdout);
        let utf16;
        loop {
            let v = leer_frame(&mut lector)?;
            if v.get("id") == Some(&json!(1)) {
                let enc = v["result"]["capabilities"]["positionEncoding"]
                    .as_str()
                    .unwrap_or("utf-16");
                utf16 = enc != "utf-8";
                break;
            }
            // Peticiones del servidor que lleguen antes de la respuesta: responder.
            responder_si_peticion(&v, &tx);
        }
        tx.send(enmarcar(&json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        })))
        .ok()?;

        // Hilo lector continuo: diagnósticos + respuestas a peticiones del servidor.
        let (tx_ent, rx) = channel::<Entrante>();
        let tx_resp = tx.clone();
        std::thread::spawn(move || {
            while let Some(v) = leer_frame(&mut lector) {
                if responder_si_peticion(&v, &tx_resp) {
                    continue;
                }
                if v.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
                {
                    if let Some(ent) = parsear_diagnosticos(&v["params"]) {
                        let _ = tx_ent.send(ent);
                        ctx.request_repaint();
                    }
                }
            }
        });

        Some(Self {
            _hijo: hijo,
            tx,
            rx,
            utf16,
            abiertos: HashMap::new(),
            diagnosticos: HashMap::new(),
        })
    }

    /// Recoge los diagnósticos que el servidor haya empujado desde el último fotograma.
    pub fn recibir(&mut self) {
        while let Ok(Entrante::Diagnosticos { clave, diags }) = self.rx.try_recv() {
            self.diagnosticos.insert(clave, diags);
        }
    }

    pub fn esta_abierto(&self, ruta: &Path) -> bool {
        self.abiertos.contains_key(&clave_de_ruta(ruta))
    }

    /// Notifica al servidor que se abrió un archivo (textDocument/didOpen).
    pub fn abrir(&mut self, ruta: &Path, lenguaje: &str, texto: &str) {
        let clave = clave_de_ruta(ruta);
        self.abiertos.insert(clave, 1);
        self.notificar(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": ruta_a_uri(ruta),
                    "languageId": lenguaje_lsp(lenguaje),
                    "version": 1,
                    "text": texto
                }
            }),
        );
    }

    /// Notifica un cambio de contenido (sincronización completa, es lo más simple
    /// y robusto; rust-analyzer lo acepta sin problema).
    pub fn cambiar(&mut self, ruta: &Path, texto: &str) {
        let clave = clave_de_ruta(ruta);
        let version = self.abiertos.entry(clave).or_insert(1);
        *version += 1;
        let version = *version;
        self.notificar(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": ruta_a_uri(ruta), "version": version },
                "contentChanges": [{ "text": texto }]
            }),
        );
    }

    /// Los diagnósticos del archivo, ya traducidos a índices de carácter usando
    /// su texto actual y la codificación negociada.
    pub fn diagnosticos_para(&self, ruta: &Path, texto: &str) -> Vec<Diagnostico> {
        let Some(lista) = self.diagnosticos.get(&clave_de_ruta(ruta)) else {
            return Vec::new();
        };
        lista
            .iter()
            .map(|d| Diagnostico {
                linea: d.l0 + 1,
                inicio_char: indice_char(texto, d.l0, d.c0, self.utf16),
                fin_char: indice_char(texto, d.l1, d.c1, self.utf16),
                severidad: d.severidad,
                mensaje: d.mensaje.clone(),
            })
            .collect()
    }

    fn notificar(&self, metodo: &str, params: Value) {
        let msg = json!({ "jsonrpc": "2.0", "method": metodo, "params": params });
        let _ = self.tx.send(enmarcar(&msg));
    }
}

/// Gestiona los servidores de lenguaje de la carpeta abierta (uno por tipo).
#[derive(Default)]
pub struct GestorLsp {
    raiz: Option<PathBuf>,
    servidores: HashMap<&'static str, ClienteLsp>,
}

impl GestorLsp {
    /// Fija la carpeta de trabajo (raíz del workspace) y reinicia los servidores.
    pub fn establecer_raiz(&mut self, raiz: PathBuf) {
        self.raiz = Some(raiz);
        self.servidores.clear();
    }

    /// Recoge los diagnósticos de todos los servidores vivos.
    pub fn recibir(&mut self) {
        for s in self.servidores.values_mut() {
            s.recibir();
        }
    }

    /// Devuelve el servidor para este lenguaje, arrancándolo la primera vez.
    fn cliente(&mut self, lenguaje: &str, ctx: &egui::Context) -> Option<&mut ClienteLsp> {
        let (nombre, comando) = servidor_para(lenguaje)?;
        let raiz = self.raiz.clone()?;
        if !self.servidores.contains_key(nombre) {
            let cliente = ClienteLsp::iniciar(comando, &raiz, ctx.clone())?;
            self.servidores.insert(nombre, cliente);
        }
        self.servidores.get_mut(nombre)
    }

    /// Sincroniza el archivo activo con su servidor. Devuelve `true` si hay un
    /// servidor gestionándolo (y por tanto hay que preferir sus diagnósticos a
    /// los de tree-sitter).
    pub fn sincronizar(
        &mut self,
        ruta: &Path,
        lenguaje: &str,
        texto: &str,
        cambiado: bool,
        ctx: &egui::Context,
    ) -> bool {
        let Some(cliente) = self.cliente(lenguaje, ctx) else {
            return false;
        };
        if !cliente.esta_abierto(ruta) {
            cliente.abrir(ruta, lenguaje, texto);
        } else if cambiado {
            cliente.cambiar(ruta, texto);
        }
        true
    }

    /// Diagnósticos actuales del archivo (vacío si el servidor aún no ha respondido).
    pub fn diagnosticos(&self, ruta: &Path, lenguaje: &str, texto: &str) -> Option<Vec<Diagnostico>> {
        let (nombre, _) = servidor_para(lenguaje)?;
        let cliente = self.servidores.get(nombre)?;
        Some(cliente.diagnosticos_para(ruta, texto))
    }
}

// --- Funciones libres: framing, URIs y conversión de posiciones ---

/// Envuelve un valor JSON-RPC en el marco con cabecera `Content-Length`.
fn enmarcar(valor: &Value) -> Vec<u8> {
    let cuerpo = serde_json::to_vec(valor).unwrap_or_default();
    let mut salida = format!("Content-Length: {}\r\n\r\n", cuerpo.len()).into_bytes();
    salida.extend_from_slice(&cuerpo);
    salida
}

/// Lee un mensaje enmarcado del servidor. `None` al cerrar la tubería (EOF).
fn leer_frame<R: BufRead>(lector: &mut R) -> Option<Value> {
    let mut longitud = 0usize;
    loop {
        let mut linea = String::new();
        if lector.read_line(&mut linea).ok()? == 0 {
            return None; // EOF: el servidor cerró.
        }
        let linea = linea.trim_end();
        if linea.is_empty() {
            break; // fin de las cabeceras.
        }
        if let Some(v) = linea.strip_prefix("Content-Length:") {
            longitud = v.trim().parse().ok()?;
        }
    }
    let mut buf = vec![0u8; longitud];
    lector.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

/// Si el mensaje es una *petición* del servidor (lleva `id` y `method`), le
/// responde para no dejarlo colgado. Devuelve `true` si lo era.
fn responder_si_peticion(v: &Value, tx: &Sender<Vec<u8>>) -> bool {
    let (Some(id), Some(metodo)) = (v.get("id"), v.get("method").and_then(Value::as_str)) else {
        return false;
    };
    // workspace/configuration espera un array (uno por item pedido); el resto, null.
    let resultado = if metodo == "workspace/configuration" {
        let n = v["params"]["items"].as_array().map_or(0, Vec::len);
        Value::Array(vec![Value::Null; n])
    } else {
        Value::Null
    };
    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": resultado });
    let _ = tx.send(enmarcar(&resp));
    true
}

/// Extrae los diagnósticos de un `textDocument/publishDiagnostics`.
fn parsear_diagnosticos(params: &Value) -> Option<Entrante> {
    let uri = params["uri"].as_str()?;
    let arr = params["diagnostics"].as_array()?;
    let diags = arr
        .iter()
        .filter_map(|d| {
            let r = &d["range"];
            Some(DiagLsp {
                l0: r["start"]["line"].as_u64()? as usize,
                c0: r["start"]["character"].as_u64()? as usize,
                l1: r["end"]["line"].as_u64()? as usize,
                c1: r["end"]["character"].as_u64()? as usize,
                severidad: match d["severity"].as_u64().unwrap_or(1) {
                    1 => Severidad::Error,
                    2 => Severidad::Advertencia,
                    _ => Severidad::Info,
                },
                mensaje: d["message"].as_str().unwrap_or("").to_owned(),
            })
        })
        .collect();
    Some(Entrante::Diagnosticos {
        clave: clave_de_uri(uri),
        diags,
    })
}

/// Clave normalizada de una ruta, para casar nuestras rutas con las URIs que
/// manda el servidor (minúsculas y barras hacia delante).
fn clave_de_ruta(ruta: &Path) -> String {
    ruta.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// La misma clave, pero partiendo de una `file://` URI.
fn clave_de_uri(uri: &str) -> String {
    let sin = uri.strip_prefix("file://").unwrap_or(uri);
    let decodificado = percent_decode(sin);
    // "/C:/..." → "c:/..."
    let limpio = decodificado.trim_start_matches('/');
    limpio.to_lowercase().replace('\\', "/")
}

/// Convierte una ruta del sistema en una `file://` URI con porcentaje-escape.
fn ruta_a_uri(ruta: &Path) -> String {
    let s = ruta.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file:///");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decodifica los `%XX` de una URI.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Prueba real contra rust-analyzer: crea un proyecto con un error de tipos
    // y comprueba que el servidor publica diagnósticos. Lenta (arranca e indexa
    // el servidor), por eso va marcada #[ignore]; correr con:
    //   cargo test --bin fulgur -- --ignored lsp
    #[test]
    #[ignore]
    fn rust_analyzer_detecta_error_de_tipos() {
        let dir = std::env::temp_dir().join(format!("fulgur_lsp_{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"prueba_lsp\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let archivo = src.join("main.rs");
        let codigo = "fn main() {\n    let _x: i32 = \"no es un entero\";\n}\n";
        std::fs::write(&archivo, codigo).unwrap();

        let ctx = egui::Context::default();
        let mut cliente =
            ClienteLsp::iniciar("rust-analyzer", &dir, ctx).expect("arrancar rust-analyzer");
        cliente.abrir(&archivo, "rs", codigo);

        // rust-analyzer tarda en indexar y correr cargo check: sondeamos hasta 90 s.
        let mut encontrado = false;
        for _ in 0..180 {
            cliente.recibir();
            if !cliente.diagnosticos_para(&archivo, codigo).is_empty() {
                encontrado = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        let _ = std::fs::remove_dir_all(&dir);
        assert!(encontrado, "rust-analyzer no publicó diagnósticos del error de tipos");
    }
}

/// Traduce una posición LSP (línea, carácter) a índice de carácter global del
/// texto. El `carácter` LSP cuenta unidades UTF-16 o bytes UTF-8 según la
/// codificación negociada; egui necesita índice de carácter.
fn indice_char(texto: &str, linea: usize, caracter: usize, utf16: bool) -> usize {
    let mut idx = 0usize;
    for (n, l) in texto.split_inclusive('\n').enumerate() {
        if n == linea {
            let mut unidades = 0usize;
            for ch in l.chars() {
                if unidades >= caracter {
                    return idx;
                }
                unidades += if utf16 { ch.len_utf16() } else { ch.len_utf8() };
                idx += 1;
            }
            return idx;
        }
        idx += l.chars().count();
    }
    idx
}
