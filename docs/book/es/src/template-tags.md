# Template tags

Los campos de un request aceptan plantillas Nunjucks. Una plantilla puede
referenciar variables de environment y llamar template tags:

```text
{{ base_url }}/users?since={{ timestamp('Y-m-d') }}
Authorization: Bearer {% uuid 'v4' %}
```

Heric reimplementa el conjunto de tags del cliente original, incluidos
`response`, `prompt`, `uuid`, `timestamp`, `base64`, `hash` y `faker`. Las
tags pueden referenciar la respuesta de otro request, y los environments
encadenan variables de la misma forma.

El objetivo de compatibilidad es el comportamiento de las tags documentado
por el cliente original; las diferencias encontradas en la práctica se
registran como una issue de paridad.
