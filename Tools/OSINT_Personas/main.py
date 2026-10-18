#!/usr/bin/env python3
"""
OSINT TOOL — Recolector modular de información de fuentes abiertas
(uso educativo / investigación autorizada). Solo fuentes GRATUITAS sin
API keys; herramientas externas (holehe, sherlock, maigret) opcionales.

Uso:
    python main.py                      menú interactivo
    python main.py user@example.com     detecta el tipo automáticamente
    python main.py -t name "Juan Pérez" fuerza el tipo
"""
import argparse
import re
import sys
import time
import webbrowser
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

try:
    from rich.panel import Panel
    from rich.progress import (BarColumn, Progress, SpinnerColumn,
                               TextColumn, TimeElapsedColumn)
    from rich.prompt import Confirm, Prompt
except ImportError:
    sys.exit("Falta 'rich'. Instala dependencias con:\n"
             "    pip install -r requirements.txt")

from core import render, report                       # noqa: E402
from core.registry import Ctx, get_modules, result, short_error  # noqa: E402
from core.render import console                       # noqa: E402
from core.utils import EMAIL_RE                        # noqa: E402

# Importar los módulos ejecuta sus @register
import modules.username        # noqa: E402,F401
import modules.email_osint     # noqa: E402,F401
import modules.domain_osint    # noqa: E402,F401
import modules.phone_osint     # noqa: E402,F401
import modules.name_osint      # noqa: E402,F401
import modules.external_tools  # noqa: E402,F401

BANNER = r"""[bold cyan]
   ___  ____ ___ _   _ _____   _____ ___   ___  _
  / _ \/ ___|_ _| \ | |_   _| |_   _/ _ \ / _ \| |
 | | | \___ \| ||  \| | | |     | || | | | | | | |
 | |_| |___) | || |\  | | |     | || |_| | |_| | |___
  \___/|____/___|_| \_| |_|     |_| \___/ \___/|_____|[/]
 [bright_black]fuentes abiertas · gratis · sin API keys[/]
"""

TYPES = {"1": ("email", "Email", "usuario@ejemplo.com"),
         "2": ("username", "Username", "johndoe"),
         "3": ("name", "Nombre completo", "Juan Pérez"),
         "4": ("phone", "Teléfono", "+34600123456"),
         "5": ("domain", "Dominio", "ejemplo.com")}
TYPE_NAMES = {t[0]: t[1] for t in TYPES.values()}


def detect_type(value: str) -> str:
    v = value.strip()
    if EMAIL_RE.match(v):
        return "email"
    if re.fullmatch(r"\+?[\d\s().-]{7,}", v):
        return "phone"
    if " " in v:
        return "name"
    if re.fullmatch(r"[a-z0-9-]+(\.[a-z0-9-]+)*\.[a-z]{2,}", v.lower()):
        return "domain"
    return "username"


def validate(itype: str, value: str):
    """Devuelve el valor normalizado o None con un mensaje de error."""
    v = value.strip()
    if not v:
        return None, "Entrada vacía."
    if itype == "email" and not EMAIL_RE.match(v):
        return None, "Formato de email no válido."
    if itype == "phone":
        v = re.sub(r"[\s().-]", "", v)
        if not v.startswith("+"):
            return None, "Usa formato internacional con prefijo, ej. +34600123456"
    if itype == "domain":
        v = re.sub(r"^https?://", "", v.lower()).split("/")[0].removeprefix("www.")
        if "." not in v:
            return None, "Dominio no válido."
    if itype == "name" and " " not in v:
        return None, "Escribe nombre y apellido."
    return v, None


