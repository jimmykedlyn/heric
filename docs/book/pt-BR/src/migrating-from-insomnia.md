# Migrando do Insomnia

Previsto para o M2. Heric lê uma instalação existente do Insomnia no lugar e
importa cada workspace, environment, pasta, request e cookie jar sem perdas.
O diretório de dados fica em:

| Plataforma | Caminho |
|---|---|
| Windows | `%APPDATA%\Insomnia` |
| Linux | `~/.config/Insomnia` |
| macOS | `~/Library/Application Support/Insomnia` |

A migração roda uma vez por importação e nunca escreve no diretório original.
