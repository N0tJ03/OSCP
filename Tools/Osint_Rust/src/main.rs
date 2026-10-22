//! OSINT TOOL — Recolector modular de información de fuentes abiertas
//! (uso educativo / investigación autorizada). Solo fuentes GRATUITAS sin
//! API keys; herramientas externas (holehe, sherlock, maigret) opcionales.
//!
//! Uso:
//!     osint_v2_rust                        menú interactivo
//!     osint_v2_rust user@example.com       detecta el tipo automáticamente
//!     osint_v2_rust -t name "Juan Pérez"   fuerza el tipo

mod base;
mod modules;

use std::any::Any;
use std::fs;
use std::io::{self, BufRead, Write};
use std::panic::{self, AssertUnwindSafe};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use clap::Parser;
use console::{style, Term};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use regex::Regex;

use base::registry::{get_modules, result, Ctx, ModResult, Status};
use base::render::{self, status_label};
use base::utils::{root, take_chars, EMAIL_RE};
use base::report;

const BANNER: &str = r"
   ___  ____ ___ _   _ _____   _____ ___   ___  _
  / _ \/ ___|_ _| \ | |_   _| |_   _/ _ \ / _ \| |
 | | | \___ \| ||  \| | | |     | || | | | | | | |
 | |_| |___) | || |\  | | |     | || |_| | |_| | |___
  \___/|____/___|_| \_| |_|     |_| \___/ \___/|_____|";

/// (opción del menú, tipo interno, nombre visible, ejemplo)
const TYPES: [(&str, &str, &str, &str); 5] = [
    ("1", "email", "Email", "usuario@ejemplo.com"),
    ("2", "username", "Username", "johndoe"),
    ("3", "name", "Nombre completo", "Juan Pérez"),
    ("4", "phone", "Teléfono", "+34600123456"),
    ("5", "domain", "Dominio", "ejemplo.com"),
];

fn type_name(itype: &str) -> &'static str {
    TYPES.iter().find(|t| t.1 == itype).map_or("?", |t| t.2)
}

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\+?[\d\s().-]{7,}$").unwrap());
static DOMAIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9-]+(\.[a-z0-9-]+)*\.[a-z]{2,}$").unwrap());
static PHONE_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s().-]").unwrap());
static SCHEME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://").unwrap());
static UNSAFE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w.@+-]+").unwrap());

fn detect_type(value: &str) -> &'static str {
    let v = value.trim();
    if EMAIL_RE.is_match(v) {
        "email"
    } else if PHONE_RE.is_match(v) {
        "phone"
    } else if v.contains(' ') {
        "name"
    } else if DOMAIN_RE.is_match(&v.to_lowercase()) {
        "domain"
    } else {
        "username"
    }
}

/// Devuelve el valor normalizado o un mensaje de error.
fn validate(itype: &str, value: &str) -> Result<String, String> {
    let mut v = value.trim().to_string();
    if v.is_empty() {
        return Err("Entrada vacía.".into());
    }
    match itype {
        "email" if !EMAIL_RE.is_match(&v) => return Err("Formato de email no válido.".into()),
        "phone" => {
            v = PHONE_STRIP_RE.replace_all(&v, "").into_owned();
            if !v.starts_with('+') {
                return Err("Usa formato internacional con prefijo, ej. +34600123456".into());
            }
        }
        "domain" => {
            let lower = v.to_lowercase();
            let no_scheme = SCHEME_RE.replace(&lower, "");
            let host = no_scheme.split('/').next().unwrap_or_default();
            v = host.strip_prefix("www.").unwrap_or(host).to_string();
            if !v.contains('.') {
                return Err("Dominio no válido.".into());
            }
        }
        "name" if !v.contains(' ') => return Err("Escribe nombre y apellido.".into()),
        _ => {}
    }
    Ok(v)
}

fn panic_message(p: Box<dyn Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "error desconocido".into())
}

