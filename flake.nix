{
  description = "Satori - Wayland screen translator for manga and visual novels";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "satori";
          version = "2.0.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook4
          ];

          buildInputs = with pkgs; [
            gtk4
            libadwaita
            gtk4-layer-shell
            openssl
          ];

          postFixup = ''
            wrapProgram $out/bin/satori \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.slurp pkgs.grim ]}
          '';

          meta = with pkgs.lib; {
            description = "Wayland screen translator for manga and visual novels";
            homepage = "https://github.com/una/satori";
            license = licenses.mit;
            platforms = platforms.linux;
            mainProgram = "satori";
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          packages = with pkgs; [
            rust-analyzer
            clippy
            rustfmt
            slurp
            grim
          ];
        };
      });
}
