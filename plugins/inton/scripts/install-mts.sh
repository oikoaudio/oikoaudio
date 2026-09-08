#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" == Darwin ]]; then
  inton_mts_path='/Library/Application Support/MTS-ESP/libMTS.dylib'
  if [[ -f "$inton_mts_path" ]]; then
    printf 'Keeping the existing %s.\n' "$inton_mts_path"
    exit 0
  fi
  sudo /usr/sbin/installer \
    -pkg 'vendor/mts-esp/libMTS/Mac/x86_64_ARM/libMTSMac_v1.03.pkg' \
    -target /
  printf '%s\n' 'Installed the official ODDsound MTS-ESP library. Restart your DAW.'
  exit 0
fi
if [[ "$(uname -s)" != Linux ]]; then
  printf '%s\n' 'Use scripts/install-mts.ps1 on Windows.' >&2
  exit 1
fi
if [[ -f /usr/local/lib/libMTS.so ]]; then
  printf '%s\n' 'Keeping the existing /usr/local/lib/libMTS.so.'
  exit 0
fi
case "$(uname -m)" in
  x86_64) inton_arch=x86_64;;
  aarch64) inton_arch=arm64;;
  armv7l) inton_arch=arm32hf;;
  *) printf '%s\n' 'No bundled MTS library for this architecture.' >&2; exit 1;;
esac
sudo install -D -m 755 "vendor/mts-esp/libMTS/Linux/$inton_arch/libMTS.so" /usr/local/lib/libMTS.so
printf '%s\n' 'Installed the official ODDsound MTS-ESP library. Restart your DAW.'
