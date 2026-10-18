#!/usr/bin/env python3
# enumeracion.py - Enumeración pasiva y activa de un dominio/IP
# Requiere: pip install python-nmap requests dnspython python-whois
#           nmap instalado en el sistema

import argparse
import json
import os
import re
import socket
import ssl
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime
from functools import wraps
from urllib.parse import urlparse

import requests
import dns.resolver
import whois

try:
    import nmap
    NMAP_DISPONIBLE = True
except ImportError:
    NMAP_DISPONIBLE = False

PUERTOS_COMUNES = {
    21: "FTP", 22: "SSH", 23: "Telnet", 25: "SMTP", 53: "DNS",
    80: "HTTP", 110: "POP3", 143: "IMAP", 443: "HTTPS",
    445: "SMB", 3306: "MySQL", 3389: "RDP", 8080: "HTTP-alt",
}

USER_AGENT = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"

# ------------------------- COLOR -------------------------

class Color:
    _activar = sys.stdout.isatty() and os.environ.get("NO_COLOR") is None

    try:
        import colorama  # noqa: F401
        _activar = _activar or os.name == "nt"
    except ImportError:
        pass

    def _c(codigo, texto):
        return f"\033[{codigo}m{texto}\033[0m" if Color._activar else str(texto)

    @classmethod
    def titulo(cls, t):    return cls._c("1;36", t)     # cian negrita
    @classmethod
    def ok(cls, t):        return cls._c("1;32", t)     # verde
    @classmethod
    def aviso(cls, t):     return cls._c("1;33", t)     # amarillo
    @classmethod
    def error(cls, t):     return cls._c("1;31", t)     # rojo
    @classmethod
    def atenuado(cls, t):  return cls._c("2", t)        # gris
    @classmethod
    def valor(cls, t):     return cls._c("1;37", t)     # blanco negrita

# ------------------------- REPORTERO -------------------------

