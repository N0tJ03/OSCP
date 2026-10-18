# Máquinas

Writeups y notas de las máquinas que voy resolviendo durante la preparación del **OSCP**.
Cada máquina tiene su propia carpeta con el procedimiento, los comandos usados y las lecciones aprendidas.

## Plataformas

- [Hack The Box](https://www.hackthebox.com/)
- [Proving Grounds (Offensive Security)](https://www.offsec.com/labs/)
- [TryHackMe](https://tryhackme.com/)
- [VulnHub](https://www.vulnhub.com/)

## Registro de máquinas

| Máquina | Plataforma | SO | Dificultad | Técnicas clave | Estado |
|---------|------------|----|------------|----------------|--------|
| Ejemplo | HTB | Linux | Easy | SQLi, sudo misconfig | ✅ |
|         |            |    |            |                |        |

**Leyenda:** ✅ Completada · 🔄 En progreso · ❌ Con ayuda / pendiente de repetir

## Estructura

```
maquinas/
├── README.md
├── htb/
│   └── nombre-maquina/
│       ├── README.md      # writeup
│       └── img/           # capturas
├── proving-grounds/
├── tryhackme/
└── vulnhub/
```

## Plantilla de writeup

Cada máquina sigue esta estructura:

```markdown
# Nombre de la máquina

**Plataforma:** HTB | **SO:** Linux | **Dificultad:** Easy | **IP:** 10.10.10.X

## Resumen
Breve descripción de la cadena de ataque.

## Reconocimiento
Escaneo de puertos y servicios (nmap, etc.).

## Enumeración
Hallazgos por servicio.

## Acceso inicial
Vulnerabilidad explotada y pasos seguidos.

## Escalada de privilegios
Vector utilizado hasta conseguir root/SYSTEM.

## Lecciones aprendidas
- Qué ha funcionado
- Dónde me he atascado
- Qué haría distinto

## Herramientas y referencias
- Herramientas usadas
- Enlaces útiles
```

## Metodología

1. Reconocimiento y escaneo completo de puertos
2. Enumeración de cada servicio
3. Búsqueda de vulnerabilidades y acceso inicial
4. Post-explotación y escalada de privilegios
5. Documentación de todo el proceso
