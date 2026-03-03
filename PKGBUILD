pkgname=grunner
pkgver=1.0.0
pkgrel=1
pkgdesc="A fast, keyboard-driven application launcher for GNOME"
arch=('x86_64')
url="https://github.com/Nihmar/grunner"
license=('MIT')
depends=('gtk4')
makedepends=('rust' 'cargo')

_srcdir="$PWD"

build() {
    cd "$_srcdir"
    RUSTFLAGS="-C target-cpu=native" cargo build --release
}

package() {
    cd "$_srcdir"

    # Binary
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

    # Icon
    install -Dm644 "assets/$pkgname.svg" "$pkgdir/usr/share/icons/hicolor/scalable/apps/$pkgname.svg"

    # Desktop entry (see below)
    install -Dm644 "packaging/$pkgname.desktop" "$pkgdir/usr/share/applications/$pkgname.desktop"
}
