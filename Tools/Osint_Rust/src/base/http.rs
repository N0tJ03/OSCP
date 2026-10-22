//! base/http.rs — Cliente HTTP compartido con reintentos y User-Agent de navegador.

use std::fmt;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue, RETRY_AFTER};

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

const RETRY_STATUS: [u16; 5] = [429, 500, 502, 503, 504];
const BACKOFF_FACTOR: f64 = 1.5;

/// Errores HTTP ya traducidos a algo legible (equivale a `short_error`).
#[derive(Debug)]
pub enum HttpError {
    Timeout,
    Connect,
    Overloaded,
    Status(u16),
    Other(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::Timeout => write!(f, "el servicio tardó demasiado en responder (timeout)"),
            HttpError::Connect => write!(f, "no se pudo conectar con el servicio"),
            HttpError::Overloaded => write!(f, "el servicio está caído o saturado (errores 5xx)"),
            HttpError::Status(401) => write!(f, "requiere autenticación (401)"),
            HttpError::Status(403) => write!(f, "acceso bloqueado (403)"),
            HttpError::Status(429) => write!(f, "demasiadas peticiones, prueba en un rato (429)"),
            HttpError::Status(code) => write!(f, "error HTTP {code}"),
            HttpError::Other(msg) => write!(f, "{}", msg.chars().take(160).collect::<String>()),
        }
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            HttpError::Timeout
        } else if e.is_connect() {
            HttpError::Connect
        } else if let Some(code) = e.status() {
            HttpError::Status(code.as_u16())
        } else if e.is_decode() {
            HttpError::Other("respuesta no válida del servicio".into())
        } else {
            HttpError::Other(e.to_string())
        }
    }
}

pub struct Http {
    client: Client,
    retries: u32,
}

impl Http {
    pub fn new(pool: usize, retries: u32) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        let client = Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            .pool_max_idle_per_host(pool)
            .build()
            .expect("no se pudo crear el cliente HTTP");
        Http { client, retries }
    }

    /// Envía la petición construida por `make`, reintentando ante errores de
    /// conexión y respuestas 429/5xx con espera exponencial (respeta Retry-After).
    pub fn send(&self, make: impl Fn(&Client) -> RequestBuilder) -> Result<Response, HttpError> {
        let mut attempt = 0;
        loop {
            let last = attempt >= self.retries;
            match make(&self.client).send() {
                Ok(r) if RETRY_STATUS.contains(&r.status().as_u16()) && self.retries > 0 => {
                    if last {
                        return Err(HttpError::Overloaded);
                    }
                    thread::sleep(retry_after(&r).unwrap_or_else(|| backoff(attempt)));
                }
                Ok(r) => return Ok(r),
                Err(e) if (e.is_connect() || e.is_timeout()) && !last => {
                    thread::sleep(backoff(attempt));
                }
                Err(e) => return Err(e.into()),
            }
            attempt += 1;
        }
    }

    pub fn get(&self, url: &str, timeout: u64) -> Result<Response, HttpError> {
        self.send(|c| c.get(url).timeout(Duration::from_secs(timeout)))
    }

    pub fn get_query(&self, url: &str, query: &[(&str, &str)], timeout: u64) -> Result<Response, HttpError> {
        self.send(|c| c.get(url).query(query).timeout(Duration::from_secs(timeout)))
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs_f64(BACKOFF_FACTOR * 2f64.powi(attempt as i32))
}

fn retry_after(r: &Response) -> Option<Duration> {
    let secs: u64 = r.headers().get(RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(secs.min(60)))
}

/// Equivalente a `raise_for_status()`.
pub fn check(r: Response) -> Result<Response, HttpError> {
    let code = r.status().as_u16();
    if code >= 400 { Err(HttpError::Status(code)) } else { Ok(r) }
}

/// Descarga y decodifica JSON comprobando el estado HTTP.
pub fn json(r: Response) -> Result<serde_json::Value, HttpError> {
    Ok(check(r)?.json()?)
}

pub static SESSION: LazyLock<Http> = LazyLock::new(|| Http::new(20, 2));
