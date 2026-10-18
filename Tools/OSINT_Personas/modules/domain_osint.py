"""
modules/domain_osint.py — Información de dominios con servicios gratuitos:
  - RDAP (WHOIS moderno): registrador, fechas, estado
  - DNS vía Google DNS-over-HTTPS: A, AAAA, MX, NS, TXT, DMARC
  - crt.sh: subdominios por certificados (Certificate Transparency)
  - HIBP: filtraciones públicas conocidas de ese dominio
"""
import time

from core.http import session
from core.registry import register, result, short_error
from core.utils import extract_domain, search_links


_rdap_servers = {}


def _rdap_base(tld: str):
    """Servidor RDAP oficial del TLD según el registro de IANA."""
    if not _rdap_servers:
        r = session.get("https://data.iana.org/rdap/dns.json", timeout=20)
        r.raise_for_status()
        for tlds, urls in r.json()["services"]:
            for t in tlds:
                _rdap_servers[t] = urls[0].rstrip("/") + "/"
    return _rdap_servers.get(tld)


@register(name="rdap", title="WHOIS / RDAP", accepts=["domain"])
def rdap(domain: str, ctx) -> dict:
    title = "WHOIS / RDAP"
    d = extract_domain(domain)
    data, err = None, None
    try:
        base = _rdap_base(d.rsplit(".", 1)[-1])
    except Exception:
        base = None
    for url in filter(None, [base and f"{base}domain/{d}",
                             f"https://rdap.org/domain/{d}"]):
        try:
            r = session.get(url, timeout=15)
            if r.status_code == 404:
                return result("rdap", title, "empty",
                              "Dominio no registrado")
            r.raise_for_status()
            data = r.json()
            break
        except Exception as e:
            err = e
    if data is None:
        return result("rdap", title, "error", short_error(err) if err else
                      "Este TLD no tiene servicio RDAP público")

    events = {e.get("eventAction"): e.get("eventDate", "")[:10]
              for e in data.get("events", [])}
    fields = {"Dominio": data.get("ldhName", d).lower(),
              "Creado": events.get("registration"),
              "Expira": events.get("expiration"),
              "Último cambio": events.get("last changed"),
              "Estado": ", ".join(data.get("status", []))}
    for ent in data.get("entities", []):
        roles = ent.get("roles", [])
        vcard = (ent.get("vcardArray") or [None, []])[1]
        name = next((v[3] for v in vcard if v[0] in ("fn", "org") and v[3]), None)
        if name and "registrar" in roles:
            fields["Registrador"] = name
        elif name and "registrant" in roles:
            fields["Titular"] = name
    fields["Nameservers"] = ", ".join(n.get("ldhName", "").lower()
                                      for n in data.get("nameservers", []))
    fields = {k: v for k, v in fields.items() if v}
    links = [{"label": "Web", "url": f"https://{d}"},
             {"label": "Archivo histórico (Wayback)",
              "url": f"https://web.archive.org/web/*/{d}"}]
    links += search_links([("Google: páginas indexadas", f"site:{d}"),
                           ("Google: emails del dominio", f'"@{d}"')])
    return result("rdap", title, "found",
                  f"Registrado desde {fields.get('Creado', '?')} · "
                  f"{fields.get('Registrador', 'registrador ?')}", fields, links)


