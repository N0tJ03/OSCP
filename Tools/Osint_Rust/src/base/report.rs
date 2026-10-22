//! base/report.rs — Reporte HTML visual (tarjetas, enlaces clicables, filtro)
//! y JSON con los datos crudos.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_json::json;

use super::registry::{ModResult, Status};
use super::utils::html_escape as escape;

fn status_html(s: Status) -> (&'static str, &'static str) {
    match s {
        Status::Found => ("Resultados", "ok"),
        Status::Info => ("Info", "info"),
        Status::Empty => ("Sin resultados", "empty"),
        Status::Skipped => ("Omitido", "skip"),
        Status::Error => ("Error", "err"),
    }
}

const CSS: &str = r#"
:root{--bg:#f5f6f8;--card:#fff;--fg:#1d2330;--muted:#6b7280;--line:#e5e7eb;
--accent:#2563eb;--ok:#16a34a;--info:#0891b2;--empty:#ca8a04;--skip:#9ca3af;--err:#dc2626}
@media (prefers-color-scheme:dark){:root{--bg:#0f1318;--card:#171c23;--fg:#e6e8eb;
--muted:#8b93a1;--line:#262d37;--accent:#60a5fa;--ok:#4ade80;--info:#22d3ee;
--empty:#facc15;--skip:#6b7280;--err:#f87171}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);
font:15px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
.wrap{max-width:1100px;margin:0 auto;padding:24px 16px 60px}
header h1{margin:0 0 4px;font-size:26px}
.meta{color:var(--muted)}
.target{font-family:ui-monospace,Consolas,monospace;background:var(--card);
border:1px solid var(--line);padding:2px 8px;border-radius:6px}
.stats{display:flex;gap:12px;flex-wrap:wrap;margin:20px 0}
.stat{background:var(--card);border:1px solid var(--line);border-radius:10px;
padding:12px 16px;min-width:140px}
.stat b{display:block;font-size:24px}
.stat span{color:var(--muted);font-size:13px}
.tools{position:sticky;top:0;background:var(--bg);padding:10px 0;z-index:2}
#q{width:100%;padding:10px 14px;border-radius:8px;border:1px solid var(--line);
background:var(--card);color:var(--fg);font-size:15px}
.card{background:var(--card);border:1px solid var(--line);border-left:5px solid var(--c);
border-radius:10px;margin:14px 0;overflow:hidden}
.card>summary{list-style:none;cursor:pointer;padding:14px 16px;display:flex;
gap:12px;align-items:center;flex-wrap:wrap}
.card>summary::-webkit-details-marker{display:none}
.card h2{margin:0;font-size:17px;flex:1;min-width:200px}
.sum{color:var(--muted);width:100%}
.badge{font-size:12px;font-weight:600;padding:2px 10px;border-radius:99px;
color:var(--c);border:1px solid var(--c)}
.ok{--c:var(--ok)}.info{--c:var(--info)}.empty{--c:var(--empty)}
.skip{--c:var(--skip)}.err{--c:var(--err)}
.body{padding:0 16px 16px}
dl{display:grid;grid-template-columns:max-content 1fr;gap:4px 18px;margin:0 0 12px}
dt{color:var(--muted)}dd{margin:0;white-space:pre-wrap;word-break:break-word}
table{width:100%;border-collapse:collapse;font-size:14px}
th,td{text-align:left;padding:7px 8px;border-top:1px solid var(--line);vertical-align:top}
th{color:var(--muted);font-weight:600;border-top:0}
td.n{color:var(--muted);width:32px}td.u{min-width:200px;max-width:380px}
a{color:var(--accent);text-decoration:none;word-break:break-all}a:hover{text-decoration:underline}
.x{color:var(--muted)}
.note{color:var(--empty);margin:8px 0 0}
.tablewrap{overflow-x:auto}
@media (max-width:600px){dl{grid-template-columns:1fr}dt{font-weight:600}}
"#;

const JS: &str = r#"
const q=document.getElementById('q');
q.addEventListener('input',()=>{const v=q.value.toLowerCase();
document.querySelectorAll('tr.l').forEach(tr=>{tr.style.display=
tr.textContent.toLowerCase().includes(v)?'':'none'});
document.querySelectorAll('.card').forEach(c=>{if(v)c.open=true});});
"#;

fn card(r: &ModResult) -> String {
    let (label, cls) = status_html(r.status);
    let open = if matches!(r.status, Status::Found | Status::Info) { " open" } else { "" };
    let mut h = vec![
        format!(r#"<details class="card {cls}"{open}>"#),
        format!(
            r#"<summary><h2>{}</h2><span class="badge">{label}</span><span class="sum">{}</span></summary><div class="body">"#,
            escape(&r.title),
            escape(&r.summary)
        ),
    ];
    if !r.fields.is_empty() {
        let items: String = r
            .fields
            .iter()
            .map(|(k, v)| format!("<dt>{}</dt><dd>{}</dd>", escape(k), escape(v)))
            .collect();
        h.push(format!("<dl>{items}</dl>"));
    }
    if !r.links.is_empty() {
        let rows: String = r
            .links
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let shown = l.url.split_once("://").map_or(l.url.as_str(), |(_, rest)| rest).trim_end_matches('/');
                let a = if l.url.is_empty() {
                    String::new()
                } else {
                    format!(r#"<a href="{}" target="_blank" rel="noopener">{}</a>"#, escape(&l.url), escape(shown))
                };
                format!(
                    r#"<tr class="l"><td class="n">{}</td><td><b>{}</b></td><td class="u">{a}</td><td class="x">{}</td></tr>"#,
                    i + 1,
                    escape(&l.label),
                    escape(&l.extra)
                )
            })
            .collect();
        h.push(format!(
            r#"<div class="tablewrap"><table><tr><th>#</th><th>Sitio / hallazgo</th><th>Enlace</th><th>Detalle</th></tr>{rows}</table></div>"#
        ));
    }
    for n in r.notes.iter().filter(|n| !n.is_empty()) {
        h.push(format!(r#"<p class="note">⚠ {}</p>"#, escape(n)));
    }
    h.push("</div></details>".into());
    h.concat()
}

pub fn save_html(results: &[ModResult], target: &str, itype: &str, secs: f64, out: &Path) -> io::Result<PathBuf> {
    let mut results: Vec<&ModResult> = results.iter().collect();
    results.sort_by_key(|r| r.status.order());
    let hits: usize = results.iter().filter(|r| r.status == Status::Found).map(|r| r.links.len()).sum();
    let n_found = results.iter().filter(|r| r.status == Status::Found).count();
    let n_err = results.iter().filter(|r| r.status == Status::Error).count();
    let stats = [
        (n_found.to_string(), "fuentes con resultados"),
        (hits.to_string(), "hallazgos / enlaces"),
        (results.len().to_string(), "fuentes consultadas"),
        (n_err.to_string(), "errores"),
        (format!("{secs:.0}s"), "duración"),
    ];
    let stats_html: String = stats
        .iter()
        .map(|(v, t)| format!(r#"<div class="stat"><b>{v}</b><span>{t}</span></div>"#))
        .collect();
    let cards: String = results.iter().map(|r| card(r)).collect();
    let t = escape(target);
    let html = format!(
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OSINT · {t}</title><style>{CSS}</style></head><body><div class="wrap">
<header><h1>Reporte OSINT</h1>
<div class="meta">Objetivo: <span class="target">{t}</span> · tipo
<b>{}</b> · {}</div></header>
<div class="stats">{stats_html}</div>
<div class="tools"><input id="q" placeholder="Filtrar hallazgos (sitio, enlace, dato)..."></div>
{cards}
</div><script>{JS}</script></body></html>"#,
        escape(itype),
        Local::now().format("%d/%m/%Y %H:%M")
    );
    fs::write(out, html)?;
    Ok(out.to_path_buf())
}

pub fn save_json(results: &[ModResult], target: &str, itype: &str, out: &Path) -> io::Result<PathBuf> {
    let data = json!({
        "target": target,
        "type": itype,
        "generated_at": Local::now().format("%Y-%m-%dT%H:%M:%S%.6f").to_string(),
        "results": results,
    });
    fs::write(out, serde_json::to_string_pretty(&data)?)?;
    Ok(out.to_path_buf())
}
