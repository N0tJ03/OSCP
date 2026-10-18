"""
modules/username.py — Búsqueda de usernames con el dataset gratuito de
WhatsMyName (WebBreacher/WhatsMyName, ~700 sitios). Sin API keys.

Esquema real del dataset: uri_check, post_body, headers, e_code, e_string,
m_code, m_string, cat. Una cuenta "existe" si el código HTTP es e_code, el
texto contiene e_string y NO contiene m_string.

Las comprobaciones van en paralelo (hilos) y el dataset se cachea 24 h en
data/wmn-data.json para no descargarlo cada vez.
"""
import json
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from core.http import make_session
from core.registry import register, result
from core.utils import name_variants

WMN_URL = ("https://raw.githubusercontent.com/WebBreacher/WhatsMyName/"
           "main/wmn-data.json")
CACHE = Path(__file__).resolve().parent.parent / "data" / "wmn-data.json"
WORKERS = 64

# Una sola reintentada: con cientos de sitios, los reintentos largos lo frenan todo
_session = make_session(pool=WORKERS, retries=0)


def load_sites() -> list:
    fresh = CACHE.exists() and time.time() - CACHE.stat().st_mtime < 86400
    if not fresh:
        try:
            r = _session.get(WMN_URL, timeout=30)
            r.raise_for_status()
            CACHE.parent.mkdir(exist_ok=True)
            CACHE.write_text(r.text, encoding="utf-8")
        except Exception:
            if not CACHE.exists():
                raise
    data = json.loads(CACHE.read_text(encoding="utf-8"))
    return [s for s in data["sites"] if s.get("valid", True) is not False]


def _check(site: dict, username: str):
    """True = existe, False = no existe, None = no se pudo comprobar."""
    try:
        url = site["uri_check"].replace("{account}", username)
        headers = site.get("headers") or {}
        if site.get("post_body"):
            body = site["post_body"].replace("{account}", username)
            r = _session.post(url, data=body, headers=headers, timeout=8)
        else:
            r = _session.get(url, headers=headers, timeout=8,
                             allow_redirects=True)
        text = r.text
        if r.status_code != site.get("e_code"):
            return False
        if site.get("e_string") and site["e_string"] not in text:
            return False
        if site.get("m_string") and site["m_string"] in text:
            return False
        return True
    except Exception:
        return None


def scan_usernames(candidates: list, ctx, module: str, title: str) -> dict:
    try:
        sites = load_sites()
    except Exception as e:
        return result(module, title, "error",
                      f"No se pudo descargar el dataset de WhatsMyName: {e}")

    jobs = [(s, u) for u in candidates for s in sites]
    found, errors, done = [], 0, 0
    ctx.progress(0, len(jobs))
    with ThreadPoolExecutor(WORKERS) as pool:
        futs = {pool.submit(_check, s, u): (s, u) for s, u in jobs}
        for fut in as_completed(futs):
            site, user = futs[fut]
            done += 1
            ok = fut.result()
            if ok is None:
                errors += 1
            elif ok:
                pretty = site.get("uri_pretty") or site["uri_check"]
                found.append({"label": site["name"],
                              "url": pretty.replace("{account}", user),
                              "extra": f"{site.get('cat', '')} · @{user}"})
                ctx.progress(done, len(jobs), f"{len(found)} encontradas")
            if done % 25 == 0:
                ctx.progress(done, len(jobs), f"{len(found)} encontradas")

    # Primero redes sociales y las categorías más útiles, luego el resto
    prio = {"social": 0, "images": 1, "video": 1, "coding": 2, "business": 2,
            "professional": 2, "dating": 3, "finance": 3, "music": 4}
    found.sort(key=lambda x: (prio.get(x["extra"].split(" ·")[0], 5),
                              x["extra"], x["label"].lower()))
    fields = {"Usernames probados": ", ".join(candidates),
              "Sitios comprobados": f"{len(sites)} × {len(candidates)}",
              "Sin respuesta / timeout": errors}
    if not found:
        return result(module, title, "empty",
                      "No se encontraron cuentas", fields)
    return result(module, title, "found",
                  f"{len(found)} cuentas encontradas", fields, found,
                  ["Revisa cada enlace: algunos sitios pueden dar falsos "
                   "positivos."])


@register(name="whatsmyname", title="Cuentas en redes (WhatsMyName)",
          accepts=["username", "name"],
          description="Busca el username en ~700 sitios; los nombres completos "
                      "se convierten en variantes (sergio.rodriguez, ...)")
def whatsmyname(target: str, ctx) -> dict:
    if " " in target.strip():
        candidates = name_variants(target, limit=8)
    else:
        candidates = [target.strip().lstrip("@")]
    return scan_usernames(candidates, ctx, "whatsmyname",
                          "Cuentas en redes (WhatsMyName)")