fn run_collection(itype: &str, target: &str) -> Vec<ModResult> {
    let mods = get_modules(itype);
    println!(
        "\n{} {}  ·  {} {}  ·  {} fuentes en paralelo\n",
        style("Objetivo:").bold(),
        style(target).cyan(),
        style("Tipo:").bold(),
        type_name(itype),
        mods.len()
    );
    let mp = MultiProgress::new();
    let bar_style = ProgressStyle::with_template(
        "{spinner:.cyan} {prefix:38!.bold} {bar:28.cyan/blue} {msg:50!} {elapsed:.dim}",
    )
    .unwrap()
    .progress_chars("━╸─");
    let bars: Vec<ProgressBar> = mods
        .iter()
        .map(|m| {
            let pb = mp.add(ProgressBar::new(0).with_style(bar_style.clone()).with_prefix(m.title));
            pb.enable_steady_tick(Duration::from_millis(120));
            pb
        })
        .collect();

    // Una fuente nunca tumba el resto: los pánicos se convierten en "error"
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let results: Vec<ModResult> = thread::scope(|s| {
        let handles: Vec<_> = mods
            .iter()
            .zip(&bars)
            .map(|(m, pb)| {
                s.spawn(move || {
                    let on_progress = |done: u64, total: u64, text: &str| {
                        pb.set_length(total);
                        pb.set_position(done);
                        pb.set_message(if text.is_empty() { format!("{done}/{total}") } else { text.to_string() });
                    };
                    let ctx = Ctx::new(&on_progress);
                    let res = panic::catch_unwind(AssertUnwindSafe(|| (m.func)(target, &ctx))).unwrap_or_else(|p| {
                        result(m.name, m.title, Status::Error, format!("Fallo inesperado: {}", panic_message(p)))
                    });
                    pb.set_length(1);
                    pb.set_position(1);
                    pb.finish_with_message(format!("{} {}", status_label(res.status), take_chars(&res.summary, 40)));
                    res
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("hilo de módulo")).collect()
    });
    panic::set_hook(prev_hook);
    results
}

fn investigate(itype: &str, target: &str, open_browser: bool) {
    let start = Instant::now();
    let results = run_collection(itype, target);
    let secs = start.elapsed().as_secs_f64();
    render::show_results(&results, target, itype, secs);

    let outdir = root().join("reports");
    if let Err(e) = fs::create_dir_all(&outdir) {
        eprintln!("{} no se pudo crear {}: {e}", style("✖").red(), outdir.display());
        return;
    }
    let safe = take_chars(&UNSAFE_RE.replace_all(target, "_"), 40);
    let base = outdir.join(format!("{}_{safe}", Local::now().format("%Y%m%d_%H%M%S")));
    let html = report::save_html(&results, target, itype, secs, &base.with_extension("html"));
    let js = report::save_json(&results, target, itype, &base.with_extension("json"));
    match (html, js) {
        (Ok(html), Ok(js)) => {
            let html = fs::canonicalize(&html).unwrap_or(html);
            let line = style("─".repeat(60)).green();
            println!("{line}");
            println!("{} {}", style("Reporte HTML:").green().bold(), html.display());
            println!("{}  {}", style("Datos JSON:").bold(), js.display());
            println!("{line}");
            if open_browser {
                let _ = webbrowser::open(&format!("file://{}", html.display()));
            }
        }
        (Err(e), _) | (_, Err(e)) => eprintln!("{} no se pudo guardar el reporte: {e}", style("✖").red()),
    }
}

/// Lee una línea; None si se cerró la entrada (Ctrl-D).
fn ask(prompt: &str, default: Option<&str>) -> Option<String> {
    match default {
        Some(d) if !d.is_empty() => print!("{prompt} {}: ", style(format!("({d})")).cyan()),
        _ => print!("{prompt}: "),
    }
    io::stdout().flush().ok();
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).ok()? == 0 {
        return None;
    }
    let line = line.trim().to_string();
    Some(if line.is_empty() { default.unwrap_or("").to_string() } else { line })
}

fn ask_choice(prompt: &str, choices: &[&str], default: &str) -> Option<String> {
    loop {
        let p = format!("{prompt} {}", style(format!("[{}]", choices.join("/"))).magenta());
        let ans = ask(&p, Some(default))?;
        if choices.contains(&ans.as_str()) {
            return Some(ans);
        }
        println!("{}", style("Elige una de las opciones disponibles").red());
    }
}

fn confirm(prompt: &str, default: bool) -> Option<bool> {
    loop {
        let ans = ask(&format!("{prompt} {}", style("[s/n]").magenta()), Some(if default { "s" } else { "n" }))?;
        match ans.to_lowercase().as_str() {
            "s" | "si" | "sí" | "y" | "yes" => return Some(true),
            "n" | "no" => return Some(false),
            _ => println!("{}", style("Responde s o n").red()),
        }
    }
}

fn menu() {
    let term = Term::stdout();
    loop {
        let _ = term.clear_screen();
        println!("{}", style(BANNER).cyan().bold());
        println!(" {}\n", style("fuentes abiertas · gratis · sin API keys").black().bright());
        for (k, _, label, ex) in TYPES {
            println!("  {}) {label:<17} {}", style(k).cyan().bold(), style(ex).black().bright());
        }
        println!("  {}) Detectar automáticamente", style("6").cyan().bold());
        println!("  {}) Salir\n", style("0").cyan().bold());
        let Some(choice) = ask_choice("Opción", &["0", "1", "2", "3", "4", "5", "6"], "6") else { break };
        if choice == "0" {
            break;
        }
        let (itype, raw) = if choice == "6" {
            let Some(raw) = ask("Introduce email, username, nombre, teléfono o dominio", None) else { break };
            let itype = detect_type(&raw);
            println!("{} {}", style("→ detectado como:").black().bright(), type_name(itype));
            (itype, raw)
        } else {
            let (_, itype, label, ex) = TYPES.iter().find(|t| t.0 == choice).copied().unwrap();
            let Some(raw) = ask(&format!("{label} {}", style(format!("(ej. {ex})")).black().bright()), None) else {
                break;
            };
            (itype, raw)
        };
        let target = match validate(itype, &raw) {
            Ok(t) => t,
            Err(err) => {
                println!("{}", style(format!("✖ {err}")).red());
                if ask("Pulsa ENTER para continuar", None).is_none() {
                    break;
                }
                continue;
            }
        };
        investigate(itype, &target, true);
        println!();
        if confirm("¿Nueva investigación?", true) != Some(true) {
            break;
        }
    }
    println!("\n{}", style("Hasta luego.").cyan());
}

#[derive(Parser)]
#[command(about = "OSINT tool (fuentes gratuitas)")]
struct Args {
    /// email, username, nombre, teléfono o dominio
    target: Option<String>,
    /// forzar el tipo
    #[arg(short = 't', long = "type", value_parser = ["email", "username", "name", "phone", "domain"])]
    itype: Option<String>,
    /// no abrir el navegador
    #[arg(long)]
    no_open: bool,
}

fn main() {
    let args = Args::parse();
    let Some(raw) = args.target else {
        menu();
        return;
    };
    let itype: &str = match args.itype.as_deref() {
        Some(t) => TYPES.iter().find(|x| x.1 == t).map(|x| x.1).unwrap(),
        None => detect_type(&raw),
    };
    match validate(itype, &raw) {
        Ok(target) => investigate(itype, &target, !args.no_open),
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }
}
