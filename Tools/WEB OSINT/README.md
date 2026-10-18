# 🔍 enumeracion.py

Script en Python para realizar **enumeración pasiva y activa** de un dominio o dirección IP. Combina consultas WHOIS, DNS, Certificate Transparency, escaneo de puertos, detección de servicios con `nmap` y análisis básico de tecnologías web, generando un informe legible en consola (con colores), un archivo de texto limpio y un archivo JSON con los datos estructurados.

> ⚠️ **Aviso legal:** utiliza esta herramienta **únicamente** sobre sistemas de los que seas propietario o para los que tengas autorización explícita y por escrito. El escaneo de sistemas ajenos sin permiso puede ser ilegal. El autor no se hace responsable del uso indebido.

---

## 📋 Índice

- [Características](#-características)
- [Requisitos](#-requisitos)
- [Instalación](#-instalación)
- [Uso](#-uso)
- [Opciones](#-opciones)
- [Ejemplos](#-ejemplos)
- [Funcionamiento detallado](#-funcionamiento-detallado)
- [Salida generada](#-salida-generada)
- [Códigos de salida](#-códigos-de-salida)
- [Limitaciones](#-limitaciones)

---

## ✨ Características

### Fase pasiva
| Módulo | Descripción |
|---|---|
| **WHOIS** | Dominio, registrador, fechas de creación/expiración, name servers y emails de contacto. |
| **DNS** | Consulta de registros `A`, `AAAA`, `MX`, `NS`, `TXT` y `CNAME`. |
| **Subdominios (crt.sh)** | Obtiene subdominios desde los logs de *Certificate Transparency*, con 3 reintentos ante fallos del servicio, y comprueba cuáles resuelven a una IP (en paralelo). |

### Fase activa
| Módulo | Descripción |
|---|---|
| **Resolución de IP** | Resuelve el objetivo a su dirección IP. |
| **Análisis web** | Detecta si responde por HTTPS/HTTP, muestra el código de estado y las cabeceras `Server` y `X-Powered-By`, y busca `/robots.txt`, `/sitemap.xml` y `/.well-known/security.txt`. |
| **Escaneo de puertos** | Escaneo TCP *connect* multihilo sobre 13 puertos comunes. |
| **nmap** | Si `nmap` está disponible, ejecuta `-sV -Pn --script=default` sobre los puertos abiertos para identificar productos y versiones. |
| **Banner grabbing** | Alternativa si `nmap` no está disponible: obtiene el banner de los puertos 21, 22, 25, 80, 443 y 8080. |

### Otras funcionalidades
- Salida en consola **con colores** y barras de progreso.
- Informe en **texto plano** (sin códigos ANSI) y en **JSON**.
- Resumen final con el número de hallazgos.
- Manejo de errores por módulo: si uno falla, el resto continúa.
- Validación del objetivo (dominio, IP o URL).

### Puertos escaneados

| Puerto | Servicio | Puerto | Servicio |
|---|---|---|---|
| 21 | FTP | 143 | IMAP |
| 22 | SSH | 443 | HTTPS |
| 23 | Telnet | 445 | SMB |
| 25 | SMTP | 3306 | MySQL |
| 53 | DNS | 3389 | RDP |
| 80 | HTTP | 8080 | HTTP-alt |
| 110 | POP3 | | |

---

## 🛠 Requisitos

- Python **3.8** o superior
- Librerías de Python:
  - [`requests`](https://pypi.org/project/requests/)
  - [`dnspython`](https://pypi.org/project/dnspython/)
  - [`python-whois`](https://pypi.org/project/python-whois/)
  - [`python-nmap`](https://pypi.org/project/python-nmap/) *(opcional, pero recomendado)*
  - [`colorama`](https://pypi.org/project/colorama/) *(opcional; habilita colores en Windows)*
- [`nmap`](https://nmap.org/) instalado en el sistema *(opcional; sin él se usa banner grabbing básico)*

---

## 📦 Instalación

```bash
# Clonar el repositorio
git clone https://github.com/N0tJ03/Enumeracion-de-dominio-Pasiva-Activa.git
cd Enumeracion-de-dominio-Pasiva-Activa

# Instalar dependencias de Python
pip install requests dnspython python-whois python-nmap colorama

# Instalar nmap (ejemplos)
sudo apt install nmap        # Debian / Ubuntu / Kali
brew install nmap            # macOS
```

---

## 🚀 Uso

```bash
python3 enumeracion.py <objetivo> [opciones]
```

El objetivo puede ser:
- Un dominio: `ejemplo.com`
- Una IP: `192.168.1.10`
- Una URL: `https://ejemplo.com` (se extrae automáticamente el hostname)

---

## ⚙️ Opciones

| Opción | Descripción | Valor por defecto |
|---|---|---|
| `objetivo` | **(Obligatorio)** Dominio, IP o URL a analizar. | — |
| `-o`, `--salida` | Ruta del archivo de resultados `.txt`. El JSON se guarda con el mismo nombre y extensión `.json`. | `enumeracion_<objetivo>.txt` |
| `-t`, `--timeout` | Tiempo máximo (segundos) para las peticiones HTTP y consultas DNS. | `10` |
| `--hilos` | Número de hilos para el escaneo de puertos y la resolución de subdominios. | `20` |
| `--sin-whois` | Omite la consulta WHOIS. | Desactivado |
| `--sin-subdominios` | Omite la búsqueda de subdominios en crt.sh. | Desactivado |
| `-h`, `--help` | Muestra la ayuda. | — |

### Variables de entorno

| Variable | Efecto |
|---|---|
| `NO_COLOR` | Si está definida, desactiva los colores en la consola. |

> Los colores también se desactivan automáticamente si la salida no es un terminal (por ejemplo, al redirigirla a un archivo o a otro comando).

---

## 💡 Ejemplos

**Enumeración completa de un dominio:**
```bash
python3 enumeracion.py ejemplo.com
```

**Guardar el informe en una ruta concreta:**
```bash
python3 enumeracion.py ejemplo.com -o resultados/ejemplo.txt
```

**Omitir WHOIS y subdominios (solo DNS y fase activa):**
```bash
python3 enumeracion.py ejemplo.com --sin-whois --sin-subdominios
```

**Aumentar timeout y número de hilos:**
```bash
python3 enumeracion.py ejemplo.com -t 20 --hilos 50
```

**Analizar una IP:**
```bash
python3 enumeracion.py 192.168.1.10 --sin-whois --sin-subdominios
```

**Sin colores:**
```bash
NO_COLOR=1 python3 enumeracion.py ejemplo.com
```

---

## 🔬 Funcionamiento detallado

El flujo de ejecución es el siguiente:

1. **Validación** del objetivo (IP, dominio o URL).
2. **WHOIS** *(omitible con `--sin-whois`)*.
3. **Registros DNS** (`A`, `AAAA`, `MX`, `NS`, `TXT`, `CNAME`).
4. **Subdominios vía crt.sh** *(omitible con `--sin-subdominios`)*: consulta los certificados emitidos para `%.objetivo`, filtra y deduplica los nombres, y resuelve cada uno en paralelo para indicar cuáles están "vivos".
5. **Resolución de IP**. Si el objetivo no resuelve, se omite toda la parte de red y el script termina con código `2`.
6. **Análisis web**: prueba primero `https://` y luego `http://`; extrae cabeceras y comprueba ficheros comunes (`robots.txt`, `sitemap.xml`, `security.txt`).
7. **Escaneo de puertos** TCP sobre la lista de puertos comunes.
8. **Identificación de servicios**:
   - Con `nmap` disponible: `nmap -sV -Pn --script=default -p <puertos abiertos> <IP>`, mostrando producto, versión y la salida resumida de los scripts NSE.
   - Sin `nmap`: *banner grabbing* básico en los puertos 21, 22, 25, 80, 443 y 8080.
9. **Resumen final** y guardado de los informes.

---

## 📄 Salida generada

Cada ejecución crea dos archivos:

| Archivo | Contenido |
|---|---|
| `enumeracion_<objetivo>.txt` | Informe en texto plano, sin códigos de color, dividido por secciones con la duración de cada una. |
| `enumeracion_<objetivo>.json` | Datos estructurados por sección, útiles para procesarlos con otras herramientas. |

Estructura básica del JSON:

```json
{
  "fecha": "2025-01-01T12:00:00.000000",
  "secciones": {
    "FASE PASIVA · DNS": { "...": "..." },
    "FASE ACTIVA · PUERTOS": { "...": "..." }
  }
}
```

El resumen final incluye: puertos abiertos, servicios identificados por nmap, subdominios vivos y tecnologías/recursos web detectados.

---

## 🔢 Códigos de salida

| Código | Significado |
|---|---|
| `0` | Ejecución completada correctamente. |
| `2` | El objetivo no resuelve a una IP; se omitió el escaneo de red. |
| `≠ 0` (argparse) | Argumentos inválidos u objetivo no válido. |

---

## ⚠️ Limitaciones

- El escaneo de puertos se limita a **13 puertos comunes**, no a un rango completo.
- Solo se escanea **TCP**; no hay escaneo UDP.
- Solo se escanean puertos **IPv4** (la resolución usa `gethostbyname`).
- **crt.sh** puede responder con errores (502/504) o tardar; el script reintenta hasta 3 veces, pero puede no obtener resultados.
- La información de **WHOIS** varía según el TLD y puede estar parcialmente oculta por servicios de privacidad.
- El script `nmap` con `--script=default` es **intrusivo** y puede ser detectado por sistemas IDS/IPS.
- La detección de tecnologías web es básica (cabeceras y ficheros comunes); no sustituye a herramientas como Wappalyzer o WhatWeb.
