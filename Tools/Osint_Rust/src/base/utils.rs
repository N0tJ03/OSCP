//! base/utils.rs — Utilidades compartidas.
//!
//! FREE_MAIL_DOMAINS: dominios de proveedores públicos. Para user@gmail.com el
//! dominio es de Google, no del objetivo, así que los módulos de dominio lo saltan.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use super::registry::Link;

pub const FREE_MAIL_DOMAINS: &[&str] = &[
    "gmail.com", "googlemail.com", "yahoo.com", "yahoo.es", "yahoo.co.uk",
    "yahoo.com.mx", "hotmail.com", "hotmail.es", "outlook.com", "outlook.es",
    "live.com", "live.com.mx", "msn.com", "aol.com", "icloud.com", "me.com",
    "mac.com", "protonmail.com", "proton.me", "pm.me", "gmx.com", "gmx.es",
    "gmx.de", "web.de", "mail.ru", "yandex.ru", "yandex.com", "zoho.com",
    "tutanota.com", "tuta.io", "fastmail.com", "hushmail.com", "qq.com",
    "163.com", "126.com",
];

pub static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());

/// Carpeta base para `reports/` y `data/` (OSINT_HOME o el directorio actual).
pub fn root() -> PathBuf {
    std::env::var_os("OSINT_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn extract_domain(target: &str) -> String {
    match target.rsplit_once('@') {
        Some((_, d)) => d.trim().to_lowercase(),
        None => target.trim().to_lowercase(),
    }
}

pub fn is_free_mail(target: &str) -> bool {
    target.contains('@') && FREE_MAIL_DOMAINS.contains(&extract_domain(target).as_str())
}

const PARTICLES: &[&str] = &["de", "del", "la", "las", "los", "y", "da", "do", "dos", "van", "von"];

pub fn strip_accents(s: &str) -> String {
    s.nfkd().filter(|c| !is_combining_mark(*c)).collect()
}

/// 'Pepe Pepito García de la Torre' -> ['Pepe','Pepito','García','Torre']
/// (sin partículas; conserva tildes y mayúsculas para mostrar).
pub fn name_parts(name: &str) -> Vec<String> {
    name.split_whitespace()
        .filter(|p| !PARTICLES.contains(&p.to_lowercase().as_str()))
        .map(String::from)
        .collect()
}

/// Todas las formas en que alguien puede usar su nombre, de más a menos
/// completa. Pepe Pepito García López ->
///   [Pepe Pepito García López], [Pepe García López], [Pepe Pepito García],
///   [Pepe García], [Pepe López], [Pepe Pepito], [Pepito García López], ...
/// Siempre empiezan por el primer nombre o (si hay 3+ partes) por el segundo,
/// y conservan el orden original.
pub fn name_combos(name: &str) -> Vec<Vec<String>> {
    let parts = name_parts(name);
    if parts.len() < 2 {
        return if parts.is_empty() { vec![] } else { vec![parts] };
    }
    let starts: &[usize] = if parts.len() >= 3 { &[0, 1] } else { &[0] };
    let mut combos = Vec::new();
    for &s in starts {
        let rest = &parts[s + 1..];
        for mask in 1u32..(1 << rest.len()) {
            let mut combo = vec![parts[s].clone()];
            combo.extend((0..rest.len()).filter(|i| mask >> i & 1 == 1).map(|i| rest[i].clone()));
            combos.push(combo);
        }
    }
    // Prioridad: primer nombre antes que segundo; con 4+ partes, las que llevan
    // el primer apellido (parts[2]); luego más partes. Con 3 partes no se sabe
    // si la segunda es nombre o apellido, así que no se prioriza ninguna.
    let key_surname = if parts.len() >= 4 { Some(&parts[2]) } else { None };
    combos.sort_by_key(|c| {
        (
            c[0] != parts[0],
            key_surname.is_some_and(|k| !c.contains(k)),
            std::cmp::Reverse(c.len()),
        )
    });
    combos
}

pub fn has_accents(name: &str) -> bool {
    strip_accents(name) != name
}

/// Parte del nombre lista para username/slug: minúsculas, solo letras y
/// números; con accents=true conserva tildes y ñ (garcía, muñoz).
fn clean(part: &str, accents: bool) -> String {
    if accents {
        part.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
    } else {
        strip_accents(part)
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .collect()
    }
}

/// Usernames probables a partir de un nombre completo.
/// Pepe Pepito García -> pepepepitogarcia, pepegarcia, pepe.garcia,
/// pepe.pepito.garcia, pgarcia, pepepepito, pepitogarcia, ...
/// Con accents=true conserva las tildes: pepepepitogarcía, pepegarcía, ...
pub fn name_variants(name: &str, limit: usize, accents: bool) -> Vec<String> {
    let combos: Vec<Vec<String>> = name_combos(name)
        .iter()
        .map(|c| c.iter().map(|p| clean(p, accents)).collect())
        .collect();
    if combos.is_empty() {
        return vec![];
    }
    if combos[0].len() == 1 {
        return combos[0].clone();
    }
    let mut ordered = Vec::new();
    for (i, c) in combos.iter().enumerate() {
        ordered.push(c.concat());
        if i < 3 {
            // las 3 más probables, con punto
            ordered.push(c.join("."));
        }
        if c.len() == 2 && c[0] == combos[0][0]
            && let Some(initial) = c[0].chars().next() {
                ordered.push(format!("{initial}{}", c[1])); // pgarcia
            }
    }
    ordered.push(combos[0].join("_"));
    let mut out: Vec<String> = Vec::new();
    for v in ordered {
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out.truncate(limit);
    out
}

/// LinkedIn usa slugs con guiones (/in/pepe-pepito-garcia) y admite
/// tildes (/in/pepe-pepito-garcía): si el nombre las lleva, salen ambas.
pub fn linkedin_slugs(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in name_combos(name) {
        for accents in [true, false] {
            let slug = c.iter().map(|p| clean(p, accents)).collect::<Vec<_>>().join("-");
            if !out.contains(&slug) {
                out.push(slug);
            }
        }
    }
    out
}

/// ['Pepe García', 'Pepe Garcia'] — la original y sin tildes (si cambia).
pub fn accent_forms(text: &str) -> Vec<String> {
    let plain = strip_accents(text);
    if plain != text { vec![text.to_string(), plain] } else { vec![text.to_string()] }
}

/// Equivalente a urllib.parse.quote (espacios como %20).
pub fn quote(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// Equivalente a urllib.parse.quote_plus (espacios como +).
pub fn quote_plus(s: &str) -> String {
    urlencoding::encode(s).replace("%20", "+")
}

pub fn google(q: &str) -> String {
    format!("https://www.google.com/search?q={}", quote_plus(q))
}

/// [(label, query)] -> links de búsqueda en Google.
pub fn search_links(queries: &[(&str, String)]) -> Vec<Link> {
    queries.iter().map(|(label, q)| Link::with_extra(*label, google(q), q.clone())).collect()
}

/// Escapado HTML como html.escape de Python.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Primeros `n` caracteres (no bytes).
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Texto de un valor JSON: "" si falta o es null; los no-string se serializan.
pub fn vstr(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// Como `vstr` pero con un valor por defecto si queda vacío.
pub fn vstr_or(v: Option<&Value>, default: &str) -> String {
    let s = vstr(v);
    if s.is_empty() { default.to_string() } else { s }
}

/// Valor "verdadero" al estilo Python.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// 1234567 -> "1,234,567"
pub fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_and_variants() {
        let c = name_combos("Pepe Pepito García");
        assert_eq!(c[0], vec!["Pepe", "Pepito", "García"]);
        let v = name_variants("Pepe Pepito García", 10, false);
        assert_eq!(v[0], "pepepepitogarcia");
        assert!(v.contains(&"pgarcia".to_string()));
        assert!(name_variants("Pepe García", 3, true).contains(&"pepegarcía".to_string()));
        assert_eq!(linkedin_slugs("Pepe García")[..2], ["pepe-garcía", "pepe-garcia"]);
    }

    #[test]
    fn helpers() {
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(quote_plus("\"a b\""), "%22a+b%22");
        assert_eq!(extract_domain("x@Gmail.com"), "gmail.com");
        assert!(is_free_mail("x@gmail.com"));
    }
}
