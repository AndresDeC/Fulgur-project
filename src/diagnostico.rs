// Detección de errores de sintaxis con tree-sitter.
//
// Importante: esto NO es IA. tree-sitter es un parser real (el mismo que usan
// Zed o Neovim), pesa pocos MB y es incremental. Construye el árbol de sintaxis
// del archivo y reporta dos tipos de nodo problemáticos:
//   - ERROR:   un trozo de código que no encaja con la gramática.
//   - MISSING: falta un token que la gramática esperaba (p. ej. un «;» o un «)»).
//
// Añadir un lenguaje nuevo = añadir su crate de gramática y un brazo al `match`
// de `gramatica()`. Nada más.

use tree_sitter::{Node, Parser};

/// Un error de sintaxis ya localizado, listo para pintar y listar.
pub struct Diagnostico {
    /// Línea donde empieza (1-based), para mostrarla al usuario.
    pub linea: usize,
    /// Índice de carácter global del inicio y fin del error. Son índices de
    /// carácter (no de byte) porque es lo que entiende el galley de egui para
    /// traducir a coordenadas de pantalla y dibujar el subrayado.
    pub inicio_char: usize,
    pub fin_char: usize,
    pub mensaje: String,
}

/// Gramática para un código de lenguaje de Fulgur ("rs", "py", …).
/// `None` = no tenemos parser para ese lenguaje (no se analiza nada).
fn gramatica(codigo: &str) -> Option<tree_sitter::Language> {
    let lang = match codigo {
        "rs" => tree_sitter_rust::LANGUAGE,
        "py" => tree_sitter_python::LANGUAGE,
        "js" => tree_sitter_javascript::LANGUAGE,
        "json" => tree_sitter_json::LANGUAGE,
        _ => return None,
    };
    Some(lang.into())
}

/// ¿Sabemos analizar este lenguaje? Lo usa la UI para distinguir
/// «0 errores» de «sin análisis para este lenguaje».
pub fn soportado(codigo: &str) -> bool {
    gramatica(codigo).is_some()
}

/// Nodo problemático en bruto: posiciones en bytes, tal como las da tree-sitter.
struct Bruto {
    inicio_byte: usize,
    fin_byte: usize,
    linea: usize,
    mensaje: String,
}

/// Analiza el código y devuelve sus errores de sintaxis.
/// Para lenguajes sin gramática devuelve una lista vacía.
pub fn analizar(codigo_lenguaje: &str, fuente: &str) -> Vec<Diagnostico> {
    let Some(lang) = gramatica(codigo_lenguaje) else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return Vec::new();
    }
    let Some(arbol) = parser.parse(fuente, None) else {
        return Vec::new();
    };

    let raiz = arbol.root_node();
    if !raiz.has_error() {
        return Vec::new(); // atajo: el árbol está limpio, no hay nada que recorrer.
    }

    let mut brutos = Vec::new();
    recolectar(raiz, &mut brutos);

    // Traducir bytes → índices de carácter (una sola vez, no por fotograma).
    // Los offsets de tree-sitter siempre caen en frontera de carácter UTF-8.
    brutos
        .into_iter()
        .map(|b| Diagnostico {
            linea: b.linea,
            inicio_char: fuente[..b.inicio_byte.min(fuente.len())].chars().count(),
            fin_char: fuente[..b.fin_byte.min(fuente.len())].chars().count(),
            mensaje: b.mensaje,
        })
        .collect()
}

/// Recorre el árbol en profundidad recogiendo nodos ERROR y MISSING.
/// Poda ramas sin errores (`has_error`) y no profundiza dentro de un error ya
/// encontrado: basta con marcar la región una vez.
fn recolectar(nodo: Node, out: &mut Vec<Bruto>) {
    if nodo.is_error() || nodo.is_missing() {
        let mensaje = if nodo.is_missing() {
            format!("Falta «{}»", nodo.kind())
        } else {
            "Error de sintaxis".to_owned()
        };
        out.push(Bruto {
            inicio_byte: nodo.start_byte(),
            fin_byte: nodo.end_byte(),
            linea: nodo.start_position().row + 1,
            mensaje,
        });
        return;
    }
    if !nodo.has_error() {
        return;
    }
    let mut cursor = nodo.walk();
    for hijo in nodo.children(&mut cursor) {
        recolectar(hijo, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_valido_sin_errores() {
        let codigo = "fn main() {\n    println!(\"hola\");\n}\n";
        assert!(analizar("rs", codigo).is_empty());
    }

    #[test]
    fn rust_con_llave_sin_cerrar() {
        let codigo = "fn main() {\n    let x = 1;\n"; // falta la «}»
        assert!(!analizar("rs", codigo).is_empty());
    }

    #[test]
    fn python_con_parentesis_sin_cerrar() {
        let codigo = "print('hola'\n"; // falta el «)»
        assert!(!analizar("py", codigo).is_empty());
    }

    #[test]
    fn json_invalido_detectado() {
        let codigo = "{ \"a\": }"; // valor faltante
        assert!(!analizar("json", codigo).is_empty());
    }

    #[test]
    fn lenguaje_sin_gramatica_no_falla() {
        // Un lenguaje que no soportamos devuelve lista vacía, no pánico.
        assert!(analizar("txt", "lo que sea $$$ {[(").is_empty());
        assert!(!soportado("txt"));
        assert!(soportado("rs"));
    }
}
