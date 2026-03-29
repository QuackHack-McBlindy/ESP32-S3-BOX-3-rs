{
  description = "ESP32-S3 Rust dev shell (with espup)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Host Rust for building host-side tools, includes rust-src
        rustNightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
          toolchain.default.override {
            extensions = (toolchain.default.extensions or []) ++ [ "rust-src" ];
          }
        );
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.curl
            pkgs.git
            pkgs.pkg-config
            pkgs.openssl
            pkgs.cmake
            pkgs.clang
            pkgs.usbutils
            pkgs.perl
            pkgs.python3
            rustNightly
            pkgs.rustup
          ];

          shellHook = ''
            export CARGO_TARGET_DIR="$PWD/target"
            export RUSTUP_HOME="$HOME/.rustup"
            export CARGO_HOME="$HOME/.cargo"

            # Install espup if missing
            if ! command -v espup &> /dev/null; then
              cargo +stable install espup --locked
            fi

            # Install ESP32-S3 toolchain and set up paths
            espup install

            # Add espup toolchains to PATH for cross-compilation
            export PATH="$HOME/.espup/toolchains/xtensa-esp32s3-elf/bin:$PATH"
            export LIBCLANG_PATH="$HOME/.espup/toolchains/xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib"

            echo "🦆 ESP32-S3 dev shell ready! Use: cargo +nightly build --release --target xtensa-esp32s3-none-elf"
          '';
        };
      }
    );
}
