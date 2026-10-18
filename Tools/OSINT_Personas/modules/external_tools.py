"""
modules/external_tools.py — Herramientas externas OPCIONALES. Si no están
instaladas el módulo aparece como "omitido" y todo lo demás sigue igual.
    pip install sherlock-project maigret
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile

from core.registry import register, result
from core.utils import name_variants

FOUND_RE = re.compile(r"\[\+\]\s*([^:]+):\s*(https?://\S+)")


def _find_tool(tool: str):
    """Busca en PATH y junto al Python que se está usando (venv sin activar)."""
    path = shutil.which(tool)
    if path:
        return path
    bindir = os.path.dirname(sys.executable)
    for name in (tool, tool + ".exe"):
        for d in (bindir, os.path.join(bindir, "Scripts")):
            cand = os.path.join(d, name)
            if os.path.isfile(cand):
                return cand
    return None


def _username(target: str) -> str:
    t = target.strip().lstrip("@")
    return name_variants(t, 1)[0] if " " in t else t


def _run_tool(module, title, tool, args, target):
    binary = _find_tool(tool)
    if not binary:
        return result(module, title, "skipped", f"{tool} no instalado (opcional)",
                      notes=[f"Instálalo con:  pip install "
                             f"{'sherlock-project' if tool == 'sherlock' else tool}"])
    user = _username(target)
    with tempfile.TemporaryDirectory() as tmp:
        try:
            proc = subprocess.run([binary, user, *args], capture_output=True,
                                  text=True, encoding="utf-8", errors="replace",
                                  timeout=420, cwd=tmp)
        except subprocess.TimeoutExpired:
            return result(module, title, "error", "Tardó demasiado (>7 min)")
        except Exception as e:
            return result(module, title, "error", str(e))
    out = re.sub(r"\x1b\[[0-9;]*m", "", proc.stdout + "\n" + proc.stderr)
    seen, links = set(), []
    for site, url in FOUND_RE.findall(out):
        if url not in seen:
            seen.add(url)
            links.append({"label": site.strip(), "url": url, "extra": f"@{user}"})
    if not links:
        return result(module, title, "empty", f"Sin cuentas para @{user}")
    return result(module, title, "found", f"{len(links)} cuentas para @{user}",
                  links=links)


@register(name="sherlock", title="Sherlock (400+ redes)",
          accepts=["username", "name"])
def sherlock(target: str, ctx) -> dict:
    return _run_tool("sherlock", "Sherlock (400+ redes)", "sherlock",
                     ["--print-found", "--timeout", "15", "--no-color",
                      "--no-txt"], target)


@register(name="maigret", title="Maigret (top 500 sitios)",
          accepts=["username", "name"])
def maigret(target: str, ctx) -> dict:
    return _run_tool("maigret", "Maigret (top 500 sitios)", "maigret",
                     ["--no-color", "--no-progressbar", "--timeout", "15",
                      "--top-sites", "500"], target)
