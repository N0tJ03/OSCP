//! modules/username.rs — Búsqueda de usernames con el dataset gratuito de
//! WhatsMyName (WebBreacher/WhatsMyName, ~700 sitios). Sin API keys.
//!
//! Esquema real del dataset: uri_check, post_body, headers, e_code, e_string,
//! m_code, m_string, cat. Una cuenta "existe" si el código HTTP es e_code, el
//! texto contiene e_string y NO contiene m_string.
//!
//! Las comprobaciones van en paralelo (hilos) y el dataset se cachea 24 h en
//! data/wmn-data.json para no descargarlo cada vez.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::base::http::{check, Http};
use crate::base::registry::{result, Ctx, Fields, Link, ModResult, Module, Status};
use crate::base::utils::{has_accents, name_variants, root, vstr};

const WMN_URL: &str = "https://raw.githubusercontent.com/WebBreacher/WhatsMyName/main/wmn-data.json";
const WORKERS: usize = 64;

// Sin reintentos: con cientos de sitios, los reintentos largos lo frenan todo
static SESSION: LazyLock<Http> = LazyLock::new(|| Http::new(WORKERS, 0));

pub const MODULES: &[Module] = &[Module {
    name: "whatsmyname",
    title: "Cuentas en redes (WhatsMyName)",
    accepts: &["username", "name"],
    func: whatsmyname,
}];

fn cache_path() -> PathBuf {
    root().join("data").join("wmn-data.json")
}

pub fn load_sites() -> Result<Vec<Value>, String> {
    let cache = cache_path();
    let fresh = fs::metadata(&cache)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < Duration::from_secs(86400));
    if !fresh {
        let download = || -> Result<(), String> {
            let r = SESSION.get(WMN_URL, 30).and_then(check).map_err(|e| e.to_string())?;
            let text = r.text().map_err(|e| e.to_string())?;
            if let Some(dir) = cache.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            fs::write(&cache, text).map_err(|e| e.to_string())
        };
        if let Err(e) = download()
            && !cache.exists() {
                return Err(e);
            }
    }
    let raw = fs::read_to_string(&cache).map_err(|e| e.to_string())?;
    let data: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(data["sites"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.get("valid") != Some(&Value::Bool(false)))
        .collect())
}

/// Some(true) = existe, Some(false) = no existe, None = no se pudo comprobar.
fn check_site(site: &Value, username: &str) -> Option<bool> {
    let url = site["uri_check"].as_str()?.replace("{account}", username);
    let mut headers = HeaderMap::new();
    if let Some(obj) = site.get("headers").and_then(Value::as_object) {
        for (k, v) in obj {
            if let (Ok(name), Some(Ok(val))) =
                (HeaderName::from_bytes(k.as_bytes()), v.as_str().map(HeaderValue::from_str))
            {
                headers.insert(name, val);
            }
        }
    }
    let body = site.get("post_body").and_then(Value::as_str).filter(|b| !b.is_empty());
    let timeout = Duration::from_secs(8);
    let r = SESSION
        .send(|c| match body {
            Some(b) => c.post(&url).body(b.replace("{account}", username)),
            None => c.get(&url),
        }
        .headers(headers.clone())
        .timeout(timeout))
        .ok()?;
    let status = r.status().as_u16() as i64;
    let text = r.text().ok()?;
    if site.get("e_code").and_then(Value::as_i64) != Some(status) {
        return Some(false);
    }
    if let Some(e) = site.get("e_string").and_then(Value::as_str).filter(|s| !s.is_empty())
        && !text.contains(e) {
            return Some(false);
        }
    if let Some(m) = site.get("m_string").and_then(Value::as_str).filter(|s| !s.is_empty())
        && text.contains(m) {
            return Some(false);
        }
    Some(true)
}

struct ScanState {
    found: Vec<Link>,
    errors: usize,
    done: usize,
}

pub fn scan_usernames(candidates: &[String], ctx: &Ctx, module: &str, title: &str) -> ModResult {
    let sites = match load_sites() {
        Ok(s) => s,
        Err(e) => {
            return result(module, title, Status::Error, format!("No se pudo descargar el dataset de WhatsMyName: {e}"));
        }
    };

    let jobs: Vec<(&Value, &str)> =
        candidates.iter().flat_map(|u| sites.iter().map(move |s| (s, u.as_str()))).collect();
    let total = jobs.len() as u64;
    let next = AtomicUsize::new(0);
    let state = Mutex::new(ScanState { found: Vec::new(), errors: 0, done: 0 });
    ctx.progress(0, total, "");

    thread::scope(|s| {
        for _ in 0..WORKERS.min(jobs.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(site, user)) = jobs.get(i) else { break };
                let ok = check_site(site, user);
                let mut st = state.lock().unwrap();
                st.done += 1;
                match ok {
                    None => st.errors += 1,
                    Some(true) => {
                        let pretty = site
                            .get("uri_pretty")
                            .and_then(Value::as_str)
                            .filter(|p| !p.is_empty())
                            .or_else(|| site["uri_check"].as_str())
                            .unwrap_or_default();
                        st.found.push(Link::with_extra(
                            vstr(site.get("name")),
                            pretty.replace("{account}", user),
                            format!("{} · @{user}", vstr(site.get("cat"))),
                        ));
                        ctx.progress(st.done as u64, total, &format!("{} encontradas", st.found.len()));
                    }
                    Some(false) => {}
                }
                if st.done.is_multiple_of(25) {
                    ctx.progress(st.done as u64, total, &format!("{} encontradas", st.found.len()));
                }
            });
        }
    });

    let ScanState { mut found, errors, .. } = state.into_inner().unwrap();
    // Primero redes sociales y las categorías más útiles, luego el resto
    let prio = |cat: &str| match cat {
        "social" => 0,
        "images" | "video" => 1,
        "coding" | "business" | "professional" => 2,
        "dating" | "finance" => 3,
        "music" => 4,
        _ => 5,
    };
    found.sort_by_key(|x| {
        let cat = x.extra.split(" ·").next().unwrap_or("").to_string();
        (prio(&cat), x.extra.clone(), x.label.to_lowercase())
    });
    let mut fields = Fields::new();
    fields.set("Usernames probados", candidates.join(", "));
    fields.set("Sitios comprobados", format!("{} × {}", sites.len(), candidates.len()));
    fields.set("Sin respuesta / timeout", errors);
    if found.is_empty() {
        return result(module, title, Status::Empty, "No se encontraron cuentas").fields(fields);
    }
    result(module, title, Status::Found, format!("{} cuentas encontradas", found.len()))
        .fields(fields)
        .links(found)
        .notes(vec!["Revisa cada enlace: algunos sitios pueden dar falsos positivos.".into()])
}

/// Busca el username en ~700 sitios; los nombres completos se convierten en
/// variantes (sergio.rodriguez, ...)
fn whatsmyname(target: &str, ctx: &Ctx) -> ModResult {
    let candidates = if target.trim().contains(' ') {
        let mut c = name_variants(target, 8, false);
        if has_accents(target) {
            // Algunas redes admiten tildes en el usuario (pepegarcía ≠ pepegarcia)
            for v in name_variants(target, 3, true) {
                if !c.contains(&v) {
                    c.push(v);
                }
            }
        }
        c
    } else {
        vec![target.trim().trim_start_matches('@').to_string()]
    };
    scan_usernames(&candidates, ctx, "whatsmyname", "Cuentas en redes (WhatsMyName)")
}
