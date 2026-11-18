# SPDX-FileCopyrightText: 2025 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

{
  description = "Prime BLE firmware development environment with local cargo/rustup dirs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # v3.0.0
    cosign2.url = "github:Foundation-Devices/cosign2/45c233c1a0c08f06c5cafc36b754ba6323c5ed86";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      cosign2,
      fenix,
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      forAllSystems = f: lib.genAttrs systems f;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };
          ci-pkgs = with pkgs; {
            inherit just cargo-sort;
          };
        in
        ci-pkgs
        // import ./nix/rust-toolchain.nix {
          inherit
            self
            system
            pkgs
            fenix
            ;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };

          cosign2Pkgs = cosign2.packages.${system};

          customPackages = self.packages.${system};

          nrf-recover = pkgs.rustPlatform.buildRustPackage rec {
            pname = "nrf-recover";
            version = "0.1.0";
            src = pkgs.fetchCrate {
              inherit pname version;
              hash = "sha256-IHkCHhYfZscQACNiRFaN7w0oUy0vrSPM5AWZLdYnJ2Q=";
            };
            cargoHash = "sha256-0/mpBfFldWCjMQg7x4DVEo2CUu5iVn81S/kAqOM2Aeo=";
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.rustfmt
            ];
            buildInputs = [ pkgs.libusb1.dev ];
          };

          buildPackages =
            with pkgs;
            [
              cargo-binutils
              cargo-sort
              gcc-arm-embedded
              git
              just
            ]
            ++ [
              cosign2Pkgs.cosign2
            ]
            ++ (with customPackages; [
              rust-ble-firmware
            ]);

          devPackages =
            buildPackages
            ++ (with customPackages; [
              rust-analyzer
            ])
            ++ (with pkgs; [
              pkg-config # for host-ble tool
              probe-rs # for flashing/debugging
            ])
            ++ ([
              nrf-recover
            ]);

          darwinPackages =
            let
              xcodeenv = import (nixpkgs + "/pkgs/development/mobile/xcodeenv") { inherit (pkgs) callPackage; };
            in
            lib.optionals pkgs.stdenv.isDarwin [
              (xcodeenv.composeXcodeWrapper { versions = [ "16.0" ]; })
            ];

          linuxPackages =
            with pkgs;
            lib.optionals stdenv.isLinux [
              clang
              llvmPackages.libclang
              llvmPackages.llvm
            ];

          linuxAttrs = lib.optionalAttrs pkgs.stdenv.isLinux {
            # for bindgen in c++ libs
            # macos already has xcode clang
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          };

          mkShell =
            packages:
            pkgs.mkShellNoCC (
              {
                strictDeps = true;
                packages = packages ++ linuxPackages ++ darwinPackages;
                hardeningDisable = [ "all" ];
                buildInputs = with pkgs; [
                  dbus.dev # for host-ble tool
                ];

                LD_LIBRARY_PATH =
                  with pkgs;
                  lib.makeLibraryPath (
                    [
                    ]
                    ++ lib.optionals stdenv.isLinux [
                      llvmPackages.libclang.lib
                    ]
                  );

                shellHook = ''
                  # darwin xcode
                  unset DEVELOPER_DIR
                  unset SDKROOT

                  # unset clang env variables
                  unset CC
                  unset CXX
                  unset AR
                  unset RANLIB
                  	  
                  export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD 2>/dev/null || echo "0")
                  export TZ=UTC
                  export CARGO_HOME=$PWD/.cargo
                  export CARGO_NET_GIT_FETCH_WITH_CLI=true
                  export CARGO_INCREMENTAL=0
                  export PATH=$PATH:''${CARGO_HOME}/bin
                  # export CC="arm-none-eabi-gcc"
                  export CC_thumbv7em_none_eabi="arm-none-eabi-gcc"
                  export PROBE_RS_PROTOCOL=swd
                  export PROBE_RS_CHIP=nrf52805_xxAA
                '';
              }
              // linuxAttrs
            );
        in
        {
          # full development shell
          default = mkShell devPackages;
          # minimal build shell
          build = mkShell buildPackages;
        }
      );
    };
}
