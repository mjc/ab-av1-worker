{
  pkgs,
  lib,
  ...
}: let
  duplicate-loongarch-patch = "0001-swscale-loongarch-fix-buffer-underflow-in-yuv2plane1";
  svt-av1-hdr = pkgs.svt-av1.overrideAttrs (_old: {
    pname = "svt-av1-hdr";
    version = "4.2.0";
    src = pkgs.fetchFromGitHub {
      owner = "juliobbv-p";
      repo = "svt-av1-hdr";
      rev = "v4.2.0";
      hash = "sha256-axJ4C2gSQMdiGo6qLNxaQ5AWUuZp6gRnEI8Bx0B7tlw=";
    };
  });
  ffmpeg-svt-hdr =
    (pkgs.ffmpeg-full.override {
      version = "9.0.2";
      source = pkgs.fetchurl {
        url = "https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz";
        hash = "sha256-jDhQKD6yX6AmSCB4oEBR4L4XNHsJ74GghJvsFaluAC4=";
      };
      svt-av1 = svt-av1-hdr;
    }).overrideAttrs (old: {
      # FFmpeg 9.0.2 already contains this fix; nixpkgs' backport now reverses.
      patches =
        lib.filter
        (patch:
          !(lib.hasInfix
            duplicate-loongarch-patch
            (builtins.baseNameOf (toString patch))))
        old.patches;
      postPatch =
        (old.postPatch or "")
        + ''
          substituteInPlace libavcodec/libsvtav1.c \
            --replace-fail "param->enable_adaptive_quantization = 0;" ""
        '';
    });
in {
  languages.rust = {
    enable = true;
    toolchainFile = ./rust-toolchain.toml;
  };

  packages = with pkgs;
    [
      git
      pkg-config
      openssl
      sccache
      cargo-nextest
      ffmpeg-svt-hdr
      svt-av1-hdr
      rav1e
      cargo-audit
      cargo-deny
      cargo-flamegraph
      hyperfine
    ]
    ++ lib.optionals pkgs.stdenv.isLinux [pkgs.clang pkgs.wild];

  env = {
    OPENSSL_DIR = "${pkgs.openssl.dev}";
    OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
    PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = lib.optionalString pkgs.stdenv.isLinux "-C link-arg=-fuse-ld=${pkgs.wild}/bin/wild";
  };
}
