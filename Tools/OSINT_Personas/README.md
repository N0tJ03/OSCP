# 🔎 osint_v2

**OSINT enfocado en la búsqueda de personas.** Parte de un solo dato (email, nombre, username o teléfono) y reúne en un reporte lo que hay en fuentes abiertas: cuentas en redes, filtraciones, perfiles y búsquedas listas para abrir.

Gratis. Sin API keys. Sin registrarse en nada.

---

## ⚡ Instalación rápida

```bash
git clone https://github.com/JRXsec/OSCP-Master.git && cd OSCP-Master/Tools/osint_v2

pip3 install --break-system-packages -r requirements.txt

python3 main.py
```

---

## 🧠 Cómo funciona

1. **Le das un dato** de la persona. Detecta solo si es un email, un nombre, un username o un teléfono.
2. **Lanza todas las fuentes a la vez**, con una barra de progreso para cada una. Si una falla, las demás siguen.
3. **Te enseña los resultados** en la terminal y genera un **reporte HTML** con enlaces clicables y un buscador, que se abre solo en el navegador.

---

## 👤 Qué saca de cada dato

| Empiezas con… | Obtienes |
|---|---|
| 📧 **Email** | Filtraciones en las que aparece, perfil de Gravatar, sitios donde está registrado y cuentas con el mismo usuario |
| 🪪 **Nombre completo** | Todas las combinaciones del nombre (Pepe Pepito García → Pepe García, Pepe Pepito…), con y sin tildes, buscadas en ~700 redes, Google y LinkedIn |
| 🏷️ **Username** | Cuentas con ese nombre de usuario en ~700 sitios |
| 📱 **Teléfono** | País, zona, operador, tipo de línea, enlace a WhatsApp y Telegram, y búsquedas del número en todos sus formatos |
| 🌐 **Dominio** | WHOIS, DNS, subdominios y servicios que usa |

---

## 🛠️ Uso

```bash
python3 main.py                          # menú interactivo
python3 main.py pepe@gmail.com           # detecta el tipo solo
python3 main.py "Pepe Pepito García"     # nombre completo
python3 main.py +34600123456             # teléfono
```

Los reportes se guardan en `reports/`.
