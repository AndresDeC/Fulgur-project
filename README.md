<p align="center">
  <img src="assets/logo.png" alt="Fulgur" width="140">
</p>

# Fulgur ⚡

Un IDE nativo, ultraligero, escrito en Rust. Se parece a Visual Studio Code pero
está pensado desde el primer byte para **no comerse tu RAM**: en reposo usa unos
**90 MB** de memoria privada, frente a los **más de 2 GB** de VS Code en la misma
máquina — unas **25× menos**.

No es Electron. No hay un Chromium escondido dentro. Es una sola ventana nativa
dibujada con [egui](https://github.com/emilk/egui)/[eframe](https://github.com/emilk/egui/tree/master/crates/eframe)
sobre OpenGL (backend `glow`).

---

## Características

- **Explorador de archivos** real: abrir carpeta, árbol recursivo, crear / renombrar
  / eliminar (ignora `target`, `.git`, `node_modules`).
- **Editor con pestañas** estilo VS Code: resaltado de sintaxis (syntect), selector
  de lenguaje, indicador de cambios sin guardar.
- **Detección de errores de sintaxis** con [tree-sitter](https://tree-sitter.github.io/):
  un parser real (no IA), exacto e incremental, para **19 lenguajes**. Subraya el
  error en rojo y lo cuenta en la barra de estado, con el detalle al pasar el ratón.
- **Terminal integrada** conectada a PowerShell real: ejecución en vivo, directorio
  de trabajo persistente, historial con flechas.
- **Control de versiones (Git)**: panel de status / stage / commit / push / pull / init.
- **Asistente de IA** conectable a [Ollama](https://ollama.com/) (local, gratis) o a
  la API de Anthropic (Claude), con opción de enviar el archivo abierto como contexto.
- **Mercado de extensiones** descentralizado: fuentes por URL o ruta local, integridad
  verificada con SHA-256; las extensiones son declarativas (temas y snippets, no código).
- **Temas integrados**: Normal (gris VS Code), Oscuro (negro neón) y Claro.

### Lenguajes con detección de errores de sintaxis

Rust · Python · JavaScript · TypeScript · JSON · HTML · CSS · C · C++ · C# · Java ·
Go · PHP · Ruby · Lua · Bash · XML · YAML · SQL

El resaltado de sintaxis cubre además texto plano, Markdown y TOML.

---

## Consumo de memoria

Medido en reposo en Windows 11 (misma máquina, mismo momento):

| Editor | Memoria privada | Working set | Procesos |
|--------|-----------------|-------------|----------|
| **Fulgur** | **~90 MB** | ~180 MB | 1 |
| VS Code | ~2 218 MB | ~2 041 MB | 12 |

Las gramáticas de tree-sitter solo ocupan memoria cuando analizan su lenguaje, así
que añadir soporte para más lenguajes **no sube la RAM en reposo**.

---

## Requisitos

- [Rust](https://rustup.rs/) (edición 2024, `rustc` 1.85+).
- Windows con las *Build Tools* de Visual Studio (incluyen el compilador C que
  necesitan las gramáticas de tree-sitter).

Opcionales, según lo que uses:

- [Ollama](https://ollama.com/) o una clave de la API de Anthropic para el asistente de IA.
- `git` en el `PATH` para el panel de control de versiones.

---

## Compilar y ejecutar

```sh
# Desarrollo
cargo run

# Versión optimizada y ligera (recomendada para el día a día)
cargo build --release
./target/release/fulgur       # el .exe es autónomo, no necesita instalación
```

### Descargable

Cada versión etiquetada publica automáticamente un `.exe` de Windows autónomo en la
pestaña **[Releases](../../releases)** del repositorio (vía GitHub Actions). Descárgalo
y ejecútalo: no hay instalador.

---

## Atajos

| Acción | Atajo |
|--------|-------|
| Archivo nuevo | `Ctrl+N` |
| Abrir archivo | `Ctrl+O` |
| Guardar | `Ctrl+S` |
| Cerrar pestaña | `Ctrl+W` |
| Deshacer / Rehacer | `Ctrl+Z` / `Ctrl+Y` |
| Cortar / Copiar / Pegar | `Ctrl+X` / `Ctrl+C` / `Ctrl+V` |
| Seleccionar todo | `Ctrl+A` |

---

## Arquitectura

Todo el estado de la aplicación vive en `src/`, un módulo por responsabilidad:

| Módulo | Responsabilidad |
|--------|-----------------|
| `app.rs` | Ventana principal, menús, layout de paneles y barra de estado. |
| `explorador.rs` | Árbol de archivos y operaciones sobre ellos. |
| `editor.rs` | Pestañas, resaltado, selector de lenguaje, guardado y subrayado de errores. |
| `diagnostico.rs` | Detección de errores de sintaxis con tree-sitter. |
| `terminal.rs` | Terminal conectada a PowerShell (hilos + canales, salida en vivo). |
| `git.rs` | Panel de control de versiones ejecutando el CLI de `git`. |
| `ia.rs` | Asistente de IA (Ollama / Anthropic) en un hilo aparte. |
| `mercado.rs` | Mercado de extensiones descentralizado con verificación SHA-256. |
| `tema.rs` | Temas integrados y paleta global. |

La configuración del usuario se guarda en `~/.fulgur/` (tema, proveedor de IA,
extensiones instaladas).

---

## Hoja de ruta

- [x] Editor, terminal, Git, IA y mercado de extensiones.
- [x] Backend `glow` para minimizar la RAM.
- [x] Detección de errores de sintaxis con tree-sitter (19 lenguajes).
- [x] Errores **semánticos** vía LSP (rust-analyzer, gopls): tipos, imports, borrow checker.
- [x] Más temas profesionales (Neon Black, Tokyo Night, One Dark, Papel, Solar).
- [x] Botón «Ejecutar ▶» (y F5) que detecta el lenguaje y lanza el comando en la terminal.
- [ ] Más funciones LSP: autocompletado, ir a definición, hover.
