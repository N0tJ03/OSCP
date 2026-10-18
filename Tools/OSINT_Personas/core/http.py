"""
core/http.py — Sesión HTTP compartida con reintentos y User-Agent de navegador.
"""
import logging

import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/124.0 Safari/537.36")

# Los avisos de urllib3 ("connection pool is full"...) ensucian la barra de progreso
logging.getLogger("urllib3").setLevel(logging.ERROR)


def make_session(pool: int = 20, retries: int = 2) -> requests.Session:
    s = requests.Session()
    s.headers.update({"User-Agent": UA, "Accept-Language": "en-US,en;q=0.9"})
    retry = Retry(total=retries, backoff_factor=1.5,
                  status_forcelist=(429, 500, 502, 503, 504),
                  allowed_methods=("GET", "POST"), respect_retry_after_header=True)
    adapter = HTTPAdapter(max_retries=retry, pool_connections=pool,
                          pool_maxsize=pool)
    s.mount("http://", adapter)
    s.mount("https://", adapter)
    return s


session = make_session()
