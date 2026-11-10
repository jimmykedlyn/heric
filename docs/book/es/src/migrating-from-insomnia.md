# Migrando desde Insomnia

Previsto para M2. Heric lee una instalación existente de Insomnia en el lugar
e importa cada workspace, environment, carpeta, request y cookie jar sin
pérdidas. El directorio de datos está en:

| Plataforma | Ruta |
|---|---|
| Windows | `%APPDATA%\Insomnia` |
| Linux | `~/.config/Insomnia` |
| macOS | `~/Library/Application Support/Insomnia` |

La migración se ejecuta una vez por importación y nunca escribe en el
directorio original.