@register(name="dns_lookup", title="Registros DNS", accepts=["domain"])
def dns_lookup(domain: str, ctx) -> dict:
    d = extract_domain(domain)
    fields = {}

    def q(name, rtype):
        try:
            r = session.get("https://dns.google/resolve",
                            params={"name": name, "type": rtype}, timeout=10)
            return [a.get("data", "") for a in r.json().get("Answer", [])
                    if a.get("type") != 5]          # ignora CNAME intermedios
        except Exception:
            return []

    for rtype in ["A", "AAAA", "MX", "NS", "TXT"]:
        vals = q(d, rtype)
        if vals:
            fields[rtype] = "\n".join(vals)
    dmarc = q(f"_dmarc.{d}", "TXT")
    if dmarc:
        fields["DMARC"] = "\n".join(dmarc)
    if not fields:
        return result("dns_lookup", "Registros DNS", "empty",
                      "El dominio no resuelve")
    # Servicios que usa la organización, deducidos de verificaciones TXT / MX
    txt = (fields.get("TXT", "") + fields.get("MX", "")).lower()
    services = [name for marker, name in [
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
    ] if marker in txt]
    summary = f"{len(fields)} tipos de registro"
    if services:
        fields = {"Servicios detectados": ", ".join(dict.fromkeys(services)),
                  **fields}
        summary += f" · usa {len(set(services))} servicios conocidos"
    return result("dns_lookup", "Registros DNS", "found", summary, fields)


@register(name="crtsh", title="Subdominios (certificados)", accepts=["domain"])
def crtsh(domain: str, ctx) -> dict:
    title = "Subdominios (certificados)"
    d = extract_domain(domain)
    raw, source = None, "crt.sh"
    for attempt in range(2):          # crt.sh suele dar 502 bajo carga
        try:
            r = session.get("https://crt.sh/", params={"q": f"%.{d}",
                            "output": "json"}, timeout=40)
            r.raise_for_status()
            raw = [n for e in r.json() for n in e.get("name_value", "").split("\n")]
            break
        except Exception:
            time.sleep(3)
    if raw is None:                   # respaldo: CertSpotter (gratis, sin key)
        source = "CertSpotter"
        try:
            raw = _certspotter(d)
        except Exception as e:
            return result("crtsh", title, "error",
                          f"crt.sh y CertSpotter no responden ({short_error(e)})")
    names = set()
    for n in raw:
        n = n.strip().lower().lstrip("*.")
        if n.endswith("." + d):
            names.add(n)
    if not names:
        return result("crtsh", title, "empty", "Sin subdominios en certificados")
    links = [{"label": n, "url": f"https://{n}"} for n in sorted(names)[:200]]
    return result("crtsh", title, "found", f"{len(names)} subdominios",
                  {"Total": len(names), "Fuente": source}, links,
                  ["Mostrando los primeros 200."] if len(names) > 200 else [])


def _certspotter(d: str, pages: int = 5) -> list:
    names, after = [], None
    for _ in range(pages):
        params = {"domain": d, "include_subdomains": "true",
                  "expand": "dns_names"}
        if after:
            params["after"] = after
        r = session.get("https://api.certspotter.com/v1/issuances",
                        params=params, timeout=30)
        r.raise_for_status()
        batch = r.json()
        if not batch:
            break
        for c in batch:
            names += c.get("dns_names", [])
        after = batch[-1]["id"]
    return names


@register(name="hibp_domain", title="Filtraciones del dominio (HIBP)",
          accepts=["domain"])
def hibp_domain(domain: str, ctx) -> dict:
    title = "Filtraciones del dominio (HIBP)"
    d = extract_domain(domain)
    try:
        r = session.get("https://haveibeenpwned.com/api/v3/breaches", timeout=25)
        r.raise_for_status()
        hits = [b for b in r.json() if (b.get("Domain") or "").lower() == d]
    except Exception as e:
        return result("hibp_domain", title, "error", short_error(e))
    if not hits:
        return result("hibp_domain", title, "empty",
                      "Este servicio no tiene filtraciones públicas conocidas")
    links = [{"label": f"{b['Title']} ({b['BreachDate']})",
              "url": f"https://haveibeenpwned.com/PwnedWebsites#{b['Name']}",
              "extra": f"{b['PwnCount']:,} cuentas · " + ", ".join(b["DataClasses"])}
             for b in hits]
    return result("hibp_domain", title, "found",
                  f"{len(hits)} filtraciones del servicio", links=links)
