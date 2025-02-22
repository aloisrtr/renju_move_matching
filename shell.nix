with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "dev-renju-mm";
  buildInputs = with pkgs; [ 
    pkg-config
    linuxPackages_latest.perf 
    fontconfig
    freetype
    openssl
  ];
}
