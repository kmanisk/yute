# Maintainer: Kumar Manisk <mathewmanisk699@gmail.com>
pkgname=yute-git
pkgver=0.1.0.r0.g0000000
pkgrel=1
pkgdesc="Fast, keyboard-driven YouTube terminal client in Rust with personal recommendations, Sixel/Kitty thumbnails, and MPV playback"
arch=('x86_64')
url="https://github.com/kmanisk/yute"
license=('MIT')
depends=('gcc-libs' 'glibc' 'mpv' 'yt-dlp')
optdepends=(
    'ffmpeg: Local video durations and thumbnail extraction'
    'wl-copy: Wayland clipboard integration'
    'xclip: X11 clipboard integration'
)
makedepends=('cargo' 'git')
provides=('yute')
conflicts=('yute')
source=("git+file://$(pwd)")
sha256sums=('SKIP')

pkgver() {
    cd "$srcdir/$pkgname" 2>/dev/null || cd "$startdir"
    printf "0.1.0.r%s.g%s" "$(git rev-list --count HEAD 2>/dev/null || echo 1)" "$(git rev-parse --short HEAD 2>/dev/null || echo "init")"
}

build() {
    cd "$srcdir/$pkgname" 2>/dev/null || cd "$startdir"
    export CARGO_TARGET_DIR="target"
    cargo build --release
}

check() {
    cd "$srcdir/$pkgname" 2>/dev/null || cd "$startdir"
    cargo test --release
}

package() {
    cd "$srcdir/$pkgname" 2>/dev/null || cd "$startdir"
    install -Dm755 target/release/yute "$pkgdir/usr/bin/yute"
    install -Dm644 README.md "$pkgdir/usr/share/doc/yute/README.md"
}
