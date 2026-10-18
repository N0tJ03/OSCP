"""
modules/email_osint.py — Fuentes gratuitas para emails (sin API keys):
  - Datos básicos: proveedor, MX, enlaces de búsqueda
  - XposedOrNot: filtraciones de datos en las que aparece el email
  - Gravatar: perfil público y cuentas vinculadas
  - Holehe (opcional, pip install holehe): en qué sitios está registrado
  - Username del email: busca la parte antes de @ en ~700 sitios
"""
import asyncio
import hashlib

from core.http import session
from core.registry import register, result, short_error
from core.utils import extract_domain, is_free_mail, search_links


def _dns(name: str, rtype: str) -> list:
    r = session.get("https://dns.google/resolve",
                    params={"name": name, "type": rtype}, timeout=10)
    r.raise_for_status()
    return [a.get("data", "") for a in r.json().get("Answer", [])]


@register(name="email_basics", title="Datos del email", accepts=["email"])
def email_basics(email: str, ctx) -> dict:
    user, domain = email.split("@", 1)
    fields = {"Usuario": user, "Dominio": domain,
              "Tipo": "Proveedor público (gmail, outlook...)"
              if is_free_mail(email) else "Dominio propio / corporativo"}
    notes = []
    try:
        mx = _dns(domain, "MX")
        fields["Servidores MX"] = ", ".join(m.split()[-1].rstrip(".")
                                            for m in mx) or "ninguno"
        if not mx:
            notes.append("El dominio no tiene MX: el email probablemente no "
                         "puede recibir correo.")
    except Exception as e:
        notes.append(f"No se pudo consultar MX: {e}")
    links = search_links([
        ("Google: email exacto", f'"{email}"'),
        ("Google: en Pastebin / leaks", f'"{email}" site:pastebin.com OR site:ghostbin.com'),
        ("Google: en GitHub", f'"{email}" site:github.com'),
        ("Google: en documentos", f'"{email}" filetype:pdf OR filetype:xls OR filetype:doc'),
        ("Google: usuario", f'"{user}"'),
    ])
    return result("email_basics", "Datos del email", "info",
                  f"{domain} · {fields['Tipo']}", fields, links, notes)


@register(name="xposedornot", title="Filtraciones (XposedOrNot)",
          accepts=["email"])
def xposedornot(email: str, ctx) -> dict:
    title = "Filtraciones (XposedOrNot)"
    try:
        r = session.get("https://api.xposedornot.com/v1/breach-analytics",
                        params={"email": email}, timeout=25)
        if r.status_code == 404:
            return result("xposedornot", title, "empty",
                          "No aparece en filtraciones conocidas")
        r.raise_for_status()
        data = r.json()
    except Exception as e:
        return result("xposedornot", title, "error", short_error(e))

    details = (data.get("ExposedBreaches") or {}).get("breaches_details") or []
    if not details:
        return result("xposedornot", title, "empty",
                      "No aparece en filtraciones conocidas")
    links = []
    for b in sorted(details, key=lambda b: str(b.get("xposed_date", "")),
                    reverse=True):
        dom = b.get("domain") or ""
        links.append({"label": f"{b.get('breach')} ({b.get('xposed_date', '?')})",
                      "url": f"https://{dom}" if dom else "",
                      "extra": "Datos: " + ", ".join(
                          x.strip() for x in str(b.get("xposed_data") or "?")
                          .split(";") if x.strip())})
    metrics = data.get("BreachMetrics") or {}
    fields = {"Filtraciones": len(details)}
    risk = metrics.get("risk") or []
    if risk and isinstance(risk[0], dict):
        fields["Riesgo"] = f"{risk[0].get('risk_label', '?')} " \
                           f"({risk[0].get('risk_score', '?')}/100)"
    pw = metrics.get("passwords_strength") or []
    if pw and isinstance(pw[0], dict):
        fields["Contraseñas filtradas"] = ", ".join(
            f"{k}: {v}" for k, v in pw[0].items() if v)
    pastes = (data.get("PastesSummary") or {}).get("cnt")
    if pastes:
        fields["Pastes"] = pastes
    return result("xposedornot", title, "found",
                  f"Aparece en {len(details)} filtraciones", fields, links)


