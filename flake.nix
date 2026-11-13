{
  description = "A GUI terminal app built on the felis terminal daemon";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";
    rust-overlay = {
      url = "git+https://github.com/oxalica/rust-overlay?shallow=1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    nix-github-release = {
      url = "github:lambdalisue/nix-github-release/v0.1.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
      nix-github-release,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
      # What felis's font shaping links against on Linux.
      linkedFor = pkgs: pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.fontconfig ];
      # What winit and wgpu open at run time on Linux. They are absent from
      # the binary's needed libraries, so an rpath does not find them.
      openedFor =
        pkgs:
        pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.vulkan-loader
          pkgs.libGL
          pkgs.libxkbcommon
          pkgs.wayland
          pkgs.libx11
          pkgs.libxcursor
          pkgs.libxi
          pkgs.libxrandr
          pkgs.libxcb
        ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          inherit (pkgs) lib;
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain (_: rust);
          # The tag this flake was fetched at, from VERSION; see
          # github:lambdalisue/nix-github-release.
          version = nix-github-release.lib.version { inherit self; };
          build = {
            pname = "cattery";
            # Cargo.toml's placeholder, so the dependency layer stays cached
            # across releases; the package itself is stamped with the version.
            version = "0.0.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
              ];
            };
            strictDeps = true;
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.makeWrapper
            ];
            buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ] ++ linkedFor pkgs;
            # The tests run in CI and with `just test`.
            doCheck = false;
          };
          cattery = craneLib.buildPackage (
            nix-github-release.lib.stampCargo { inherit pkgs version; } (
              build
              // {
                cargoArtifacts = craneLib.buildDepsOnly build;
                postFixup = lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
                  wrapProgram "$out/bin/cattery" \
                    --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath (openedFor pkgs)}"
                '';
              }
            )
          );
        in
        {
          inherit cattery;
          default = cattery;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          default = pkgs.mkShell {
            # Say why each package beyond these is here.
            packages = [
              rust
              pkgs.just
              # Finds the libraries felis's font shaping links against.
              pkgs.pkg-config
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ]
            ++ linkedFor pkgs;
            # So `cargo run` opens a window, as the package's wrapper does.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (openedFor pkgs);
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
