//! modules/domain_osint.rs — Información de dominios con servicios gratuitos:
//!   - RDAP (WHOIS moderno): registrador, fechas, estado
//!   - DNS vía Google DNS-over-HTTPS: A, AAAA, MX, NS, TXT, DMARC
//!   - crt.sh: subdominios por certificados (Certificate Transparency)
//!   - HIBP: filtraciones públicas conocidas de ese dominio

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::base::http::{json, HttpError, SESSION};
use crate::base::registry::{result, Ctx, Fields, Link, ModResult, Module, Status};
use crate::base::utils::{extract_domain, search_links, thousands, vstr, vstr_or};

pub const MODULES: &[Module] = &[
    Module { name: "rdap", title: "WHOIS / RDAP", accepts: &["domain"], func: rdap },
    Module { name: "dns_lookup", title: "Registros DNS", accepts: &["domain"], func: dns_lookup },
    Module { name: "crtsh", title: "Subdominios (certificados)", accepts: &["domain"], func: crtsh },
    Module { name: "hibp_domain", title: "Filtraciones del dominio (HIBP)", accepts: &["domain"], func: hibp_domain },
];

static RDAP_SERVERS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Servidor RDAP oficial del TLD según el registro de IANA.
fn rdap_base(tld: &str) -> Result<Option<String>, HttpError> {
    let mut cache = RDAP_SERVERS.lock().unwrap();
    if cache.is_none() {
        let data = json(SESSION.get("https://data.iana.org/rdap/dns.json", 20)?)?;
        let mut map = HashMap::new();
        for svc in data["services"].as_array().into_iter().flatten() {
            let url = svc[1][0].as_str().unwrap_or_default().trim_end_matches('/').to_string() + "/";
            for t in svc[0].as_array().into_iter().flatten().filter_map(Value::as_str) {
                map.insert(t.to_string(), url.clone());
            }
        }
        *cache = Some(map);
    }
    Ok(cache.as_ref().and_then(|m| m.get(tld).cloned()))
}

