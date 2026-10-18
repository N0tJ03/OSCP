"""
core/registry.py — Registro modular de fuentes.
Cada fuente se registra con @register. Si una falla o no está instalada,
las demás siguen funcionando.

Cada módulo recibe (target, ctx) y devuelve un dict creado con result():
    module   nombre interno
    title    título visible
    status   found | empty | info | skipped | error
    summary  una línea con el resultado
    fields   {etiqueta: valor} datos clave
    links    [{label, url, extra}] hallazgos clicables
    notes    [str] avisos
"""
from typing import Callable, Dict, List

_MODULES: Dict[str, dict] = {}


def register(name: str, title: str, accepts: List[str], description: str = ""):
    def decorator(func: Callable):
        _MODULES[name] = {"name": name, "title": title, "func": func,
                          "accepts": accepts, "description": description}
        return func
    return decorator


def get_modules(input_type: str) -> List[dict]:
    return [m for m in _MODULES.values() if input_type in m["accepts"]]


def all_modules() -> Dict[str, dict]:
    return _MODULES


def result(module: str, title: str, status: str = "info", summary: str = "",
           fields: dict = None, links: list = None, notes: list = None) -> dict:
    return {"module": module, "title": title, "status": status,
            "summary": summary, "fields": fields or {}, "links": links or [],
            "notes": notes or []}


def short_error(e: Exception) -> str:
    """Convierte las excepciones larguísimas de requests en algo legible."""
    import requests
    if isinstance(e, requests.exceptions.Timeout):
        return "el servicio tardó demasiado en responder (timeout)"
    if isinstance(e, requests.exceptions.ConnectionError):
        msg = str(e)
        if "too many" in msg and "error responses" in msg:
            return "el servicio está caído o saturado (errores 5xx)"
        return "no se pudo conectar con el servicio"
    if isinstance(e, requests.exceptions.HTTPError) and e.response is not None:
        code = e.response.status_code
        return {401: "requiere autenticación (401)", 403: "acceso bloqueado (403)",
                429: "demasiadas peticiones, prueba en un rato (429)"}.get(
                    code, f"error HTTP {code}")
    return str(e)[:160]


class Ctx:
    """Contexto que recibe cada módulo para informar de su progreso."""

    def __init__(self, on_progress: Callable = None):
        self._cb = on_progress

    def progress(self, done: int, total: int, text: str = ""):
        if self._cb:
            self._cb(done, total, text)
