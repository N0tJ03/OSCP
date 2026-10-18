# osint_v2

Herramienta OSINT con fuentes gratuitas y sin API keys. Acepta email, username, nombre completo, teléfono o dominio.

## Instalar (Kali)

```bash
python3 -m venv venv
source venv/bin/activate
pip3 install -r requirements.txt
```

## Usar

```bash
source venv/bin/activate
python3 main.py                    # menú
python3 main.py usuario@gmail.com  # detecta el tipo solo
python3 main.py -t name "Juan Pérez"
```

Los reportes se guardan en `reports/` (HTML + JSON). El HTML se abre solo en el navegador.

holehe, sherlock y maigret son opcionales: si no están instalados, salen como "omitido".
