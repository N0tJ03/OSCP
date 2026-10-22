//! modules/email_osint.rs — Fuentes gratuitas para emails (sin API keys):
//!   - Datos básicos: proveedor, MX, enlaces de búsqueda
//!   - XposedOrNot: filtraciones de datos en las que aparece el email
//!   - Gravatar: perfil público y cuentas vinculadas
//!   - Holehe (opcional, pip install holehe): en qué sitios está registrado.
//!     Holehe es una librería Python, así que aquí se usa su CLI.
//!   - Username del email: busca la parte antes de @ en ~700 sitios

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::domain_osint::rdap;
use super::external_tools::{find_tool, run_command, RunError, ANSI_RE};
use super::username::scan_usernames;
use crate::base::http::{json, HttpError, SESSION};
use crate::base::registry::{result, Ctx, Fields, Link, ModResult, Module, Status};
use crate::base::utils::{extract_domain, is_free_mail, search_links, truthy, vstr, vstr_or};

pub const MODULES: &[Module] = &[
    Module { name: "email_basics", title: "Datos del email", accepts: &["email"], func: email_basics },
    Module { name: "xposedornot", title: "Filtraciones (XposedOrNot)", accepts: &["email"], func: xposedornot },
    Module { name: "gravatar", title: "Perfil Gravatar", accepts: &["email"], func: gravatar },
    Module { name: "holehe", title: "Registrado en sitios (Holehe)", accepts: &["email"], func: holehe },
    Module { name: "email_username", title: "Cuentas con el usuario del email", accepts: &["email"], func: email_username },
    Module { name: "email_domain", title: "Dominio corporativo", accepts: &["email"], func: email_domain },
];

fn dns(name: &str, rtype: &str) -> Result<Vec<String>, HttpError> {
    let v = json(SESSION.get_query("https://dns.google/resolve", &[("name", name), ("type", rtype)], 10)?)?;
    Ok(v["Answer"].as_array().into_iter().flatten().map(|a| vstr(a.get("data"))).collect())
}

fn email_basics(email: &str, _ctx: &Ctx) -> ModResult {
    let (user, domain) = email.split_once('@').unwrap_or((email, ""));
    let kind = if is_free_mail(email) { "Proveedor público (gmail, outlook...)" } else { "Dominio propio / corporativo" };
    let mut fields = Fields::new();
    fields.set("Usuario", user);
    fields.set("Dominio", domain);
    fields.set("Tipo", kind);
    let mut notes = Vec::new();
    match dns(domain, "MX") {
        Ok(mx) => {
            let servers: Vec<String> = mx
                .iter()
                .map(|m| m.split_whitespace().last().unwrap_or("").trim_end_matches('.').to_string())
                .collect();
            let joined = servers.join(", ");
            fields.set("Servidores MX", if joined.is_empty() { "ninguno".into() } else { joined });
            if mx.is_empty() {
                notes.push("El dominio no tiene MX: el email probablemente no puede recibir correo.".to_string());
            }
        }
        Err(e) => notes.push(format!("No se pudo consultar MX: {e}")),
    }
    let links = search_links(&[
        ("Google: email exacto", format!("\"{email}\"")),
        ("Google: en Pastebin / leaks", format!("\"{email}\" site:pastebin.com OR site:ghostbin.com")),
        ("Google: en GitHub", format!("\"{email}\" site:github.com")),
        ("Google: en documentos", format!("\"{email}\" filetype:pdf OR filetype:xls OR filetype:doc")),
        ("Google: usuario", format!("\"{user}\"")),
    ]);
    result("email_basics", "Datos del email", Status::Info, format!("{domain} · {kind}"))
        .fields(fields)
        .links(links)
        .notes(notes)
}

