"""
core/utils.py — Utilidades compartidas.

FREE_MAIL_DOMAINS: dominios de proveedores públicos. Para user@gmail.com el
dominio es de Google, no del objetivo, así que los módulos de dominio lo saltan.
"""
import re
import unicodedata
from urllib.parse import quote_plus

FREE_MAIL_DOMAINS = {
    "gmail.com", "googlemail.com", "yahoo.com", "yahoo.es", "yahoo.co.uk",
    "yahoo.com.mx", "hotmail.com", "hotmail.es", "outlook.com", "outlook.es",
    "live.com", "live.com.mx", "msn.com", "aol.com", "icloud.com", "me.com",
    "mac.com", "protonmail.com", "proton.me", "pm.me", "gmx.com", "gmx.es",
    "gmx.de", "web.de", "mail.ru", "yandex.ru", "yandex.com", "zoho.com",
    "tutanota.com", "tuta.io", "fastmail.com", "hushmail.com", "qq.com",
    "163.com", "126.com",
}

EMAIL_RE = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")


def extract_domain(target: str) -> str:
    return target.split("@")[-1].strip().lower() if "@" in target \
        else target.strip().lower()


def is_free_mail(target: str) -> bool:
    return "@" in target and extract_domain(target) in FREE_MAIL_DOMAINS


PARTICLES = {"de", "del", "la", "las", "los", "y", "da", "do", "dos", "van", "von"}


def _strip_accents(s: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFKD", s)
                   if not unicodedata.combining(c))


def name_parts(name: str) -> list:
    """'Pepe Pepito García de la Torre' -> ['Pepe','Pepito','García','Torre']
    (sin partículas; conserva tildes y mayúsculas para mostrar)."""
    return [p for p in re.split(r"\s+", name.strip())
            if p and p.lower() not in PARTICLES]


def name_combos(name: str) -> list:
    """Todas las formas en que alguien puede usar su nombre, de más a menos
    completa. Pepe Pepito García López ->
      [Pepe Pepito García López], [Pepe García López], [Pepe Pepito García],
      [Pepe García], [Pepe López], [Pepe Pepito], [Pepito García López], ...
    Siempre empiezan por el primer nombre o (si hay 3+ partes) por el segundo,
    y conservan el orden original."""
    parts = name_parts(name)
    if len(parts) < 2:
        return [parts] if parts else []
    starts = [0, 1] if len(parts) >= 3 else [0]
    combos = []
    for s in starts:
        rest = parts[s + 1:]
        for mask in range(1, 2 ** len(rest)):
            combo = [parts[s]] + [rest[i] for i in range(len(rest)) if mask >> i & 1]
            combos.append(combo)
    # Prioridad: primer nombre antes que segundo; con 4+ partes, las que llevan
    # el primer apellido (parts[2]); luego más partes. Con 3 partes no se sabe
    # si la segunda es nombre o apellido, así que no se prioriza ninguna.
    key_surname = parts[2] if len(parts) >= 4 else None
    combos.sort(key=lambda c: (c[0] != parts[0],
                               key_surname is not None and key_surname not in c,
                               -len(c)))
    return combos


def name_variants(name: str, limit: int = 10) -> list:
    """Usernames probables a partir de un nombre completo, sin tildes.
    Pepe Pepito García -> pepepepitogarcia, pepegarcia, pepe.garcia,
    pepe.pepito.garcia, pgarcia, pepepepito, pepitogarcia, ..."""
    combos = [[_strip_accents(p).lower() for p in c] for c in name_combos(name)]
    combos = [[re.sub(r"[^a-z0-9]", "", p) for p in c] for c in combos]
    if not combos:
        return []
    if len(combos[0]) == 1:
        return combos[0]
    ordered = []
    for i, c in enumerate(combos):
        ordered.append("".join(c))
        if i < 3:                              # las 3 más probables, con punto
            ordered.append(".".join(c))
        if len(c) == 2 and c[0] == combos[0][0]:
            ordered.append(c[0][0] + c[1])              # pgarcia
    ordered += ["_".join(combos[0])]
    out = []
    for v in ordered:
        if v and v not in out:
            out.append(v)
    return out[:limit]


def linkedin_slugs(name: str) -> list:
    """LinkedIn usa slugs con guiones: /in/pepe-pepito-garcia"""
    return ["-".join(re.sub(r"[^a-z0-9]", "", _strip_accents(p).lower())
                     for p in c) for c in name_combos(name)]


def google(q: str) -> str:
    return "https://www.google.com/search?q=" + quote_plus(q)


def search_links(queries: list) -> list:
    """[(label, query)] -> links de búsqueda en Google."""
    return [{"label": label, "url": google(q), "extra": q} for label, q in queries]
