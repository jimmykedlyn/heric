# Requests e environments

## Requests

Um request carrega método, URL, headers, body e configurações como timeouts,
redirects e certificados. A aninhagem em pastas (request groups) funciona
como no cliente original: pastas podem conter requests e outras pastas.

## Environments

Todo workspace tem um **environment base**, criado automaticamente, e quantos
**sub-environments** você quiser. Sub-environments sobrescrevem o environment
base chave a chave, e sub-environments aninhados encadeiam do mesmo jeito que
no original.

Os dados de environment são uma lista de pares chave-valor. Os valores são
strings simples; marcar um par como secret guarda o valor criptografado (veja
[Segurança](#segurança)).

## Segurança

Os secrets ficam no armazenamento de credenciais da plataforma: Keychain no
macOS, Credential Manager no Windows e Secret Service no Linux.
