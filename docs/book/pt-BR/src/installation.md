# Instalação

## Windows

Baixe o instalador `.msi` ou o `.zip` portátil na
[página de releases](https://github.com/jimmykedlyn/Heric/releases). O
instalador exige Windows 10 ou mais recente em x86_64.

## Linux

Baixe o pacote `.deb` para Debian e Ubuntu, o `.AppImage` para outras
distribuições ou o arquivo `.tar.gz`. Em x86_64, as dependências de execução
são as bibliotecas do GTK 3 e as do cliente X11, que a maioria das
instalações desktop já possui.

```bash
# Debian/Ubuntu
sudo dpkg -i heric_amd64.deb
```

## macOS

Baixe a imagem `.dmg`. Ela contém um binário universal que roda em Macs com
Apple Silicon e com Intel.

No primeiro uso, o macOS pode pedir confirmação de que o app foi baixado. Se
a build não for notarizada (as notas da release dizem), clique com o botão
direito no app e escolha Abrir.

## Compilando do código-fonte

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --release --workspace
```

Os binários ficam em `target/release/heric` (aplicativo desktop) e
`target/release/heric-cli` (linha de comando). Compilar no Linux exige os
pacotes listados no
[CONTRIBUTING](https://github.com/jimmykedlyn/Heric/blob/develop/CONTRIBUTING.pt-BR.md).
