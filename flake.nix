{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, crane, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; };
      craneLib = crane.lib.${system};
      libPath = with pkgs; lib.makeLibraryPath [
        libxkbcommon
        vulkan-loader

        # wayland # maybe after I support wayland
        libGL

        xorg.libXcursor
        xorg.libXi
        xorg.libXrandr
        xorg.libxcb
        xorg.libX11
      ];

      nativeBuildInputs = with pkgs; [
        pkg-config
      ];

      buildInputs = with pkgs; [
        mold

        libxkbcommon
        vulkan-loader

        # wayland # maybe after I support wayland
        libGL

        xorg.libXcursor
        xorg.libXi
        xorg.libXrandr
        xorg.libxcb
        xorg.libX11
      ];

      cargoArtifacts = craneLib.buildDepsOnly ({
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        inherit buildInputs nativeBuildInputs;
        pname = "shareet";
      });
    in
    with pkgs; {
      packages.${system} = rec {
        shareet = craneLib.buildPackage {
          src = craneLib.path ./.;
          inherit buildInputs nativeBuildInputs cargoArtifacts;
          postFixup = ''
            patchelf $out/bin/shareet \
                --add-rpath ${lib.makeLibraryPath [ vulkan-loader ]}
          '';
        };

        default = shareet;
      };

      devShells.${system}.default = mkShell {
        NIX_CFLAGS_LINK = "-fuse-ld=mold";
        packages = [
          (rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
          })
          gdb
          cargo-watch

          # for debugging renderer
          renderdoc

          # for profiling
          linuxKernel.packages.linux_6_4.perf
          heaptrack
        ];

        inherit buildInputs nativeBuildInputs;

        LD_LIBRARY_PATH = libPath;
      };

      formatter.x86_64-linux = nixpkgs-fmt;
    };
}

