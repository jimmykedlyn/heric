# Template tags

Os campos de um request aceitam templates Nunjucks. Um template pode
referenciar variáveis de environment e chamar template tags:

```text
{{ base_url }}/users?since={{ timestamp('Y-m-d') }}
Authorization: Bearer {% uuid 'v4' %}
```

Heric reimplementa o conjunto de tags do cliente original, incluindo
`response`, `prompt`, `uuid`, `timestamp`, `base64`, `hash` e `faker`. As
tags podem referenciar a resposta de outro request, e os environments
encadeiam variáveis do mesmo jeito.

O alvo de compatibilidade é o comportamento das tags documentado pelo cliente
original; diferenças encontradas na prática viram uma issue de paridade.