fn xposedornot(email: &str, _ctx: &Ctx) -> ModResult {
    let title = "Filtraciones (XposedOrNot)";
    let data = match SESSION.get_query("https://api.xposedornot.com/v1/breach-analytics", &[("email", email)], 25) {
        Ok(r) if r.status().as_u16() == 404 => {
            return result("xposedornot", title, Status::Empty, "No aparece en filtraciones conocidas");
        }
        Ok(r) => match json(r) {
            Ok(v) => v,
            Err(e) => return result("xposedornot", title, Status::Error, e.to_string()),
        },
        Err(e) => return result("xposedornot", title, Status::Error, e.to_string()),
    };

    let mut details: Vec<Value> = data["ExposedBreaches"]["breaches_details"].as_array().cloned().unwrap_or_default();
    if details.is_empty() {
        return result("xposedornot", title, Status::Empty, "No aparece en filtraciones conocidas");
    }
    details.sort_by_key(|b| std::cmp::Reverse(vstr(b.get("xposed_date"))));
    let links: Vec<Link> = details
        .iter()
        .map(|b| {
            let dom = vstr(b.get("domain"));
            let xdata: Vec<String> = vstr_or(b.get("xposed_data"), "?")
                .split(';')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(String::from)
                .collect();
            Link::with_extra(
                format!("{} ({})", vstr(b.get("breach")), vstr_or(b.get("xposed_date"), "?")),
                if dom.is_empty() { String::new() } else { format!("https://{dom}") },
                format!("Datos: {}", xdata.join(", ")),
            )
        })
        .collect();
    let metrics = &data["BreachMetrics"];
    let mut fields = Fields::new();
    fields.set("Filtraciones", details.len());
    if let Some(risk) = metrics["risk"].get(0).filter(|r| r.is_object()) {
        fields.set(
            "Riesgo",
            format!("{} ({}/100)", vstr_or(risk.get("risk_label"), "?"), vstr_or(risk.get("risk_score"), "?")),
        );
    }
    if let Some(pw) = metrics["passwords_strength"].get(0).and_then(Value::as_object) {
        let parts: Vec<String> =
            pw.iter().filter(|(_, v)| truthy(v)).map(|(k, v)| format!("{k}: {}", vstr(Some(v)))).collect();
        fields.set("Contraseñas filtradas", parts.join(", "));
    }
    let pastes = &data["PastesSummary"]["cnt"];
    if truthy(pastes) {
        fields.set("Pastes", vstr(Some(pastes)));
    }
    result("xposedornot", title, Status::Found, format!("Aparece en {} filtraciones", details.len()))
        .fields(fields)
        .links(links)
}

fn gravatar(email: &str, _ctx: &Ctx) -> ModResult {
    let title = "Perfil Gravatar";
    let h = hex::encode(Sha256::digest(email.trim().to_lowercase().as_bytes()));
    let p = match SESSION.get(&format!("https://api.gravatar.com/v3/profiles/{h}"), 15) {
        Ok(r) if r.status().as_u16() == 404 => {
            let av = SESSION.get(&format!("https://gravatar.com/avatar/{h}?d=404"), 10);
            if av.is_ok_and(|a| a.status().as_u16() == 200) {
                return result("gravatar", title, Status::Found, "Tiene foto de Gravatar (sin perfil público)")
                    .links(vec![Link::new("Foto de perfil", format!("https://gravatar.com/avatar/{h}?s=400"))]);
            }
            return result("gravatar", title, Status::Empty, "Sin perfil en Gravatar");
        }
        Ok(r) => match json(r) {
            Ok(v) => v,
            Err(e) => return result("gravatar", title, Status::Error, e.to_string()),
        },
        Err(e) => return result("gravatar", title, Status::Error, e.to_string()),
    };

    let mut fields = Fields::new();
    for (label, key) in [
        ("Nombre", "display_name"),
        ("Ubicación", "location"),
        ("Trabajo", "job_title"),
        ("Empresa", "company"),
        ("Bio", "description"),
        ("Pronombres", "pronouns"),
    ] {
        let v = vstr(p.get(key));
        if !v.is_empty() {
            fields.set(label, v);
        }
    }
    let mut links = vec![
        Link::new("Perfil Gravatar", vstr(p.get("profile_url"))),
        Link::new("Foto de perfil", vstr(p.get("avatar_url"))),
    ];
    for acc in p["verified_accounts"].as_array().into_iter().flatten() {
        links.push(Link::with_extra(
            vstr_or(acc.get("service_label"), "cuenta"),
            vstr(acc.get("url")),
            "cuenta verificada",
        ));
    }
    for l in p["links"].as_array().into_iter().flatten() {
        links.push(Link::new(vstr_or(l.get("label"), "enlace"), vstr(l.get("url"))));
    }
    links.retain(|l| !l.url.is_empty());
    result("gravatar", title, Status::Found, format!("Perfil encontrado: {}", vstr_or(p.get("display_name"), "?")))
        .fields(fields)
        .links(links)
}

