# Interface de linha de comando

`heric-cli` executa collections e testes sem abrir a aplicação. O alvo é a
paridade com a CLI original:

```bash
heric-cli run collection.yaml --env production
heric-cli test collection.yaml
heric-cli export collection.yaml --output out.yaml
```

Previsto para o M6.
