let
    host_pkgs = import <nixpkgs> {};
    nixpkgs = host_pkgs.fetchFromGitHub {
        owner = "NixOS";
        repo = "nixpkgs";
        rev = "6cf604828568f0594db75daae39498f055c1a01f";
        sha256 = "1crmrvlwkvixq3m1xn2xzy5j72gxmgillpzg6i2wkf67mmzdi1r7";
    };
in
with import nixpkgs {};
stdenv.mkDerivation {
  name = "the-renderer";
  version = "0.1.0";
  src = ./.;
  buildInputs = [ cargo rustc librsvg gnome3.gtk ];
}