class Reportero:
    """Gestiona la salida dual: consola coloreada + informe limpio + JSON."""

    def __init__(self, archivo_txt):
        self.f = open(archivo_txt, "w", encoding="utf-8")
        self.json_path = archivo_txt.rsplit(".", 1)[0] + ".json"
        self.datos = {"fecha": datetime.now().isoformat(), "secciones": {}}
        self._seccion_actual = None
        self._t_seccion = None
        self.hallazgos = {"puertos": 0, "subdominios": 0, "web": 0, "servicios": 0}

    # --- primitivas ---

    def consola(self, texto="", end="\n"):
        print(texto, end=end)
        if end != "\n":
            sys.stdout.flush()

    def informe(self, texto=""):
        self.f.write(texto + "\n")

    def ambos(self, texto_consola, texto_informe=None):
        self.consola(texto_consola)
        self.informe(texto_informe if texto_informe is not None else self._limpiar_ansi(texto_consola))

    @staticmethod
    def _limpiar_ansi(texto):
        return re.sub(r"\033\[[0-9;]*m", "", texto)

    # --- estructura del informe ---

    def seccion(self, titulo):
        self._t_seccion = time.time()
        self.datos["secciones"][titulo] = {}
        self._seccion_actual = titulo
        self.ambos("")
        self.ambos(Color.titulo(f"┌─[ {titulo} ]" + "─" * max(0, 52 - len(titulo))))
        self.informe("-" * 60)
        self.informe(titulo.upper())
        self.informe("-" * 60)

    def fin_seccion(self):
        if self._t_seccion:
            dt = time.time() - self._t_seccion
            self.consola(Color.titulo("└─" + "─" * 54) + Color.atenuado(f" ({dt:.1f}s)"))
            self.informe(f"[-] Fin de sección ({dt:.1f}s)")
        self._seccion_actual = None
        self._t_seccion = None

    def item(self, etiqueta, valor="", estado="info"):
        """estado: info | ok | aviso | error"""
        icono = {"info": "[*]", "ok": "[+]", "aviso": "[!]", "error": "[-]"}[estado]
        color = {"info": Color.valor, "ok": Color.ok, "aviso": Color.aviso, "error": Color.error}[estado]
        linea_consola = f"  {color(icono)} {etiqueta}" + (f": {Color.valor(valor)}" if valor else "")
        linea_info = f"  {icono} {etiqueta}" + (f": {valor}" if valor else "")
        self.ambos(linea_consola, linea_info)
        if self._seccion_actual and etiqueta:
            self.datos["secciones"][self._seccion_actual][etiqueta] = valor or estado

    def tabla(self, titulo, filas, cabeceras):
        """Tabla alineada. filas: lista de tuplas/listas."""
        if not filas:
            self.item(f"Sin {titulo.lower()}", estado="aviso")
            return
        anchos = [len(str(c)) for c in cabeceras]
        for fila in filas:
            for i, celda in enumerate(fila):
                anchos[i] = max(anchos[i], len(str(celda)))
        sep = "    " + "+".join("-" * (a + 2) for a in anchos)

        def formatear(fila, colorear=False):
            celdas = [f" {str(c):<{anchos[i]}} " for i, c in enumerate(fila)]
            linea = "    " + "|".join(celdas)
            return Color.valor(linea) if colorear else linea

        self.consola(Color.titulo(f"\n  {titulo}"))
        self.informe(f"\n  {titulo}")
        self.ambos(formatear(cabeceras, colorear=True))
        self.consola(Color.atenuado(sep))
        self.informe(sep)
        for fila in filas:
            self.ambos(formatear(fila))
        if self._seccion_actual:
            self.datos["secciones"][self._seccion_actual][titulo] = [list(f) for f in filas]

    def progreso(self, actual, total, prefijo=""):
        pct = int(actual * 40 / total) if total else 40
        bar = "█" * pct + "░" * (40 - pct)
        self.consola(f"\r  {prefijo} {Color.atenuado(bar)} {actual}/{total}", end="")

    # --- resumen final ---

    def resumen(self, objetivo, duracion):
        self.ambos("")
        self.ambos(Color.titulo("=" * 60))
        self.ambos(Color.titulo(f"  RESUMEN: {objetivo}  ·  {duracion:.1f}s"))
        self.ambos(Color.titulo("=" * 60))
        self.informe("=" * 60)
        self.informe(f"  RESUMEN: {objetivo}  ·  {duracion:.1f}s")
        self.informe("=" * 60)
        self.item("Puertos abiertos", str(self.hallazgos["puertos"]),
                  "ok" if self.hallazgos["puertos"] else "aviso")
        self.item("Servicios identificados (nmap)", str(self.hallazgos["servicios"]),
                  "ok" if self.hallazgos["servicios"] else "aviso")
        self.item("Subdominios vivos", str(self.hallazgos["subdominios"]),
                  "ok" if self.hallazgos["subdominios"] else "aviso")
        self.item("Tecnologías web detectadas", str(self.hallazgos["web"]),
                  "ok" if self.hallazgos["web"] else "aviso")

    def cerrar(self):
        self.f.close()
        with open(self.json_path, "w", encoding="utf-8") as jf:
            json.dump(self.datos, jf, indent=2, ensure_ascii=False, default=str)


# ------------------------- CAPTURA ERRORES -------------------------

def captura_errores(mensaje):
    def decorador(func):
        @wraps(func)
        def envoltura(self, *args, **kwargs):
            try:
                return func(self, *args, **kwargs)
            except Exception as e:
                self.r.item(mensaje, str(e), estado="error")
                try:
                    self.r.fin_seccion()
                except Exception:
                    pass
                return None
        return envoltura
    return decorador


# ------------------------- ENUMERADOR -------------------------

