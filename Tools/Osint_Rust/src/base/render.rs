//! base/render.rs — Salida en terminal: tabla resumen y paneles por fuente.
//! Las URLs se muestran completas para que el terminal las haga clicables.

use console::{measure_text_width, pad_str, style, truncate_str, Alignment, Style, Term};

use super::registry::{ModResult, Status};

pub const MAX_LINKS: usize = 40;

pub fn status_style(s: Status) -> (Style, &'static str) {
    match s {
        Status::Found => (Style::new().green(), "✔ RESULTADOS"),
        Status::Info => (Style::new().cyan(), "ℹ INFO"),
        Status::Empty => (Style::new().yellow(), "○ SIN RESULTADOS"),
        Status::Skipped => (Style::new().black().bright(), "⏭ OMITIDO"),
        Status::Error => (Style::new().red(), "✖ ERROR"),
    }
}

pub fn status_label(s: Status) -> String {
    let (st, label) = status_style(s);
    st.apply_to(label).to_string()
}

fn width() -> usize {
    (Term::stdout().size().1 as usize).clamp(60, 160)
}

/// Rellena o recorta (respetando colores ANSI) a un ancho visible exacto.
fn cell(s: &str, w: usize) -> String {
    pad_str(&truncate_str(s, w, "…"), w, Alignment::Left, None).into_owned()
}

fn sorted(results: &[ModResult]) -> Vec<&ModResult> {
    let mut v: Vec<&ModResult> = results.iter().collect();
    v.sort_by_key(|r| r.status.order());
    v
}

pub fn summary_table(results: &[ModResult], target: &str, itype: &str, secs: f64) {
    let total = width();
    let w_src = results.iter().map(|r| measure_text_width(&r.title)).max().unwrap_or(6).clamp(6, 34);
    let w_st = 16;
    let w_res = total.saturating_sub(w_src + w_st + 10).max(10);
    let line = |l: &str, m: &str, r: &str| {
        format!("{l}{}{m}{}{m}{}{r}", "─".repeat(w_src + 2), "─".repeat(w_st + 2), "─".repeat(w_res + 2))
    };
    let title = format!("Resumen · {target} ({itype}) · {secs:.0}s");
    println!("{}", style(pad_str(&title, total, Alignment::Center, None)).bold());
    println!("{}", line("╭", "┬", "╮"));
    println!(
        "│ {} │ {} │ {} │",
        style(cell("Fuente", w_src)).bold(),
        style(cell("Estado", w_st)).bold(),
        style(cell("Resultado", w_res)).bold()
    );
    println!("{}", line("├", "┼", "┤"));
    for r in sorted(results) {
        println!(
            "│ {} │ {} │ {} │",
            style(cell(&r.title, w_src)).bold(),
            cell(&status_label(r.status), w_st),
            cell(&r.summary, w_res)
        );
    }
    println!("{}", line("╰", "┴", "╯"));
}

pub fn result_panel(r: &ModResult) {
    let (color, label) = status_style(r.status);
    let total = width();
    let bar = color.apply_to("│").to_string();
    let mut lines: Vec<String> = vec![color.clone().bold().apply_to(&r.summary).to_string()];

    if !r.fields.is_empty() {
        lines.push(String::new());
        let kw = r.fields.iter().map(|(k, _)| measure_text_width(k)).max().unwrap_or(0);
        for (k, v) in r.fields.iter() {
            let mut vl = v.lines();
            let first = vl.next().unwrap_or("");
            lines.push(format!("{}  {first}", style(cell(k, kw)).cyan().bold()));
            for more in vl {
                lines.push(format!("{}  {more}", " ".repeat(kw)));
            }
        }
    }

    if !r.links.is_empty() {
        lines.push(String::new());
        let shown = &r.links[..r.links.len().min(MAX_LINKS)];
        let lw = shown.iter().map(|l| measure_text_width(&l.label)).max().unwrap_or(5).clamp(5, 32);
        lines.push(format!(
            "{}  {}  {}",
            style(cell("#", 3)).dim(),
            style(cell("Sitio / hallazgo", lw)).bold(),
            style("Enlace · Detalle").bold()
        ));
        for (i, l) in shown.iter().enumerate() {
            let mut row = format!(
                "{}  {}  {}",
                style(cell(&(i + 1).to_string(), 3)).dim(),
                style(cell(&l.label, lw)).bold(),
                style(&l.url).blue().underlined()
            );
            if !l.extra.is_empty() {
                row.push_str(&format!("  {}", style(&l.extra).dim()));
            }
            lines.push(row);
        }
        if r.links.len() > MAX_LINKS {
            lines.push(style(format!("... y {} más en el reporte HTML", r.links.len() - MAX_LINKS)).italic().to_string());
        }
    }

    for n in r.notes.iter().filter(|n| !n.is_empty()) {
        lines.push(style(format!("⚠ {n}")).yellow().to_string());
    }

    let head = format!("{} {} ", color.apply_to("╭─"), style(&r.title).bold());
    let tail = format!(" {} {}", color.apply_to(label), color.apply_to("─"));
    let fill = total.saturating_sub(measure_text_width(&head) + measure_text_width(&tail));
    println!("{head}{}{tail}", color.apply_to("─".repeat(fill)));
    for l in lines {
        println!("{bar} {l}");
    }
    println!("{}", color.apply_to(format!("╰{}", "─".repeat(total.saturating_sub(1)))));
}

pub fn show_results(results: &[ModResult], target: &str, itype: &str, secs: f64) {
    println!();
    summary_table(results, target, itype, secs);
    println!();
    for r in sorted(results) {
        if matches!(r.status, Status::Found | Status::Info) {
            result_panel(r);
        }
    }
    for r in results {
        if !matches!(r.status, Status::Found | Status::Info) && !r.notes.is_empty() {
            result_panel(r);
        }
    }
}
