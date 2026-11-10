# Instalación

## Windows

Descarga el instalador `.msi` o el `.zip` portátil desde la
[página de releases](https://github.com/jimmykedlyn/Heric/releases). El
instalador requiere Windows 10 o posterior en x86_64.

## Linux

Descarga el paquete `.deb` para Debian y Ubuntu, el `.AppImage` para otras
distribuciones o el archivo `.tar.gz`. En x86_64, las dependencias de
ejecución son las bibliotecas de GTK 3 y las del cliente X11, que la mayoría
de las instalaciones de escritorio ya tienen.

```bash
# Debian/Ubuntu
sudo dpkg -i heric_amd64.deb
```

## macOS

Descarga la imagen `.dmg`. Contiene un binario universal que se ejecuta en
Macs con Apple Silicon y con Intel.

En el primer uso, macOS puede pedir confirmación de que la app se descargó.
Si la build no está notarizada (las notas de la release lo dicen), haz clic
con el botón derecho en la app y elige Abrir.

## Compilando desde el código fuente

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --release --workspace
```

Los binarios quedan en `target/release/heric` (aplicación de escritorio) y
`target/release/heric-cli` (línea de comandos). Compilar en Linux requiere
los paquetes listados en
[CONTRIBUTING](https://github.com/jimmykedlyn/Heric/blob/develop/CONTRIBUTING.es.md).
