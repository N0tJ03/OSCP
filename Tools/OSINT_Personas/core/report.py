"""
core/report.py — Reporte HTML visual (tarjetas, enlaces clicables, filtro)
y JSON con los datos crudos.
"""
import json
from datetime import datetime
from html import escape
from pathlib import Path

STATUS = {"found": ("Resultados", "ok"), "info": ("Info", "info"),
          "empty": ("Sin resultados", "empty"), "skipped": ("Omitido", "skip"),
          "error": ("Error", "err")}
ORDER = {"found": 0, "info": 1, "empty": 2, "skipped": 3, "error": 4}

CSS = """
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
"""

JS = """
const q=document.getElementById('q');
q.addEventListener('input',()=>{const v=q.value.toLowerCase();
document.querySelectorAll('tr.l').forEach(tr=>{tr.style.display=
tr.textContent.toLowerCase().includes(v)?'':'none'});
document.querySelectorAll('.card').forEach(c=>{if(v)c.open=true});});
"""


def _card(r: dict) -> str:
    label, cls = STATUS.get(r["status"], (r["status"], "info"))
    h = [f'<details class="card {cls}"{" open" if r["status"] in ("found", "info") else ""}>',
         f'<summary><h2>{escape(r["title"])}</h2><span class="badge">{label}</span>'
         f'<span class="sum">{escape(r["summary"])}</span></summary><div class="body">']
    if r["fields"]:
        h.append("<dl>" + "".join(f"<dt>{escape(str(k))}</dt><dd>{escape(str(v))}</dd>"
                                  for k, v in r["fields"].items()) + "</dl>")
    if r["links"]:
        rows = []
        for i, l in enumerate(r["links"], 1):
            url = l.get("url") or ""
            shown = url.split("://", 1)[-1].rstrip("/")
            a = f'<a href="{escape(url)}" target="_blank" rel="noopener">' \
                f'{escape(shown)}</a>' if url else ""
            rows.append(f'<tr class="l"><td class="n">{i}</td><td><b>{escape(l["label"])}'
                        f'</b></td><td class="u">{a}</td><td class="x">'
                        f'{escape(l.get("extra") or "")}</td></tr>')
        h.append('<div class="tablewrap"><table><tr><th>#</th><th>Sitio / hallazgo'
                 '</th><th>Enlace</th><th>Detalle</th></tr>' + "".join(rows) +
                 "</table></div>")
    for n in r["notes"]:
        if n:
            h.append(f'<p class="note">⚠ {escape(n)}</p>')
    h.append("</div></details>")
    return "".join(h)


def save_html(results: list, target: str, itype: str, secs: float, out: Path) -> Path:
    results = sorted(results, key=lambda r: ORDER.get(r["status"], 9))
    hits = sum(len(r["links"]) for r in results if r["status"] == "found")
    n_found = sum(r["status"] == "found" for r in results)
    n_err = sum(r["status"] == "error" for r in results)
    stats = [(n_found, "fuentes con resultados"), (hits, "hallazgos / enlaces"),
             (len(results), "fuentes consultadas"), (n_err, "errores"),
             (f"{secs:.0f}s", "duración")]
    html = f"""<!doctype html><html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OSINT · {escape(target)}</title><style>{CSS}</style></head><body><div class="wrap">
<header><h1>Reporte OSINT</h1>
<div class="meta">Objetivo: <span class="target">{escape(target)}</span> · tipo
<b>{escape(itype)}</b> · {datetime.now():%d/%m/%Y %H:%M}</div></header>
<div class="stats">{"".join(f'<div class="stat"><b>{v}</b><span>{t}</span></div>' for v, t in stats)}</div>
<div class="tools"><input id="q" placeholder="Filtrar hallazgos (sitio, enlace, dato)..."></div>
{"".join(_card(r) for r in results)}
</div><script>{JS}</script></body></html>"""
    out.write_text(html, encoding="utf-8")
    return out


def save_json(results: list, target: str, itype: str, out: Path) -> Path:
    out.write_text(json.dumps({"target": target, "type": itype,
                               "generated_at": datetime.now().isoformat(),
                               "results": results},
                              indent=2, ensure_ascii=False), encoding="utf-8")
    return out