static HOLEHE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\[([+\-x!])\]\s+(\S+)(.*)$").unwrap());
static HOLEHE_TOTAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)\s+websites checked").unwrap());

fn holehe(email: &str, ctx: &Ctx) -> ModResult {
    let title = "Registrado en sitios (Holehe)";
    let Some(binary) = find_tool("holehe") else {
        return result("holehe", title, Status::Skipped, "Holehe no instalado (opcional)")
            .notes(vec!["Instálalo con:  pip install holehe".into()]);
    };
    ctx.progress(0, 1, "ejecutando holehe...");
    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => return result("holehe", title, Status::Error, e.to_string()),
    };
    let args = vec![email.to_string(), "--no-color".into(), "--no-clear".into(), "-T".into(), "20".into()];
    let out = match run_command(&binary, &args, Duration::from_secs(300), tmp.path()) {
        Ok(o) => o,
        Err(RunError::Timeout) => return result("holehe", title, Status::Error, "Tardó demasiado (>5 min)"),
        Err(RunError::Failed(e)) => return result("holehe", title, Status::Error, e),
    };
    let out = ANSI_RE.replace_all(&out, "");
    let mut used: Vec<Link> = Vec::new();
    let (mut checked, mut limited) = (0usize, 0usize);
    for cap in HOLEHE_LINE.captures_iter(&out) {
        let site = &cap[2];
        if !site.contains('.') {
            continue; // leyenda: "[+] Email used, [-] Email not used, ..."
        }
        checked += 1;
        match &cap[1] {
            "+" => used.push(Link::with_extra(
                site,
                format!("https://{site}"),
                cap[3].trim().trim_start_matches('/').trim(),
            )),
            "x" => limited += 1,
            _ => {}
        }
    }
    if let Some(total) = HOLEHE_TOTAL.captures(&out).and_then(|c| c[1].parse().ok()) {
        checked = total;
    }
    used.sort_by(|a, b| a.label.cmp(&b.label));
    let mut fields = Fields::new();
    fields.set("Sitios comprobados", checked);
    fields.set("Bloqueados por rate-limit", limited);
    if used.is_empty() {
        return result("holehe", title, Status::Empty, "No se detectaron registros").fields(fields);
    }
    result("holehe", title, Status::Found, format!("Registrado en {} sitios", used.len()))
        .fields(fields)
        .links(used)
}

fn email_username(email: &str, ctx: &Ctx) -> ModResult {
    let title = "Cuentas con el usuario del email";
    let user = email.split('@').next().unwrap_or_default();
    if user.chars().count() < 4 {
        return result("email_username", title, Status::Skipped, "Usuario demasiado corto para buscar");
    }
    let mut res = scan_usernames(&[user.to_string()], ctx, "email_username", title);
    res.notes.push(format!("Se buscó '{user}' como username: puede ser otra persona con el mismo nick."));
    res
}

fn email_domain(email: &str, ctx: &Ctx) -> ModResult {
    if is_free_mail(email) {
        return result("email_domain", "Dominio corporativo", Status::Skipped, "Proveedor público: no se analiza el dominio");
    }
    let mut res = rdap(&extract_domain(email), ctx);
    res.module = "email_domain".into();
    res.title = "Dominio corporativo (WHOIS)".into();
    res
}

