"""
modules/name_osint.py — Nombre completo: búsquedas dirigidas en buscadores y
redes con TODAS las combinaciones del nombre (Pepe Pepito García ->
"Pepe Pepito García", "Pepe García", "Pepe Pepito", ...). La búsqueda de
cuentas por username la hace modules/username.py.
"""
from urllib.parse import quote, quote_plus

from core.registry import register, result
from core.utils import linkedin_slugs, name_combos, name_variants, search_links


@register(name="name_search", title="Búsquedas del nombre", accepts=["name"])
def name_search(name: str, ctx) -> dict:
    combos = [" ".join(c) for c in name_combos(name)]
    # Google admite ~32 palabras por consulta: con 4+ partes se usan las 6 primeras
    any_name = " OR ".join(f'"{c}"' for c in combos[:6])
    links = search_links([
        ("Google: cualquier forma del nombre", any_name),
        ("Google: LinkedIn", f"({any_name}) site:linkedin.com/in"),
        ("Google: Facebook", f"({any_name}) site:facebook.com"),
        ("Google: Instagram", f"({any_name}) site:instagram.com"),
        ("Google: X / Twitter", f"({any_name}) site:x.com OR site:twitter.com"),
        ("Google: TikTok", f"({any_name}) site:tiktok.com"),
        ("Google: documentos (PDF, Excel, Word)",
         f"({any_name}) filetype:pdf OR filetype:xlsx OR filetype:docx"),
        ("Google: boletines oficiales",
         f"({any_name}) site:boe.es OR site:dof.gob.mx OR site:gob.mx"),
    ])
    for c in combos:
        links.append({"label": f'Google: "{c}"', "url": "https://www.google.com/"
                      "search?q=" + quote_plus(f'"{c}"'), "extra": "nombre exacto"})
    for slug in linkedin_slugs(name):
        links.append({"label": f"LinkedIn /in/{slug}",
                      "url": f"https://www.linkedin.com/in/{slug}",
                      "extra": "perfil probable (comprobar a mano)"})
    links += [
        {"label": "LinkedIn (buscador)",
         "url": "https://www.linkedin.com/search/results/people/?keywords=" + quote(name)},
        {"label": "Facebook (buscador)",
         "url": "https://www.facebook.com/search/people/?q=" + quote(name)},
        {"label": "Google Imágenes",
         "url": "https://www.google.com/search?tbm=isch&q=" + quote_plus(any_name)},
    ]
    fields = {"Combinaciones del nombre": "\n".join(combos),
              "Usernames que se prueban": ", ".join(name_variants(name, 8))}
    return result("name_search", "Búsquedas del nombre", "info",
                  f"{len(combos)} combinaciones · {len(links)} búsquedas preparadas",
                  fields, links,
                  ["LinkedIn exige iniciar sesión, así que los perfiles /in/ "
                   "no se pueden comprobar automáticamente: ábrelos a mano."])
