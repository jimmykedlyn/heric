[English](CONTRIBUTING.md) | Português | [Español](CONTRIBUTING.es.md)

# Contribuindo com o Heric

Obrigado por ajudar. Este documento explica o fluxo de branches, os quality
gates e o processo de revisão. Ele vale para toda mudança, inclusive
documentação.

## Código de conduta

Seja respeitoso. Discuta o código, não a pessoa. Discordâncias de design são
resolvidas com evidência do cliente original ou com um teste, não por
hierarquia.

## Modelo de branches (Gitflow)

| Branch | Origem | Destino | Uso |
|---|---|---|---|
| `main` | — | — | Só código lançado. Todo commit em `main` tem uma tag `vX.Y.Z`. |
| `develop` | `main` | — | Integração. Branch padrão do repositório. |
| `feature/<crate>-<tarefa>` | `develop` | `develop` | Toda funcionalidade, correção, docs ou CI. |
| `release/X.Y.0` | `develop` | `main` + `develop` | Preparação de release: bump de versão, changelog, só correções. |
| `hotfix/X.Y.Z` | `main` | `main` + `develop` | Correção urgente em versão lançada. |

Regras:

- Nunca commite diretamente em `main` ou `develop`. Tudo entra por pull
  request.
- `feature/*` aponta para `develop` e entra por squash merge. Apague o branch
  depois do merge.
- `release/*` e `hotfix/*` apontam para `main` com merge commit, sem squash.
  Depois do merge, crie a tag `vX.Y.Z` em `main` e abra um PR de back-merge de
  `main` para `develop`.
- Em `release/*` entram apenas bump de versão, `CHANGELOG.md`, ajustes de
  documentação e correções de bugs. Nada de funcionalidade nova.
- Faça rebase do seu branch de feature em `develop` antes do merge se o
  `develop` andou.

O job `gitflow` do CI valida o par origem/destino em todo PR e falha em
qualquer outra combinação.

## Preparação do ambiente

1. Instale a toolchain Rust. O `rust-toolchain.toml` fixa o canal e os
   componentes, então o `rustup` aplica sozinho.
2. No Linux, instale as dependências do desktop: `libxkbcommon-dev`,
   `libgtk-3-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`,
   `libxcb-xfixes0-dev`, `libssl-dev` e `protobuf-compiler`.
3. Compile e teste:

```bash
cargo build --workspace --all-targets
cargo test --workspace
```

## Quality gates

Uma mudança só pode ser mesclada quando tudo isto passa localmente e no CI
das três plataformas:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Regras de código

- Nada de `unwrap()` ou `expect()` em código de biblioteca. Bibliotecas usam
  `thiserror`; apenas binários usam `anyhow`.
- Mantenha funções pequenas, documentadas em rustdoc e livres de caminhos
  específicos de plataforma. Use `directories` para config e dados.
- Comportamento novo exige teste falhando antes (red, green, refactor).
- O comportamento precisa bater com o cliente original. Cite o arquivo e a
  linha de origem no comentário do código ou no corpo do PR ao implementar
  paridade.

## Mensagens de commit

Conventional Commits, em inglês:

```
feat: add response timeline panel
fix(http): keep trailers on chunked responses
docs: explain template tag chaining
```

Use um escopo com a crate afetada quando ajudar.

## Pull requests

- Aponte para `develop` (ou para `main` só a partir de `release/*` e
  `hotfix/*`).
- Referencie a issue com `Closes #123`.
- Preencha o checklist do template de PR.
- Se a mudança afeta o comportamento visível, atualize a documentação em
  inglês no mesmo PR e inclua as traduções `pt-BR` e `es`.
- Espere o CI nas três plataformas. Uma plataforma vermelha bloqueia o merge.

## Documentação

O inglês é a fonte de verdade. Toda mudança nele é enviada com as atualizações
em `pt-BR` e `es` no mesmo PR. Blocos de código, comandos e nomes de flags
nunca são traduzidos. Siga a terminologia de `docs/book/GLOSSARY.md`.

## Reportando bugs e pedindo funcionalidades

Use os templates de issue. Em bugs, inclua o sistema operacional, a versão
(visível no diálogo Sobre) e a menor reprodução possível.

## Licença

Ao contribuir você concorda que sua contribuição é licenciada sob a licença
MIT, e que as partes derivadas do Insomnia continuam sob a Apache License 2.0,
conforme descrito no `NOTICE`.
