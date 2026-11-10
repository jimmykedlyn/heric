# Requests y environments

## Requests

Un request lleva método, URL, headers, body y configuraciones como timeouts,
redirecciones y certificados. El anidamiento en carpetas (request groups)
funciona como en el cliente original: las carpetas pueden contener requests y
otras carpetas.

## Environments

Cada workspace tiene un **environment base**, creado automáticamente, y
tantos **sub-environments** como quieras. Los sub-environments sobrescriben
el environment base clave por clave, y los sub-environments anidados se
encadenan igual que en el original.

Los datos del environment son una lista de pares clave-valor. Los valores son
strings simples; marcar un par como secret guarda el valor cifrado (ver
[Seguridad](#seguridad)).

## Seguridad

Los secrets se guardan en el almacén de credenciales de la plataforma:
Keychain en macOS, Credential Manager en Windows y Secret Service en Linux.
