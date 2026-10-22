//! base/registry.rs — Registro modular de fuentes.
//! Cada fuente es un `Module`. Si una falla o no está instalada, las demás
//! siguen funcionando.
//!
//! Cada módulo recibe (target, ctx) y devuelve un `ModResult`:
//!     module   nombre interno
//!     title    título visible
//!     status   found | empty | info | skipped | error
//!     summary  una línea con el resultado
//!     fields   {etiqueta: valor} datos clave
//!     links    [{label, url, extra}] hallazgos clicables
//!     notes    [str] avisos

use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;

use crate::modules;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Found,
    Info,
    Empty,
    Skipped,
    Error,
}

impl Status {
    pub fn order(self) -> u8 {
        match self {
            Status::Found => 0,
            Status::Info => 1,
            Status::Empty => 2,
            Status::Skipped => 3,
            Status::Error => 4,
        }
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Link {
    pub label: String,
    pub url: String,
    pub extra: String,
}

impl Link {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Link { label: label.into(), url: url.into(), extra: String::new() }
    }

    pub fn with_extra(label: impl Into<String>, url: impl Into<String>, extra: impl Into<String>) -> Self {
        Link { label: label.into(), url: url.into(), extra: extra.into() }
    }
}

/// Diccionario ordenado {etiqueta: valor}, como un dict de Python.
#[derive(Clone, Debug, Default)]
pub struct Fields(pub Vec<(String, String)>);

impl Fields {
    pub fn new() -> Self {
        Fields(Vec::new())
    }

    /// Inserta o reemplaza conservando la posición original de la clave.
    pub fn set(&mut self, key: &str, value: impl ToString) {
        let value = value.to_string();
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key.to_string(), value)),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, String)> {
        self.0.iter()
    }
}

impl Serialize for Fields {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ModResult {
    pub module: String,
    pub title: String,
    pub status: Status,
    pub summary: String,
    pub fields: Fields,
    pub links: Vec<Link>,
    pub notes: Vec<String>,
}

/// Equivalente a `result()` de la versión Python.
pub fn result(module: &str, title: &str, status: Status, summary: impl Into<String>) -> ModResult {
    ModResult {
        module: module.into(),
        title: title.into(),
        status,
        summary: summary.into(),
        fields: Fields::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

impl ModResult {
    pub fn fields(mut self, fields: Fields) -> Self {
        self.fields = fields;
        self
    }

    pub fn links(mut self, links: Vec<Link>) -> Self {
        self.links = links;
        self
    }

    pub fn notes(mut self, notes: Vec<String>) -> Self {
        self.notes = notes;
        self
    }
}

/// Contexto que recibe cada módulo para informar de su progreso.
pub struct Ctx<'a> {
    cb: Option<&'a (dyn Fn(u64, u64, &str) + Sync)>,
}

impl<'a> Ctx<'a> {
    pub fn new(cb: &'a (dyn Fn(u64, u64, &str) + Sync)) -> Self {
        Ctx { cb: Some(cb) }
    }

    pub fn progress(&self, done: u64, total: u64, text: &str) {
        if let Some(cb) = self.cb {
            cb(done, total, text);
        }
    }
}

pub type ModFn = fn(&str, &Ctx) -> ModResult;

pub struct Module {
    pub name: &'static str,
    pub title: &'static str,
    pub accepts: &'static [&'static str],
    pub func: ModFn,
}

pub fn all_modules() -> Vec<&'static Module> {
    [
        modules::username::MODULES,
        modules::email_osint::MODULES,
        modules::domain_osint::MODULES,
        modules::phone_osint::MODULES,
        modules::name_osint::MODULES,
        modules::external_tools::MODULES,
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn get_modules(input_type: &str) -> Vec<&'static Module> {
    all_modules().into_iter().filter(|m| m.accepts.contains(&input_type)).collect()
}
