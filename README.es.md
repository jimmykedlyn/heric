[English](README.md) | [Português](README.pt-BR.md) | Español

# Heric

Cliente de API nativo en Rust con interfaz egui/eframe, compatible con los
datos y el flujo de trabajo de Insomnia.

> Heric es un proyecto de código abierto independiente. No está afiliado,
> respaldado ni patrocinado por Kong Inc. Está inspirado en el cliente de API
> Insomnia y busca ser compatible con él.

## Estado

Desarrollo inicial. El repositorio contiene por ahora el esqueleto del
workspace, la configuración de CI y el plan del proyecto. La primera versión
utilizable, con una request real de extremo a extremo, está prevista para el
hito M1.

## Objetivos

- Paridad de funcionalidades con el cliente original: HTTP, GraphQL, gRPC,
  WebSocket, SSE, environments, template tags, scripts, importación/exportación
  y generación de código.
- Leer instalaciones existentes de Insomnia e importar/exportar su formato sin
  pérdidas.
- Ejecutarse de forma nativa en Windows (x86_64), Linux (x86_64) y macOS
  (Apple Silicon e Intel).
- Mantener la interfaz responsiva: la ejecución de requests corre fuera del
  hilo de UI.

## Compilación

Requisitos: cadena de herramientas de Rust (ver `rust-toolchain.toml`) y, en
Linux, los paquetes de desarrollo listados en la documentación.

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --workspace
```

Ejecutar la aplicación de escritorio:

```bash
cargo run -p heric
```

Ejecutar la interfaz de línea de comandos:

```bash
cargo run -p heric-cli --version
```

## Estructura del repositorio

```
crates/     bibliotecas reutilizables (model, storage, template, http, auth, ...)
apps/       binarios: gui (heric) y cli (heric-cli)
docs/       decisiones de arquitectura, planes, inventario y los sitios mdBook
```

## Documentación

- Inglés: https://heric.github.io/Heric/en/
- Portugués: https://heric.github.io/Heric/pt-BR/
- Español: https://heric.github.io/Heric/es/

## Contribuir

Ver [CONTRIBUTING.es.md](CONTRIBUTING.es.md). El proyecto usa Gitflow:
`develop` es la rama predeterminada y todo cambio entra por pull request.

## Licencia

MIT. Ver [LICENSE](LICENSE).

Partes de este proyecto derivan de Insomnia (Kong Inc.), licenciado bajo la
Apache License 2.0. Ver [NOTICE](NOTICE) y
[licenses/APACHE-2.0-insomnia.txt](licenses/APACHE-2.0-insomnia.txt).