pub fn rdap(domain: &str, _ctx: &Ctx) -> ModResult {
    let title = "WHOIS / RDAP";
    let d = extract_domain(domain);
    let tld = d.rsplit('.').next().unwrap_or_default();
    let base = rdap_base(tld).ok().flatten();
    let mut urls: Vec<String> = Vec::new();
    if let Some(b) = base {
        urls.push(format!("{b}domain/{d}"));
    }
    urls.push(format!("https://rdap.org/domain/{d}"));

    let mut data: Option<Value> = None;
    let mut err: Option<HttpError> = None;
    for url in &urls {
        match SESSION.get(url, 15) {
            Ok(r) if r.status().as_u16() == 404 => {
                return result("rdap", title, Status::Empty, "Dominio no registrado");
            }
            Ok(r) => match json(r) {
                Ok(v) => {
                    data = Some(v);
                    break;
                }
                Err(e) => err = Some(e),
            },
            Err(e) => err = Some(e),
        }
    }
    let Some(data) = data else {
        let msg = err.map_or("Este TLD no tiene servicio RDAP público".to_string(), |e| e.to_string());
        return result("rdap", title, Status::Error, msg);
    };

    let mut events: HashMap<String, String> = HashMap::new();
    for e in data["events"].as_array().into_iter().flatten() {
        let date: String = vstr(e.get("eventDate")).chars().take(10).collect();
        events.insert(vstr(e.get("eventAction")), date);
    }
    let ev = |k: &str| events.get(k).cloned().unwrap_or_default();
    let mut fields = Fields::new();
    fields.set("Dominio", vstr_or(data.get("ldhName"), &d).to_lowercase());
    fields.set("Creado", ev("registration"));
    fields.set("Expira", ev("expiration"));
    fields.set("Último cambio", ev("last changed"));
    let status: Vec<String> = data["status"].as_array().into_iter().flatten().map(|s| vstr(Some(s))).collect();
    fields.set("Estado", status.join(", "));
    for ent in data["entities"].as_array().into_iter().flatten() {
        let roles: Vec<&str> = ent["roles"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let name = ent["vcardArray"][1].as_array().into_iter().flatten().find_map(|v| {
            let kind = v[0].as_str()?;
            let val = vstr(v.get(3));
            ((kind == "fn" || kind == "org") && !val.is_empty() && val != "[]").then_some(val)
        });
        match name {
            Some(n) if roles.contains(&"registrar") => fields.set("Registrador", n),
            Some(n) if roles.contains(&"registrant") => fields.set("Titular", n),
            _ => {}
        }
    }
    let ns: Vec<String> = data["nameservers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|n| vstr(n.get("ldhName")).to_lowercase())
        .collect();
    fields.set("Nameservers", ns.join(", "));
    fields.0.retain(|(_, v)| !v.is_empty());

    let mut links = vec![
        Link::new("Web", format!("https://{d}")),
        Link::new("Archivo histórico (Wayback)", format!("https://web.archive.org/web/*/{d}")),
    ];
    links.extend(search_links(&[
        ("Google: páginas indexadas", format!("site:{d}")),
        ("Google: emails del dominio", format!("\"@{d}\"")),
    ]));
    let summary = format!(
        "Registrado desde {} · {}",
        fields.get("Creado").unwrap_or("?"),
        fields.get("Registrador").unwrap_or("registrador ?")
    );
    result("rdap", title, Status::Found, summary).fields(fields).links(links)
}

fn doh(name: &str, rtype: &str) -> Vec<String> {
    let Ok(r) = SESSION.get_query("https://dns.google/resolve", &[("name", name), ("type", rtype)], 10) else {
        return vec![];
    };
    let Ok(v) = r.json::<Value>() else { return vec![] };
    v["Answer"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a.get("type").and_then(Value::as_i64) != Some(5)) // ignora CNAME intermedios
        .map(|a| vstr(a.get("data")))
        .collect()
}

fn dns_lookup(domain: &str, _ctx: &Ctx) -> ModResult {
    let title = "Registros DNS";
    let d = extract_domain(domain);
    let mut fields = Fields::new();
    for rtype in ["A", "AAAA", "MX", "NS", "TXT"] {
        let vals = doh(&d, rtype);
        if !vals.is_empty() {
            fields.set(rtype, vals.join("\n"));
        }
    }
    let dmarc = doh(&format!("_dmarc.{d}"), "TXT");
    if !dmarc.is_empty() {
        fields.set("DMARC", dmarc.join("\n"));
    }
    if fields.is_empty() {
        return result("dns_lookup", title, Status::Empty, "El dominio no resuelve");
    }
    // Servicios que usa la organización, deducidos de verificaciones TXT / MX
    let txt = format!("{}{}", fields.get("TXT").unwrap_or(""), fields.get("MX").unwrap_or("")).to_lowercase();
    const MARKERS: &[(&str, &str)] = &[
        ("google-site-verification", "Google"), ("ms=", "Microsoft 365"),
        ("outlook.com", "Exchange Online"), ("aspmx.l.google.com", "Gmail / Workspace"),
        ("facebook-domain", "Facebook"), ("atlassian", "Atlassian"),
        ("apple-domain", "Apple"), ("docusign", "DocuSign"), ("zoho", "Zoho"),
        ("stripe-verification", "Stripe"), ("shopify", "Shopify"),
        ("adobe-idp", "Adobe"), ("openai-domain", "OpenAI"),
        ("anthropic-domain", "Anthropic"), ("_spf.salesforce", "Salesforce"),
        ("zendesk", "Zendesk"), ("sendgrid", "SendGrid"), ("mailchimp", "Mailchimp"),
        ("servers.mcsv", "Mailchimp"), ("hubspot", "HubSpot"), ("slack", "Slack"),
        ("tailscale", "Tailscale"), ("calendly", "Calendly"), ("miro", "Miro"),
    ];
    let mut services: Vec<&str> = Vec::new();
    for (marker, name) in MARKERS {
        if txt.contains(marker) && !services.contains(name) {
            services.push(name);
        }
    }
    let mut summary = format!("{} tipos de registro", fields.len());
    if !services.is_empty() {
        fields.0.insert(0, ("Servicios detectados".into(), services.join(", ")));
        summary += &format!(" · usa {} servicios conocidos", services.len());
    }
    result("dns_lookup", title, Status::Found, summary).fields(fields)
}

fn crtsh(domain: &str, _ctx: &Ctx) -> ModResult {
    let title = "Subdominios (certificados)";
    let d = extract_domain(domain);
    let mut raw: Option<Vec<String>> = None;
    let mut source = "crt.sh";
    let q = format!("%.{d}");
    for _ in 0..2 {
        // crt.sh suele dar 502 bajo carga
        let attempt = SESSION
            .send(|c| c.get("https://crt.sh/").query(&[("q", q.as_str()), ("output", "json")]).timeout(Duration::from_secs(40)))
            .and_then(json);
        match attempt {
            Ok(v) => {
                raw = Some(
                    v.as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|e| vstr(e.get("name_value")).split('\n').map(String::from).collect::<Vec<_>>())
                        .collect(),
                );
                break;
            }
            Err(_) => thread::sleep(Duration::from_secs(3)),
        }
    }
    let raw = match raw {
        Some(r) => r,
        None => {
            // respaldo: CertSpotter (gratis, sin key)
            source = "CertSpotter";
            match certspotter(&d, 5) {
                Ok(r) => r,
                Err(e) => {
                    return result("crtsh", title, Status::Error, format!("crt.sh y CertSpotter no responden ({e})"));
                }
            }
        }
    };
    let suffix = format!(".{d}");
    let names: BTreeSet<String> = raw
        .iter()
        .map(|n| n.trim().to_lowercase().trim_start_matches(['*', '.']).to_string())
        .filter(|n| n.ends_with(&suffix))
        .collect();
    if names.is_empty() {
        return result("crtsh", title, Status::Empty, "Sin subdominios en certificados");
    }
    let links = names.iter().take(200).map(|n| Link::new(n.as_str(), format!("https://{n}"))).collect();
    let mut fields = Fields::new();
    fields.set("Total", names.len());
    fields.set("Fuente", source);
    let notes = if names.len() > 200 { vec!["Mostrando los primeros 200.".to_string()] } else { vec![] };
    result("crtsh", title, Status::Found, format!("{} subdominios", names.len()))
        .fields(fields)
        .links(links)
        .notes(notes)
}

fn certspotter(d: &str, pages: usize) -> Result<Vec<String>, HttpError> {
    let mut names = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..pages {
        let mut params = vec![("domain", d.to_string()), ("include_subdomains", "true".into()), ("expand", "dns_names".into())];
        if let Some(a) = &after {
            params.push(("after", a.clone()));
        }
        let batch = json(SESSION.send(|c| {
            c.get("https://api.certspotter.com/v1/issuances").query(&params).timeout(Duration::from_secs(30))
        })?)?;
        let Some(items) = batch.as_array().filter(|a| !a.is_empty()) else { break };
        for c in items {
            names.extend(c["dns_names"].as_array().into_iter().flatten().map(|n| vstr(Some(n))));
        }
        after = items.last().map(|c| vstr(c.get("id")));
    }
    Ok(names)
}

fn hibp_domain(domain: &str, _ctx: &Ctx) -> ModResult {
    let title = "Filtraciones del dominio (HIBP)";
    let d = extract_domain(domain);
    let data = match SESSION.get("https://haveibeenpwned.com/api/v3/breaches", 25).and_then(json) {
        Ok(v) => v,
        Err(e) => return result("hibp_domain", title, Status::Error, e.to_string()),
    };
    let hits: Vec<&Value> = data
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| vstr(b.get("Domain")).to_lowercase() == d)
        .collect();
    if hits.is_empty() {
        return result("hibp_domain", title, Status::Empty, "Este servicio no tiene filtraciones públicas conocidas");
    }
    let links: Vec<Link> = hits
        .iter()
        .map(|b| {
            let classes: Vec<String> = b["DataClasses"].as_array().into_iter().flatten().map(|c| vstr(Some(c))).collect();
            Link::with_extra(
                format!("{} ({})", vstr(b.get("Title")), vstr(b.get("BreachDate"))),
                format!("https://haveibeenpwned.com/PwnedWebsites#{}", vstr(b.get("Name"))),
                format!("{} cuentas · {}", thousands(b["PwnCount"].as_i64().unwrap_or(0)), classes.join(", ")),
            )
        })
        .collect();
    result("hibp_domain", title, Status::Found, format!("{} filtraciones del servicio", hits.len())).links(links)
}
