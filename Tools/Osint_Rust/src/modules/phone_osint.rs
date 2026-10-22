//! modules/phone_osint.rs — Análisis de teléfono con el crate `phonenumber`
//! (offline, puerto de libphonenumber) + enlaces de búsqueda.
//!
//! Diferencia con la versión Python: el crate no incluye las bases de datos de
//! geocodificación, operador ni zona horaria de libphonenumber, así que "Zona"
//! y "Operador" quedan como desconocidos.

use phonenumber::{Mode, Type};

use crate::base::registry::{result, Ctx, Fields, Link, ModResult, Module, Status};
use crate::base::utils::search_links;

pub const MODULES: &[Module] =
    &[Module { name: "phone_analysis", title: "Análisis del teléfono", accepts: &["phone"], func: phone_analysis }];

fn type_name(t: Type) -> &'static str {
    match t {
        Type::FixedLine => "Fijo",
        Type::Mobile => "Móvil",
        Type::FixedLineOrMobile => "Fijo o móvil",
        Type::TollFree => "Gratuito",
        Type::PremiumRate => "Tarificación especial",
        Type::SharedCost => "Coste compartido",
        Type::Voip => "VoIP",
        Type::PersonalNumber => "Número personal",
        Type::Pager => "Busca",
        Type::Uan => "UAN",
        Type::Voicemail => "Buzón de voz",
        _ => "Desconocido",
    }
}

/// Nombre en español de los países más habituales (si no, el código ISO).
fn country_name(iso: &str) -> String {
    let name = match iso {
        "ES" => "España", "MX" => "México", "AR" => "Argentina", "CO" => "Colombia",
        "CL" => "Chile", "PE" => "Perú", "VE" => "Venezuela", "EC" => "Ecuador",
        "BO" => "Bolivia", "PY" => "Paraguay", "UY" => "Uruguay", "CR" => "Costa Rica",
        "PA" => "Panamá", "GT" => "Guatemala", "HN" => "Honduras", "SV" => "El Salvador",
        "NI" => "Nicaragua", "CU" => "Cuba", "DO" => "República Dominicana",
        "PR" => "Puerto Rico", "US" => "Estados Unidos", "CA" => "Canadá", "BR" => "Brasil",
        "GB" => "Reino Unido", "IE" => "Irlanda", "FR" => "Francia", "DE" => "Alemania",
        "IT" => "Italia", "PT" => "Portugal", "NL" => "Países Bajos", "BE" => "Bélgica",
        "CH" => "Suiza", "AT" => "Austria", "SE" => "Suecia", "NO" => "Noruega",
        "DK" => "Dinamarca", "FI" => "Finlandia", "PL" => "Polonia", "RO" => "Rumanía",
        "GR" => "Grecia", "RU" => "Rusia", "UA" => "Ucrania", "TR" => "Turquía",
        "MA" => "Marruecos", "DZ" => "Argelia", "EG" => "Egipto", "ZA" => "Sudáfrica",
        "NG" => "Nigeria", "IN" => "India", "CN" => "China", "JP" => "Japón",
        "KR" => "Corea del Sur", "AU" => "Australia", "NZ" => "Nueva Zelanda",
        "AD" => "Andorra", "IL" => "Israel", "AE" => "Emiratos Árabes Unidos",
        "SA" => "Arabia Saudí", "PH" => "Filipinas", "ID" => "Indonesia", "TH" => "Tailandia",
        _ => return iso.to_string(),
    };
    name.to_string()
}

fn phone_analysis(number: &str, _ctx: &Ctx) -> ModResult {
    let title = "Análisis del teléfono";
    let p = match phonenumber::parse(None, number) {
        Ok(p) => p,
        Err(e) => {
            return result(
                "phone_analysis",
                title,
                Status::Error,
                format!("{e} (usa formato internacional, ej. +34600123456)"),
            );
        }
    };
    if !phonenumber::is_valid(&p) {
        let mut fields = Fields::new();
        fields.set("Número", number);
        fields.set("Válido", "No");
        return result("phone_analysis", title, Status::Empty, "Número NO válido").fields(fields);
    }

    let e164 = p.format().mode(Mode::E164).to_string();
    let intl = p.format().mode(Mode::International).to_string();
    let national = p.format().mode(Mode::National).to_string();
    let digits = e164.trim_start_matches('+').to_string();
    let country = p.country().id().map(|id| country_name(&format!("{id:?}"))).unwrap_or_else(|| "?".into());
    let kind = type_name(p.number_type(&phonenumber::metadata::DATABASE));

    let mut fields = Fields::new();
    fields.set("Número", &intl);
    fields.set("Válido", "Sí");
    fields.set("País", &country);
    fields.set("Prefijo", format!("+{}", p.country().code()));
    fields.set("Zona", "—");
    fields.set("Operador", "desconocido");
    fields.set("Tipo", kind);
    fields.set("Formatos", format!("{e164}  ·  {national}"));

    let mut links = vec![
        Link::new("Abrir chat de WhatsApp", format!("https://wa.me/{digits}")),
        Link::new("Telegram", format!("https://t.me/+{digits}")),
    ];
    links.extend(search_links(&[
        (
            "Google: número (todos los formatos)",
            format!("\"{e164}\" OR \"{intl}\" OR \"{national}\" OR \"{digits}\""),
        ),
        (
            "Google: en Facebook / Instagram",
            format!("\"{national}\" OR \"{digits}\" site:facebook.com OR site:instagram.com"),
        ),
        (
            "Google: anuncios y directorios",
            format!("\"{national}\" OR \"{digits}\" (anuncio OR contacto OR telefono)"),
        ),
    ]));
    result("phone_analysis", title, Status::Found, format!("{country} · {kind} · desconocido"))
        .fields(fields)
        .links(links)
        .notes(vec![
            "El operador es el original; si se hizo portabilidad puede ser otro.".into(),
            "La versión Rust no incluye la base de datos de zonas/operadores de libphonenumber.".into(),
        ])
}
