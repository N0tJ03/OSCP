"""
modules/phone_osint.py — Análisis de teléfono con la librería phonenumbers
(offline, gratis: pip install phonenumbers) + enlaces de búsqueda.
"""
from core.registry import register, result
from core.utils import search_links

try:
    import phonenumbers
    from phonenumbers import geocoder, carrier, timezone, PhoneNumberFormat
    HAVE_PHONENUMBERS = True
except ImportError:
    HAVE_PHONENUMBERS = False

TYPES = {0: "Fijo", 1: "Móvil", 2: "Fijo o móvil", 3: "Gratuito",
         4: "Tarificación especial", 5: "Coste compartido", 6: "VoIP",
         7: "Número personal", 8: "Busca", 9: "UAN", 10: "Buzón de voz",
         99: "Desconocido"}


@register(name="phone_analysis", title="Análisis del teléfono",
          accepts=["phone"],
          description="País, operador, zona, tipo y validez (offline)")
def phone_analysis(number: str, ctx) -> dict:
    title = "Análisis del teléfono"
    if not HAVE_PHONENUMBERS:
        return result("phone_analysis", title, "error",
                      "phonenumbers no instalado: pip install phonenumbers")
    try:
        p = phonenumbers.parse(number, None)
    except Exception as e:
        return result("phone_analysis", title, "error",
                      f"{e} (usa formato internacional, ej. +34600123456)")
    if not phonenumbers.is_valid_number(p):
        return result("phone_analysis", title, "empty", "Número NO válido",
                      {"Número": number, "Válido": "No"})

    e164 = phonenumbers.format_number(p, PhoneNumberFormat.E164)
    intl = phonenumbers.format_number(p, PhoneNumberFormat.INTERNATIONAL)
    national = phonenumbers.format_number(p, PhoneNumberFormat.NATIONAL)
    digits = e164.lstrip("+")
    fields = {"Número": intl, "Válido": "Sí",
              "País": geocoder.country_name_for_number(p, "es"),
              "Zona": geocoder.description_for_number(p, "es") or "—",
              "Operador": carrier.name_for_number(p, "es") or "desconocido",
              "Tipo": TYPES.get(phonenumbers.number_type(p), "Desconocido"),
              "Zona horaria": ", ".join(timezone.time_zones_for_number(p)),
              "Formatos": f"{e164}  ·  {national}"}
    links = [{"label": "Abrir chat de WhatsApp", "url": f"https://wa.me/{digits}"},
             {"label": "Telegram", "url": f"https://t.me/+{digits}"}]
    links += search_links([
        ("Google: número (todos los formatos)",
         f'"{e164}" OR "{intl}" OR "{national}" OR "{digits}"'),
        ("Google: en Facebook / Instagram",
         f'"{national}" OR "{digits}" site:facebook.com OR site:instagram.com'),
        ("Google: anuncios y directorios",
         f'"{national}" OR "{digits}" (anuncio OR contacto OR telefono)'),
    ])
    return result("phone_analysis", title, "found",
                  f"{fields['País']} · {fields['Tipo']} · {fields['Operador']}",
                  fields, links,
                  ["El operador es el original; si se hizo portabilidad "
                   "puede ser otro."])
