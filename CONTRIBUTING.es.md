[English](CONTRIBUTING.md) | [Português](CONTRIBUTING.pt-BR.md) | Español

# Contribuir con Heric

Gracias por ayudar. Este documento explica el flujo de ramas, los quality
gates y el proceso de revisión. Se aplica a todo cambio, incluida la
documentación.

## Código de conducta

Sé respetuoso. Discute el código, no a la persona. Las discrepancias de diseño
se resuelven con evidencia del cliente original o con un test, no por
jerarquía.

## Modelo de ramas (Gitflow)

| Rama | Origen | Destino | Uso |
|---|---|---|---|
| `main` | — | — | Solo código publicado. Cada commit en `main` tiene una etiqueta `vX.Y.Z`. |
| `develop` | `main` | — | Integración. Rama predeterminada del repositorio. |
| `feature/<crate>-<tarea>` | `develop` | `develop` | Toda funcionalidad, corrección, docs o CI. |
| `release/X.Y.0` | `develop` | `main` + `develop` | Preparación de release: bump de versión, changelog, solo correcciones. |
| `hotfix/X.Y.Z` | `main` | `main` + `develop` | Corrección urgente en una versión publicada. |

Reglas:

- Nunca hagas commit directamente en `main` o `develop`. Todo entra por pull
  request.
- `feature/*` apunta a `develop` y entra por squash merge. Borra la rama
  después del merge.
- `release/*` y `hotfix/*` apuntan a `main` con merge commit, sin squash.
  Después del merge, crea la etiqueta `vX.Y.Z` en `main` y abre un PR de
  back-merge de `main` a `develop`.
- En `release/*` solo entran bumps de versión, `CHANGELOG.md`, ajustes de
  documentación y correcciones de bugs. Nada de funcionalidad nueva.
- Haz rebase de tu rama de feature sobre `develop` antes del merge si
  `develop` avanzó.

El job `gitflow` del CI valida el par origen/destino en cada PR y falla en
cualquier otra combinación.

## Preparación del entorno

1. Instala la cadena de herramientas de Rust. `rust-toolchain.toml` fija el
   canal y los componentes, así que `rustup` los aplica solo.
2. En Linux, instala las dependencias del escritorio: `libxkbcommon-dev`,
   `libgtk-3-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`,
   `libxcb-xfixes0-dev`, `libssl-dev` y `protobuf-compiler`.
3. Compila y prueba:

```bash
cargo build --workspace --all-targets
cargo test --workspace
```

## Quality gates

Un cambio solo se puede fusionar cuando todo esto pasa en local y en el CI de
las tres plataformas:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Reglas de código

- Nada de `unwrap()` ni `expect()` en código de biblioteca. Las bibliotecas
  usan `thiserror`; solo los binarios usan `anyhow`.
- Mantén funciones pequeñas, documentadas en rustdoc y libres de rutas
  específicas de plataforma. Usa `directories` para configuración y datos.
- Un comportamiento nuevo exige un test que falle antes (red, green,
  refactor).
- El comportamiento debe coincidir con el cliente original. Cita el archivo y
  la línea de origen en el comentario del código o en el cuerpo del PR al
  implementar paridad.

## Mensajes de commit

Conventional Commits, en inglés:

```
feat: add response timeline panel
fix(http): keep trailers on chunked responses
docs: explain template tag chaining
```

Usa un scope con la crate afectada cuando ayude.

## Pull requests

- Apunta a `develop` (o a `main` solo desde `release/*` y `hotfix/*`).
- Referencia la issue con `Closes #123`.
- Completa el checklist de la plantilla de PR.
- Si el cambio afecta el comportamiento visible, actualiza la documentación en
  inglés en el mismo PR e incluye las traducciones `pt-BR` y `es`.
- Espera el CI en las tres plataformas. Una plataforma en rojo bloquea el
  merge.

## Documentación

El inglés es la fuente de verdad. Todo cambio en él se envía con las
actualizaciones en `pt-BR` y `es` en el mismo PR. Los bloques de código,
comandos y nombres de flags nunca se traducen. Sigue la terminología de
`docs/book/GLOSSARY.md`.

## Reportar bugs y pedir funcionalidades

Usa las plantillas de issue. En bugs, incluye el sistema operativo, la versión
(visible en el diálogo Acerca de) y la reproducción más pequeña posible.

## Licencia

Al contribuir aceptas que tu contribución se licencia bajo la licencia MIT, y
que las partes derivadas de Insomnia siguen bajo la Apache License 2.0, como
se describe en `NOTICE`.
