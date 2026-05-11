{
  description = "Volsync WebUI - Rust backend + Yew frontend";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs = { self, nixpkgs }:
    let
      forAllSystems = f: nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux"] (system: f system);
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              rustc
              cargo
              trunk
              wasm-bindgen-cli
              pkg-config
              openssl
              nodejs
              llvmPackages.lld
              kubernetes-helm
            ];
          };
        }
      );
    };
}
