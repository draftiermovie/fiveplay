#!/usr/bin/env bash
# Builds FivePlay on Linux and puts a ready-to-run copy in ./dist/fiveplay.
#
#   bash tools/build-linux.sh
#
# Installs the system libraries Bevy needs (asks for your sudo password),
# installs or updates Rust via rustup, then compiles an optimised build.
# The first build downloads and compiles Bevy and can take 20-40 minutes.
set -euo pipefail
cd "$(dirname "$0")/.."

MIN_RUST="1.97.1"

say() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
fail() { printf '\n\033[1;31mError: %s\033[0m\n' "$*" >&2; exit 1; }

# 1. System libraries (compiler, X11/Wayland, ALSA audio, udev input).
say "Installing system libraries"
if command -v apt-get >/dev/null; then
    sudo apt-get update
    sudo apt-get install -y build-essential curl pkg-config libx11-dev libasound2-dev \
        libudev-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-0
elif command -v dnf >/dev/null; then
    sudo dnf install -y gcc gcc-c++ curl pkgconf-pkg-config libX11-devel alsa-lib-devel \
        systemd-devel wayland-devel libxkbcommon-devel libxkbcommon-x11
elif command -v pacman >/dev/null; then
    # Arch ships no Vulkan by default: add the loader plus the driver for the
    # detected GPU (NVIDIA's comes with its own driver package, nvidia-utils).
    vulkan="vulkan-icd-loader"
    gpu=$(lspci 2>/dev/null | grep -iE 'vga|3d|display' || true)
    echo "$gpu" | grep -qiE 'amd|ati|radeon' && vulkan="$vulkan vulkan-radeon"
    echo "$gpu" | grep -qi 'intel' && vulkan="$vulkan vulkan-intel"
    # shellcheck disable=SC2086
    sudo pacman -S --needed --noconfirm base-devel curl pkgconf libx11 alsa-lib systemd-libs \
        wayland libxkbcommon libxkbcommon-x11 $vulkan ||
        fail "pacman could not install the packages. Update the system with 'sudo pacman -Syu', then re-run."
    if ! echo "$gpu" | grep -qiE 'amd|ati|radeon|intel|nvidia'; then
        echo "Could not detect the GPU (lspci missing?). If the game will not start, install"
        echo "vulkan-radeon (AMD), vulkan-intel (Intel) or nvidia-utils (NVIDIA)."
    fi
elif command -v zypper >/dev/null; then
    sudo zypper install -y gcc gcc-c++ curl pkg-config libX11-devel alsa-lib-devel \
        systemd-devel wayland-devel libxkbcommon-devel libxkbcommon-x11-0
else
    echo "Unknown package manager: install a C compiler, pkg-config and the X11, ALSA,"
    echo "udev, Wayland and xkbcommon development packages yourself, then re-run."
fi

# 2. Rust toolchain (distro Rust packages are usually too old for Bevy 0.20).
say "Checking Rust (needs $MIN_RUST or newer)"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if command -v rustup >/dev/null; then
    rustup update stable
    rustup default stable
else
    echo "Installing Rust with rustup (https://rustup.rs)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi
version=$(rustc --version | awk '{print $2}')
if [ "$(printf '%s\n%s\n' "$MIN_RUST" "$version" | sort -V | head -n1)" != "$MIN_RUST" ]; then
    fail "rustc $version is older than $MIN_RUST. Remove the distro's rust package and re-run."
fi

# 3. Build.
say "Compiling FivePlay (first build takes a while)"
cargo build --release --locked

# 4. Package: the game finds its assets folder next to the executable.
say "Packaging into dist/fiveplay"
rm -rf dist/fiveplay
mkdir -p dist/fiveplay
cp target/release/fiveplay dist/fiveplay/
cp -r assets dist/fiveplay/
cat > dist/fiveplay/run.sh <<'EOF'
#!/usr/bin/env bash
cd "$(dirname "$0")" && exec ./fiveplay "$@"
EOF
chmod +x dist/fiveplay/run.sh dist/fiveplay/fiveplay

say "Done! Start the game with:  ./dist/fiveplay/run.sh"
echo "The dist/fiveplay folder is self-contained: copy it anywhere (keep 'assets' beside 'fiveplay')."
