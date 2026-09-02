#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 - <<'PY'
import pathlib, re, subprocess
text = pathlib.Path('rust-toolchain.toml').read_text()
channel = re.findall(r'(?m)^\s*channel\s*=\s*"([A-Za-z0-9._-]+)"\s*$', text)
profile = re.findall(r'(?m)^\s*profile\s*=\s*"([A-Za-z0-9._-]+)"\s*$', text)
components = re.findall(r'(?m)^\s*components\s*=\s*\[([^\]]*)\]\s*$', text)
if len(channel) != 1 or len(profile) != 1 or len(components) != 1:
    raise SystemExit('Unsupported root toolchain shape; refusing another version authority')
names = re.findall(r'"([A-Za-z0-9._-]+)"', components[0])
if re.sub(r'"[A-Za-z0-9._-]+"', '', components[0]).replace(',', '').strip() or not names:
    raise SystemExit('Invalid root toolchain component list')
subprocess.run(['rustup','toolchain','install',channel[0],'--profile',profile[0],'--component',','.join(names)], check=True)
version = subprocess.check_output(['rustup','run',channel[0],'rustc','-vV'], text=True)
if 'release: ' + channel[0] + '\n' not in version:
    raise SystemExit('Installed compiler differs from root toolchain')
print(version, end='')
PY