@register(name="gravatar", title="Perfil Gravatar", accepts=["email"])
def gravatar(email: str, ctx) -> dict:
    title = "Perfil Gravatar"
    h = hashlib.sha256(email.strip().lower().encode()).hexdigest()
    try:
        r = session.get(f"https://api.gravatar.com/v3/profiles/{h}", timeout=15)
        if r.status_code == 404:
            av = session.get(f"https://gravatar.com/avatar/{h}?d=404", timeout=10)
            if av.status_code == 200:
                return result("gravatar", title, "found",
                              "Tiene foto de Gravatar (sin perfil público)",
                              links=[{"label": "Foto de perfil",
                                      "url": f"https://gravatar.com/avatar/{h}?s=400"}])
            return result("gravatar", title, "empty", "Sin perfil en Gravatar")
        r.raise_for_status()
        p = r.json()
    except Exception as e:
        return result("gravatar", title, "error", short_error(e))

    fields = {k: v for k, v in {
        "Nombre": p.get("display_name"), "Ubicación": p.get("location"),
        "Trabajo": p.get("job_title"), "Empresa": p.get("company"),
        "Bio": p.get("description"), "Pronombres": p.get("pronouns"),
    }.items() if v}
    links = [{"label": "Perfil Gravatar", "url": p.get("profile_url", "")},
             {"label": "Foto de perfil", "url": p.get("avatar_url", "")}]
    for acc in p.get("verified_accounts") or []:
        links.append({"label": acc.get("service_label", "cuenta"),
                      "url": acc.get("url", ""), "extra": "cuenta verificada"})
    for l in p.get("links") or []:
        links.append({"label": l.get("label", "enlace"), "url": l.get("url", "")})
    return result("gravatar", title, "found",
                  f"Perfil encontrado: {p.get('display_name') or '?'}",
                  fields, [l for l in links if l["url"]])


@register(name="holehe", title="Registrado en sitios (Holehe)",
          accepts=["email"])
def holehe(email: str, ctx) -> dict:
    title = "Registrado en sitios (Holehe)"
    try:
        funcs = _holehe_functions()
    except ImportError:
        return result("holehe", title, "skipped",
                      "Holehe no instalado (opcional)",
                      notes=["Instálalo con:  pip install holehe"])
    out = asyncio.run(_holehe_run(email, funcs, ctx))
    used = [o for o in out if o.get("exists")]
    limited = [o.get("domain") for o in out if o.get("rateLimit")]
    links = []
    for o in sorted(used, key=lambda o: o.get("domain", "")):
        extra = [x for x in (o.get("emailrecovery"), o.get("phoneNumber"),
                             o.get("others") and str(o["others"])) if x]
        links.append({"label": o.get("name") or o.get("domain"),
                      "url": "https://" + o.get("domain", ""),
                      "extra": " · ".join(extra)})
    fields = {"Sitios comprobados": len(out),
              "Bloqueados por rate-limit": len(limited)}
    if not used:
        return result("holehe", title, "empty",
                      "No se detectaron registros", fields)
    return result("holehe", title, "found",
                  f"Registrado en {len(used)} sitios", fields, links)


def _holehe_functions():
    import importlib
    import inspect
    import pkgutil
    import holehe.modules as pkg

    funcs = []
    for _, modname, ispkg in pkgutil.walk_packages(pkg.__path__,
                                                   prefix="holehe.modules."):
        if ispkg:
            continue
        try:
            mod = importlib.import_module(modname)
        except Exception:
            continue
        for attr in vars(mod).values():
            # Las funciones de holehe son: async def sitio(email, client, out)
            if inspect.iscoroutinefunction(attr) and \
                    getattr(attr, "__module__", "") == modname and \
                    len(inspect.signature(attr).parameters) == 3:
                funcs.append(attr)
    return funcs


async def _holehe_run(email, funcs, ctx):
    import httpx

    out, done = [], 0
    sem = asyncio.Semaphore(25)

    async def one(fn, client):
        nonlocal done
        async with sem:
            try:
                await asyncio.wait_for(fn(email, client, out), timeout=20)
            except Exception:
                pass
        done += 1
        ctx.progress(done, len(funcs))

    async with httpx.AsyncClient(timeout=15) as client:
        await asyncio.gather(*(one(f, client) for f in funcs))
    return out


@register(name="email_username", title="Cuentas con el usuario del email",
          accepts=["email"])
def email_username(email: str, ctx) -> dict:
    from modules.username import scan_usernames
    title = "Cuentas con el usuario del email"
    user = email.split("@", 1)[0]
    if len(user) < 4:
        return result("email_username", title, "skipped",
                      "Usuario demasiado corto para buscar")
    res = scan_usernames([user], ctx, "email_username", title)
    res["notes"].append(f"Se buscó '{user}' como username: puede ser otra "
                        "persona con el mismo nick.")
    return res


@register(name="email_domain", title="Dominio corporativo", accepts=["email"])
def email_domain(email: str, ctx) -> dict:
    if is_free_mail(email):
        return result("email_domain", "Dominio corporativo", "skipped",
                      "Proveedor público: no se analiza el dominio")
    from modules.domain_osint import rdap
    res = rdap(extract_domain(email), ctx)
    res["module"], res["title"] = "email_domain", "Dominio corporativo (WHOIS)"
    return res
