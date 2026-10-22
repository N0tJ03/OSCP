//! modules/name_osint.rs — Nombre completo: búsquedas dirigidas en buscadores y
//! redes con TODAS las combinaciones del nombre (Pepe Pepito García ->
//! "Pepe Pepito García", "Pepe García", "Pepe Pepito", ...). La búsqueda de
//! cuentas por username la hace modules/username.rs.

use crate::base::registry::{result, Ctx, Fields, Link, ModResult, Module, Status};
use crate::base::utils::{
    accent_forms, has_accents, linkedin_slugs, name_combos, name_variants, quote, quote_plus, search_links,
};

pub const MODULES: &[Module] =
    &[Module { name: "name_search", title: "Búsquedas del nombre", accepts: &["name"], func: name_search }];

fn name_search(name: &str, _ctx: &Ctx) -> ModResult {
    let combos: Vec<String> = name_combos(name).iter().map(|c| c.join(" ")).collect();
    // Con y sin tildes: "Pepe García" y "Pepe Garcia" dan resultados distintos
    // en LinkedIn/Facebook según cómo lo escribiera la persona.
    // Google admite ~32 palabras por consulta: se usan las 4 primeras combinaciones
    let any_name = combos
        .iter()
        .take(4)
        .flat_map(|c| accent_forms(c))
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut links = search_links(&[
        ("Google: cualquier forma del nombre", any_name.clone()),
        ("Google: LinkedIn", format!("({any_name}) site:linkedin.com/in")),
        ("Google: Facebook", format!("({any_name}) site:facebook.com")),
        ("Google: Instagram", format!("({any_name}) site:instagram.com")),
        ("Google: X / Twitter", format!("({any_name}) site:x.com OR site:twitter.com")),
        ("Google: TikTok", format!("({any_name}) site:tiktok.com")),
        (
            "Google: documentos (PDF, Excel, Word)",
            format!("({any_name}) filetype:pdf OR filetype:xlsx OR filetype:docx"),
        ),
        (
            "Google: boletines oficiales",
            format!("({any_name}) site:boe.es OR site:dof.gob.mx OR site:gob.mx"),
        ),
    ]);
    for c in &combos {
        for f in accent_forms(c) {
            let tag = if has_accents(&f) {
                "con tildes"
            } else if has_accents(c) {
                "sin tildes"
            } else {
                ""
            };
            links.push(Link::with_extra(
                format!("Google: \"{f}\""),
                format!("https://www.google.com/search?q={}", quote_plus(&format!("\"{f}\""))),
                format!("nombre exacto{}", if tag.is_empty() { String::new() } else { format!(" · {tag}") }),
            ));
        }
    }
    for slug in linkedin_slugs(name) {
        links.push(Link::with_extra(
            format!("LinkedIn /in/{slug}"),
            format!("https://www.linkedin.com/in/{}", quote(&slug)),
            format!(
                "perfil probable (comprobar a mano){}",
                if has_accents(&slug) { " · con tildes" } else { "" }
            ),
        ));
    }
    for f in accent_forms(name.trim()) {
        links.push(Link::new(
            format!("LinkedIn (buscador): {f}"),
            format!("https://www.linkedin.com/search/results/people/?keywords={}", quote(&f)),
        ));
        links.push(Link::new(
            format!("Facebook (buscador): {f}"),
            format!("https://www.facebook.com/search/people/?q={}", quote(&f)),
        ));
    }
    links.push(Link::new(
        "Google Imágenes",
        format!("https://www.google.com/search?tbm=isch&q={}", quote_plus(&any_name)),
    ));
    let mut users = name_variants(name, 8, false);
    if has_accents(name) {
        for v in name_variants(name, 3, true) {
            if !users.contains(&v) {
                users.push(v);
            }
        }
    }
    let mut fields = Fields::new();
    fields.set("Combinaciones del nombre", combos.join("\n"));
    fields.set("Usernames que se prueban", users.join(", "));
    let summary = format!("{} combinaciones · {} búsquedas preparadas", combos.len(), links.len());
    result("name_search", "Búsquedas del nombre", Status::Info, summary)
        .fields(fields)
        .links(links)
        .notes(vec![
            "LinkedIn exige iniciar sesión, así que los perfiles /in/ no se pueden comprobar automáticamente: ábrelos a mano."
                .into(),
        ])
}
