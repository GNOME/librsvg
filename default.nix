let
    host_pkgs = import <nixpkgs> {};
    nixpkgs = host_pkgs.fetchFromGitHub {
        owner = "NixOS";
        repo = "nixpkgs-channels";
        rev = "45a85eacebf2181d2e12c0c1005bf3ba07583a74";
        sha256 = "1ksrjl897cq6j2b3h0s671sd626fbbf6sq87yvrl26wvyrrd7dz8";
    };
in
with import nixpkgs {};
stdenv.mkDerivation {
  name = "the-renderer";
  version = "0.1.0";
  src = ./.;
  buildInputs = [ cargo rustc librsvg gnome3.gtk ];
}
