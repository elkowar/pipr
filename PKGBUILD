# Maintainer: Leon Kowarschick <lkowarschick at gmail dot com>
pkgname=pipr-git
pkgver=r66.2ca7327
pkgrel=1
makedepends=('rust' 'cargo' 'git')
depends=('gcc-libs')
optdepends=('bubblewrap: run commands in isolation. STRONGLY RECOMMENDED!')
arch=('i686' 'x86_64' 'armv6h' 'armv7h')
pkgdesc="A commandline-utility to interactively build complex shell pipelines"
license=('MIT')
source=('pipr-git::git+https://gitlab.com/Elkowar/pipr.git')
url="https://gitlab.com/Elkowar/pipr"
md5sums=('SKIP')

pkgver() {
  cd "$pkgname"
  ( set -o pipefail
    git describe --long 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' ||
    printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
  )
}

build() {
    cd "$srcdir/pipr-git"
    cargo build --release --locked --all-features
}

package() {
    cd "$srcdir/pipr-git"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm 755 target/release/pipr -t "${pkgdir}/usr/bin"
}
