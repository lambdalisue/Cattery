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
                ./assets
                ./LICENSE
                ./crates
                ./scripts/icns.pl
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
                # The licences of Cattery and of the themes it ships go with
                # it, as the themes are built into it.
                postInstall = ''
                  licenses="$out/share/licenses/cattery"
                  install -Dm644 LICENSE "$licenses/LICENSE"
                  install -Dm644 crates/cattery/themes/README.md "$licenses/themes/README.md"
                  install -Dm644 -t "$licenses/themes/LICENSES" crates/cattery/themes/LICENSES/*
                ''
                # On macOS, Cattery is an app: the Dock, Finder and Launchpad
                # show it by its bundle's name and icon, and its windows'
                # menu bar is named after it. The bundle's icon is the light
                # drawing; the Dock shows the one for the theme once Cattery runs.
                + lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
                  app="$out/Applications/Cattery.app/Contents"
                  install -Dm755 "$out/bin/cattery" "$app/MacOS/cattery"
                  substitute assets/macos/Info.plist "$app/Info.plist" --subst-var-by VERSION "${version}"
                  icons=$(mktemp -d)
                  for size in 128 256 512 1024; do
                    resvg -w "$size" -h "$size" assets/logo/cattery-icon-light.svg "$icons/$size.png"
                  done
                  mkdir -p "$app/Resources"
                  perl scripts/icns.pl "$app/Resources/cattery.icns" \
                    ic07="$icons/128.png" ic08="$icons/256.png" ic09="$icons/512.png" ic10="$icons/1024.png"
                  # The executable on PATH is the app's, so a daemon it
                  # starts is the same build as the app's windows.
                  ln -sf "$app/MacOS/cattery" "$out/bin/cattery"
                '';
                nativeBuildInputs =
                  build.nativeBuildInputs ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.resvg ];
                postFixup = lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
                  wrapProgram "$out/bin/cattery" \
                    --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath (openedFor pkgs)}" \
                    --prefix PATH : "${lib.makeBinPath [ pkgs.ncurses ]}"
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
              # Renders the icon's PNGs from its SVG (scripts/icon.sh).
              pkgs.resvg
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ]
            ++ linkedFor pkgs
            # tic compiles the terminfo entry sessions use, which macOS has.
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.ncurses ];
            # So `cargo run` opens a window, as the package's wrapper does.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (openedFor pkgs);
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
