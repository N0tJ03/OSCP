"""
core/render.py — Salida en terminal con rich: paneles por fuente, tablas y
enlaces clicables (en terminales que lo soportan: Windows Terminal, etc.).
"""
from rich import box
from rich.console import Console, Group
from rich.markup import escape
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

console = Console(highlight=False)

STATUS = {
    "found":   ("green",  "✔ RESULTADOS"),
    "info":    ("cyan",   "ℹ INFO"),
    "empty":   ("yellow", "○ SIN RESULTADOS"),
    "skipped": ("bright_black", "⏭ OMITIDO"),
    "error":   ("red",    "✖ ERROR"),
}
ORDER = {"found": 0, "info": 1, "empty": 2, "skipped": 3, "error": 4}
MAX_LINKS = 40


def summary_table(results: list, target: str, itype: str, secs: float):
    t = Table(box=box.ROUNDED, expand=True, title_style="bold",
              title=f"Resumen · {escape(target)} ({itype}) · {secs:.0f}s")
    t.add_column("Fuente", style="bold")
    t.add_column("Estado", no_wrap=True)
    t.add_column("Resultado")
    for r in sorted(results, key=lambda r: ORDER.get(r["status"], 9)):
        color, label = STATUS.get(r["status"], ("white", r["status"]))
        t.add_row(escape(r["title"]), f"[{color}]{label}[/]",
                  escape(r["summary"]))
    console.print(t)


def result_panel(r: dict):
    color, label = STATUS.get(r["status"], ("white", r["status"]))
    parts = [Text(r["summary"], style=f"bold {color}")]

    if r["fields"]:
        ft = Table.grid(padding=(0, 2))
        ft.add_column(style="bold cyan", no_wrap=True)
        ft.add_column()
        for k, v in r["fields"].items():
            ft.add_row(escape(str(k)), escape(str(v)))
        parts += [Text(""), ft]

    links = r["links"]
    if links:
        lt = Table(box=box.SIMPLE_HEAD, expand=True, show_edge=False)
        lt.add_column("#", style="bright_black", width=3)
        lt.add_column("Sitio / hallazgo", style="bold")
        lt.add_column("Enlace", overflow="fold")
        lt.add_column("Detalle", style="bright_black", overflow="fold")
        for i, l in enumerate(links[:MAX_LINKS], 1):
            url = l.get("url") or ""
            shown = url if len(url) < 70 else url[:67] + "..."
            cell = f"[link={url}][blue underline]{escape(shown)}[/][/link]" \
                if url else ""
            lt.add_row(str(i), escape(l["label"]), cell,
                       escape(l.get("extra") or ""))
        parts += [Text(""), lt]
        if len(links) > MAX_LINKS:
            parts.append(Text(f"... y {len(links) - MAX_LINKS} más en el "
                              "reporte HTML", style="italic"))

    for n in r["notes"]:
        if n:
            parts.append(Text(f"⚠ {n}", style="yellow"))

    console.print(Panel(Group(*parts), border_style=color,
                        title=f"[bold]{escape(r['title'])}[/]",
                        subtitle=f"[{color}]{label}[/]",
                        title_align="left", subtitle_align="right"))


def show_results(results: list, target: str, itype: str, secs: float):
    console.print()
    summary_table(results, target, itype, secs)
    console.print()
    for r in sorted(results, key=lambda r: ORDER.get(r["status"], 9)):
        if r["status"] in ("found", "info"):
            result_panel(r)
    rest = [r for r in results if r["status"] not in ("found", "info")]
    for r in rest:
        if r["notes"]:
            result_panel(r)
