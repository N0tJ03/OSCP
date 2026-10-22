//! modules/external_tools.rs — Herramientas externas OPCIONALES. Si no están
//! instaladas el módulo aparece como "omitido" y todo lo demás sigue igual.
//!     pip install sherlock-project maigret

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::base::registry::{result, Ctx, Link, ModResult, Module, Status};
use crate::base::utils::name_variants;

pub const MODULES: &[Module] = &[
    Module { name: "sherlock", title: "Sherlock (400+ redes)", accepts: &["username", "name"], func: sherlock },
    Module { name: "maigret", title: "Maigret (top 500 sitios)", accepts: &["username", "name"], func: maigret },
];

static FOUND_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\+\]\s*([^:\n]+):\s*(https?://\S+)").unwrap());
pub static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());

/// Busca en PATH y en las carpetas habituales de pip (~/.local/bin, venv).
pub fn find_tool(tool: &str) -> Option<PathBuf> {
    if let Ok(p) = which::which(tool) {
        return Some(p);
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        dirs.push(Path::new(&home).join(".local").join("bin"));
    }
    if let Some(venv) = std::env::var_os("VIRTUAL_ENV") {
        dirs.push(Path::new(&venv).join("bin"));
        dirs.push(Path::new(&venv).join("Scripts"));
    }
    for d in dirs {
        for name in [tool.to_string(), format!("{tool}.exe")] {
            let cand = d.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub enum RunError {
    Timeout,
    Failed(String),
}

/// Ejecuta un programa con límite de tiempo y devuelve stdout + stderr.
pub fn run_command(binary: &Path, args: &[String], timeout: Duration, cwd: &Path) -> Result<String, RunError> {
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| RunError::Failed(e.to_string()))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let t_out = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let t_err = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(RunError::Failed(e.to_string())),
        }
    }
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    Ok(format!("{}\n{}", String::from_utf8_lossy(&stdout), String::from_utf8_lossy(&stderr)))
}

fn username(target: &str) -> String {
    let t = target.trim().trim_start_matches('@');
    if t.contains(' ') {
        name_variants(t, 1, false).into_iter().next().unwrap_or_else(|| t.to_string())
    } else {
        t.to_string()
    }
}

fn run_tool(module: &str, title: &str, tool: &str, args: &[&str], target: &str) -> ModResult {
    let Some(binary) = find_tool(tool) else {
        let pkg = if tool == "sherlock" { "sherlock-project" } else { tool };
        return result(module, title, Status::Skipped, format!("{tool} no instalado (opcional)"))
            .notes(vec![format!("Instálalo con:  pip install {pkg}")]);
    };
    let user = username(target);
    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => return result(module, title, Status::Error, e.to_string()),
    };
    let mut argv = vec![user.clone()];
    argv.extend(args.iter().map(|a| a.to_string()));
    let out = match run_command(&binary, &argv, Duration::from_secs(420), tmp.path()) {
        Ok(o) => o,
        Err(RunError::Timeout) => return result(module, title, Status::Error, "Tardó demasiado (>7 min)"),
        Err(RunError::Failed(e)) => return result(module, title, Status::Error, e),
    };
    let out = ANSI_RE.replace_all(&out, "");
    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for cap in FOUND_RE.captures_iter(&out) {
        let url = cap[2].to_string();
        if seen.insert(url.clone()) {
            links.push(Link::with_extra(cap[1].trim(), url, format!("@{user}")));
        }
    }
    if links.is_empty() {
        return result(module, title, Status::Empty, format!("Sin cuentas para @{user}"));
    }
    result(module, title, Status::Found, format!("{} cuentas para @{user}", links.len())).links(links)
}

fn sherlock(target: &str, _ctx: &Ctx) -> ModResult {
    run_tool(
        "sherlock",
        "Sherlock (400+ redes)",
        "sherlock",
        &["--print-found", "--timeout", "15", "--no-color", "--no-txt"],
        target,
    )
}

fn maigret(target: &str, _ctx: &Ctx) -> ModResult {
    run_tool(
        "maigret",
        "Maigret (top 500 sitios)",
        "maigret",
        &["--no-color", "--no-progressbar", "--timeout", "15", "--top-sites", "500"],
        target,
    )
}