class Enumerador:
    def __init__(self, objetivo, reportero, timeout=10, hilos=20):
        self.objetivo = objetivo
        self.r = reportero
        self.timeout = timeout
        self.hilos = hilos
        self.session = requests.Session()
        self.session.headers.update({"User-Agent": USER_AGENT})
        self.ip = None
        self.abiertos = []
        self.servicios_nmap = []
        self.nm = None
        if NMAP_DISPONIBLE:
            try:
                self.nm = nmap.PortScanner()
            except nmap.PortScannerError:
                self.nm = None

    # ---------------- FASE PASIVA ----------------

    @captura_errores("WHOIS falló")
    def whois_lookup(self):
        self.r.seccion("FASE PASIVA · WHOIS")
        w = whois.whois(self.objetivo)
        for campo, etiqueta in [("domain_name", "Dominio"), ("registrar", "Registrar"),
                                ("creation_date", "Creado"), ("expiration_date", "Expira"),
                                ("name_servers", "Name servers"), ("emails", "Contacto")]:
            valor = w.get(campo)
            if valor:
                if isinstance(valor, list):
                    valor = ", ".join(map(str, valor))
                self.r.item(etiqueta, valor)
        self.r.fin_seccion()

    @captura_errores("DNS falló")
    def dns_records(self):
        self.r.seccion("FASE PASIVA · DNS")
        filas = []
        for tipo in ["A", "AAAA", "MX", "NS", "TXT", "CNAME"]:
            try:
                for reg in dns.resolver.resolve(self.objetivo, tipo, lifetime=self.timeout):
                    filas.append((tipo, str(reg)))
            except Exception:
                pass
        if filas:
            self.r.tabla("Registros DNS", filas, ["Tipo", "Valor"])
        else:
            self.r.item("Sin registros públicos", estado="aviso")
        self.r.fin_seccion()

    @captura_errores("crt.sh falló")
    def subdominios_crtsh(self):
        self.r.seccion("FASE PASIVA · SUBDOMINIOS (Certificate Transparency)")
        url = f"https://crt.sh/?q=%25.{self.objetivo}&output=json"

        # Reintentos: crt.sh suele dar 502/504 o respuestas vacías
        datos = []
        for intento in range(3):
            try:
                resp = self.session.get(url, timeout=60)
                if resp.status_code == 200 and resp.text.strip():
                    datos = resp.json()
                    break
            except (requests.RequestException, json.JSONDecodeError, ValueError):
                pass
            self.r.consola(Color.atenuado(f"  Intento {intento + 1}/3 falló, reintentando..."))
            time.sleep(3)

        subs = set()
        for entrada in datos:
            for nombre in entrada.get("name_value", "").split("\n"):
                nombre = nombre.strip().lower().lstrip("*.")
                if nombre.endswith(self.objetivo) and nombre != self.objetivo:
                    subs.add(nombre)

        if not subs:
            self.r.item("Sin subdominios en CT logs", estado="aviso")
            self.r.fin_seccion()
            return []

        def resuelve(sub):
            try:
                return sub, socket.gethostbyname(sub)
            except OSError:
                return sub, None

        self.r.consola(Color.atenuado(f"  Resolviendo {len(subs)} subdominios..."))
        filas, vivos = [], []
        with ThreadPoolExecutor(max_workers=self.hilos) as pool:
            for i, (sub, ip) in enumerate(pool.map(resuelve, sorted(subs)), 1):
                self.r.progreso(i, len(subs), "DNS")
                filas.append((sub, ip or "—"))
                if ip:
                    vivos.append((sub, ip))
        self.r.consola("")
        self.r.tabla(f"Subdominios ({len(subs)} encontrados, {len(vivos)} vivos)",
                     filas, ["Subdominio", "IP"])
        self.r.hallazgos["subdominios"] = len(vivos)
        self.r.fin_seccion()
        return vivos

    # ---------------- FASE ACTIVA ----------------

    @captura_errores("Resolución falló")
    def resolver_ip(self):
        self.r.seccion("FASE ACTIVA · RESOLUCIÓN")
        self.ip = socket.gethostbyname(self.objetivo)
        self.r.item("IP del objetivo", self.ip, estado="ok")
        self.r.fin_seccion()
        return self.ip

    def _probar_puerto(self, puerto):
        try:
            with socket.create_connection((self.objetivo, puerto), timeout=1.5):
                return puerto, PUERTOS_COMUNES.get(puerto, "?")
        except (OSError, socket.timeout):
            return None

    @captura_errores("Escaneo de puertos falló")
    def descubrir_puertos(self):
        self.r.seccion("FASE ACTIVA · PUERTOS")
        filas = []
        with ThreadPoolExecutor(max_workers=self.hilos) as pool:
            futuros = {pool.submit(self._probar_puerto, p): p for p in PUERTOS_COMUNES}
            for i, futuro in enumerate(as_completed(futuros), 1):
                resultado = futuro.result()
                if resultado:
                    puerto, servicio = resultado
                    filas.append((puerto, "tcp", "abierto", servicio))
                    self.abiertos.append(puerto)
                self.r.progreso(i, len(PUERTOS_COMUNES), "TCP")
        self.r.consola("")
        if filas:
            self.r.tabla(f"Puertos abiertos ({len(filas)})", filas,
                         ["Puerto", "Proto", "Estado", "Servicio"])
            self.r.hallazgos["puertos"] = len(filas)
        else:
            self.r.item("Ningún puerto común abierto", estado="aviso")
        self.r.fin_seccion()

    @captura_errores("nmap falló")
    def nmap_deep(self, puertos):
        if not self.nm:
            self.r.item("nmap no disponible; usando banner grabbing básico", estado="aviso")
            return
        self.r.seccion("FASE ACTIVA · SERVICIOS Y VERSIONES (nmap -sV)")
        lista = ",".join(map(str, puertos))
        self.r.item("Comando", f"nmap -sV -Pn --script=default -p {lista} {self.ip}", estado="info")
        self.nm.scan(self.ip, arguments=f"-sV -Pn --script=default -p {lista}")
        filas = []
        for host in self.nm.all_hosts():
            for puerto in self.nm[host]["tcp"]:
                d = self.nm[host]["tcp"][puerto]
                producto = f"{d.get('product', '')} {d.get('version', '')}".strip() or "—"
                filas.append((puerto, d.get("name", "?"), producto, d.get("state", "")))
                if d.get("script"):
                    for nombre, out in d["script"].items():
                        resumen = self.r._limpiar_ansi(out.strip().replace("\n", " | "))[:300]
                        self.r.item(f"NSE {puerto}/{nombre}", resumen)
        self.r.tabla(f"Servicios identificados ({len(filas)})", filas,
                     ["Puerto", "Servicio", "Producto/Versión", "Estado"])
        self.r.hallazgos["servicios"] = len(filas)
        self.servicios_nmap = filas
        self.r.fin_seccion()

    @captura_errores("Banner grabbing falló")
    def banner_grabbing(self, puerto):
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        with socket.create_connection((self.objetivo, puerto), timeout=3) as sock:
            if puerto in (443, 8443):
                sock = ctx.wrap_socket(sock, server_hostname=self.objetivo)
            sock.sendall(f"HEAD / HTTP/1.1\r\nHost: {self.objetivo}\r\n"
                         f"Connection: close\r\n\r\n".encode())
            linea = sock.recv(1024).decode(errors="ignore").split("\r\n")[0]
            self.r.item(f"Banner {puerto}/tcp", linea)

    @captura_errores("HTTP falló")
    def tech_detection(self, url):
        self.r.seccion("FASE ACTIVA · WEB")
        r = self.session.get(url, timeout=self.timeout)
        self.r.item("URL final", r.url)
        self.r.item("Código HTTP", str(r.status_code),
                    estado="ok" if r.status_code < 400 else "aviso")
        self.r.item("Server", r.headers.get("Server", "no revelado"))
        self.r.item("X-Powered-By", r.headers.get("X-Powered-By", "no revelado"))
        techs = [t for t in (r.headers.get("Server"), r.headers.get("X-Powered-By")) if t]
        base = r.url.rstrip("/")
        for ruta in ("/robots.txt", "/sitemap.xml", "/.well-known/security.txt"):
            try:
                rr = self.session.get(base + ruta, timeout=self.timeout)
                if rr.status_code == 200:
                    self.r.item(f"Encontrado {ruta}", f"{len(rr.text)} bytes", estado="ok")
                    techs.append(ruta)
            except requests.RequestException:
                pass
        self.r.hallazgos["web"] = len(techs)
        self.r.fin_seccion()

    # ---------------- ORQUESTACIÓN ----------------

    def ejecutar(self, sin_whois=False, sin_subdominios=False):
        if not sin_whois:
            self.whois_lookup()
        self.dns_records()
        if not sin_subdominios:
            self.subdominios_crtsh()

        if not self.resolver_ip():
            self.r.item("El objetivo no resuelve a IP; se omite el escaneo de red", estado="error")
            return False

        objetivo_web = f"http://{self.objetivo}"
        for esquema in ("https", "http"):
            try:
                url = f"{esquema}://{self.objetivo}"
                if self.session.get(url, timeout=self.timeout).status_code < 500:
                    objetivo_web = url
                    break
            except requests.RequestException:
                continue
        self.tech_detection(objetivo_web)

        self.descubrir_puertos()
        if self.abiertos:
            if self.nm:
                self.nmap_deep(self.abiertos)
            else:
                for puerto in self.abiertos:
                    if puerto in (80, 443, 8080, 21, 22, 25):
                        self.banner_grabbing(puerto)
        return True


