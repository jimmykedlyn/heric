# Installation

## Windows

Download the `.msi` installer or the portable `.zip` from the
[releases page](https://github.com/jimmykedlyn/Heric/releases). The installer
requires Windows 10 or newer on x86_64.

## Linux

Download the `.deb` package for Debian and Ubuntu, the `.AppImage` for other
distributions, or the `.tar.gz` archive. On x86_64, the runtime dependencies
are the GTK 3 libraries and the X11 client libraries, which most desktop
installations already have.

```bash
# Debian/Ubuntu
sudo dpkg -i heric_amd64.deb
```

## macOS

Download the `.dmg` image. It contains a universal binary that runs on Apple
Silicon and Intel Macs.

On first launch, macOS may ask to confirm that you downloaded the app. If the
build is not notarized (the release notes say so), right-click the app and
choose Open.

## From source

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --release --workspace
```

The binaries are `target/release/heric` (desktop application) and
`target/release/heric-cli` (command line). Building on Linux needs the
packages listed in [CONTRIBUTING](https://github.com/jimmykedlyn/Heric/blob/develop/CONTRIBUTING.md).
