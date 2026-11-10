# Scripts y tests

Previsto para M5. Los scripts de pre-request y post-response corren en una
sandbox con la API `insomnia.*` y `pm.*`. Los environments se pueden leer y
escribir desde los scripts, y los tests de collection verifican respuestas
igual que el cliente original.