# ------------------------- UTILIDADES -------------------------

def validar_objetivo(objetivo):
    objetivo = objetivo.strip()
    try:
        socket.inet_aton(objetivo)
        return objetivo
    except OSError:
        pass
    if "://" in objetivo:
        objetivo = urlparse(objetivo).hostname
    if not re.match(r"^(?!-)[A-Za-z0-9-]{1,63}(?<!-)(\.[A-Za-z0-9-]{1,63})*$", objetivo):
        raise ValueError(f"Objetivo no válido: {objetivo}")
    return objetivo


def main():
    parser = argparse.ArgumentParser(description="Enumeración pasiva y activa de un dominio/IP")
    parser.add_argument("objetivo", help="dominio o IP (ej: ejemplo.com)")
    parser.add_argument("-o", "--salida", help="archivo de resultados .txt")
    parser.add_argument("-t", "--timeout", type=int, default=10, help="timeout HTTP/DNS en segundos")
    parser.add_argument("--hilos", type=int, default=20, help="hilos para escaneo/resolución")
    parser.add_argument("--sin-whois", action="store_true", help="omitir WHOIS")
    parser.add_argument("--sin-subdominios", action="store_true", help="omitir crt.sh")
    args = parser.parse_args()

    try:
        objetivo = validar_objetivo(args.objetivo)
    except ValueError as e:
        parser.error(str(e))

    archivo = args.salida or f"enumeracion_{objetivo}.txt"

    print(Color.titulo("=" * 60))
    print(Color.titulo("  Script de Enumeración Pasiva y Activa"))
    print(Color.titulo(f"  Objetivo: {objetivo}"))
    print(Color.titulo("=" * 60))

    r = Reportero(archivo)
    inicio = time.time()
    enum = Enumerador(objetivo, r, timeout=args.timeout, hilos=args.hilos)
    try:
        exito = enum.ejecutar(sin_whois=args.sin_whois,
                              sin_subdominios=args.sin_subdominios)
        r.resumen(objetivo, time.time() - inicio)
    finally:
        r.cerrar()

    print(Color.ok(f"\n[+] Informe guardado en: {archivo}"))
    print(Color.ok(f"[+] Datos JSON en:     {r.json_path}"))
    sys.exit(0 if exito else 2)


if __name__ == "__main__":
    main()
