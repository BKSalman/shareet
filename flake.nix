{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {nixpkgs,  rust-overlay, ...}:
      let 
        system = "x86_64-linux";
        pkgs = import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; };
      in
    with pkgs; {
      devShells.${system}.default = mkShell rec {
          NIX_CFLAGS_LINK = "-fuse-ld=mold";
          packages = [
           (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })
            gdb
            renderdoc
            cargo-watch
            linuxKernel.packages.linux_6_4.perf
            heaptrack
          ];
          
          nativeBuildInputs = [
          ];
          
          buildInputs = [
            mold

            libxkbcommon
            vulkan-loader

            wayland
            libGL

            pkg-config
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            xorg.libxcb
            xorg.libX11
          ];

          LD_LIBRARY_PATH=pkgs.lib.makeLibraryPath (buildInputs);
        };

      formatter.x86_64-linux = legacyPackages.${system}.nixpkgs-fmt;
    };
}

