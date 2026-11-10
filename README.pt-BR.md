[English](README.md) | Português | [Español](README.es.md)

# Heric

Cliente de API nativo em Rust com interface egui/eframe, compatível com os
dados e o fluxo de trabalho do Insomnia.

> O Heric é um projeto de código aberto independente. Não é afiliado,
> endossado nem patrocinado pela Kong Inc. É inspirado no cliente de API
> Insomnia e busca ser compatível com ele.

## Estado

Desenvolvimento inicial. O repositório contém por enquanto o esqueleto do
workspace, a configuração de CI e o plano do projeto. A primeira versão
utilizável, com um request real de ponta a ponta, está prevista para o
milestone M1.

## Objetivos

- Paridade de funcionalidades com o cliente original: HTTP, GraphQL, gRPC,
  WebSocket, SSE, environments, template tags, scripts, import/export e
  geração de código.
- Ler instalações existentes do Insomnia e importar/exportar o formato dele
  sem perdas.
- Rodar nativamente em Windows (x86_64), Linux (x86_64) e macOS (Apple Silicon
  e Intel).
- Manter a interface responsiva: a execução de requests roda fora da thread
  de UI.

## Compilação

Pré-requisitos: toolchain Rust (veja `rust-toolchain.toml`) e, no Linux, os
pacotes de desenvolvimento listados na documentação.

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --workspace
```

Executar o aplicativo desktop:

```bash
cargo run -p heric
```

Executar a interface de linha de comando:

```bash
cargo run -p heric-cli --version
```

## Estrutura do repositório

```
crates/     bibliotecas reutilizáveis (model, storage, template, http, auth, ...)
apps/       binários: gui (heric) e cli (heric-cli)
docs/       decisões de arquitetura, planos, inventário e os sites mdBook
```

## Documentação

- Inglês: https://heric.github.io/Heric/en/
- Português: https://heric.github.io/Heric/pt-BR/
- Espanhol: https://heric.github.io/Heric/es/

## Contribuição

Veja [CONTRIBUTING.pt-BR.md](CONTRIBUTING.pt-BR.md). O projeto usa Gitflow:
`develop` é o branch padrão e toda mudança entra por pull request.

## Licença

MIT. Veja [LICENSE](LICENSE).

Partes deste projeto derivam do Insomnia (Kong Inc.), licenciado sob a Apache
License 2.0. Veja [NOTICE](NOTICE) e
[licenses/APACHE-2.0-insomnia.txt](licenses/APACHE-2.0-insomnia.txt).
