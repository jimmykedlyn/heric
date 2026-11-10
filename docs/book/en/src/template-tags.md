# Template tags

Request fields accept Nunjucks templates. A template can reference
environment variables and call template tags:

```text
{{ base_url }}/users?since={{ timestamp('Y-m-d') }}
Authorization: Bearer {% uuid 'v4' %}
```

Heric reimplements the tag set of the original client, including `response`,
`prompt`, `uuid`, `timestamp`, `base64`, `hash` and `faker`. Tags can
reference the response of another request, and environments chain variables
the same way.

The compatibility target is the tag behavior documented by the original
client; differences found in practice are tracked as parity issues.
