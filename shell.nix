let
  pkgs = import (fetchTarball("channel:nixpkgs-unstable")) {};
in pkgs.mkShell {
  buildInputs = with pkgs; [ 
  pkg-config
  linuxPackages_latest.perf 
  fontconfig
  openssl
  vulkan-tools
  vulkan-headers
  vulkan-loader
  vulkan-validation-layers
  ];
}