def run_collection(itype: str, target: str) -> list:
    mods = get_modules(itype)
    console.print(f"\n[bold]Objetivo:[/] [cyan]{target}[/]  ·  "
                  f"[bold]Tipo:[/] {TYPE_NAMES[itype]}  ·  "
                  f"{len(mods)} fuentes en paralelo\n")
    progress = Progress(SpinnerColumn(), TextColumn("[bold]{task.description:<38}"),
                        BarColumn(bar_width=28), TextColumn("{task.fields[info]}"),
                        TimeElapsedColumn(), console=console)
    results = [None] * len(mods)

    def worker(i, mod, task):
        def on_progress(done, total, text=""):
            progress.update(task, completed=done, total=total,
                            info=text or f"{done}/{total}")
        try:
            res = mod["func"](target, Ctx(on_progress))
        except Exception as e:            # una fuente nunca tumba el resto
            res = result(mod["name"], mod["title"], "error", f"Fallo inesperado: {short_error(e)}")
        color, label = render.STATUS.get(res["status"], ("white", res["status"]))
        progress.update(task, total=1, completed=1,
                        info=f"[{color}]{label}[/] {res['summary'][:40]}")
        results[i] = res

    with progress:
        tasks = [progress.add_task(m["title"], total=None, info="")
                 for m in mods]
        with ThreadPoolExecutor(len(mods)) as pool:
            for i, (m, t) in enumerate(zip(mods, tasks)):
                pool.submit(worker, i, m, t)
    return results


def investigate(itype: str, target: str, open_browser: bool = True):
    start = time.time()
    results = run_collection(itype, target)
    secs = time.time() - start
    render.show_results(results, target, itype, secs)

    outdir = ROOT / "reports"
    outdir.mkdir(exist_ok=True)
    safe = re.sub(r"[^\w.@+-]+", "_", target)[:40]
    base = outdir / f"{datetime.now():%Y%m%d_%H%M%S}_{safe}"
    html = report.save_html(results, target, itype, secs, base.with_suffix(".html"))
    js = report.save_json(results, target, itype, base.with_suffix(".json"))
    console.print(Panel(f"[bold green]Reporte HTML:[/] [link={html.as_uri()}]{html}[/link]\n"
                        f"[bold]Datos JSON:[/]  {js}", border_style="green"))
    if open_browser:
        webbrowser.open(html.as_uri())


def menu():
    while True:
        console.clear()
        console.print(BANNER)
        for k, (_, label, ex) in TYPES.items():
            console.print(f"  [bold cyan]{k}[/]) {label:<17} [bright_black]{ex}[/]")
        console.print("  [bold cyan]6[/]) Detectar automáticamente")
        console.print("  [bold cyan]0[/]) Salir\n")
        choice = Prompt.ask("Opción", choices=list("0123456"), default="6")
        if choice == "0":
            break
        if choice == "6":
            raw = Prompt.ask("Introduce email, username, nombre, teléfono o dominio")
            itype = detect_type(raw)
            console.print(f"[bright_black]→ detectado como:[/] {TYPE_NAMES[itype]}")
        else:
            itype, label, ex = TYPES[choice]
            raw = Prompt.ask(f"{label} [bright_black](ej. {ex})[/]")
        target, err = validate(itype, raw)
        if err:
            console.print(f"[red]✖ {err}[/]")
            Prompt.ask("Pulsa ENTER para continuar", default="", show_default=False)
            continue
        investigate(itype, target)
        if not Confirm.ask("\n¿Nueva investigación?", default=True):
            break
    console.print("\n[cyan]Hasta luego.[/]")


def main():
    ap = argparse.ArgumentParser(description="OSINT tool (fuentes gratuitas)")
    ap.add_argument("target", nargs="?", help="email, username, nombre, teléfono o dominio")
    ap.add_argument("-t", "--type", choices=list(TYPE_NAMES), help="forzar el tipo")
    ap.add_argument("--no-open", action="store_true", help="no abrir el navegador")
    args = ap.parse_args()
    if not args.target:
        try:
            menu()
        except (KeyboardInterrupt, EOFError):
            console.print("\n[cyan]Hasta luego.[/]")
        return
    itype = args.type or detect_type(args.target)
    target, err = validate(itype, args.target)
    if err:
        sys.exit(f"Error: {err}")
    investigate(itype, target, not args.no_open)


if __name__ == "__main__":
    main()
