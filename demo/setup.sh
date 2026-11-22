#!/usr/bin/env bash
# Builds a throwaway GPG key and a fake password store for recording the demo.
# Safe to re-run: it wipes and recreates /tmp/passtui-demo only.
set -euo pipefail
cd "$(dirname "$0")/.."
source demo/env.sh

rm -rf "$DEMO_DIR"
mkdir -p "$GNUPGHOME" "$XDG_CONFIG_HOME"
chmod 700 "$GNUPGHOME"

# Demo key with no passphrase, so GPG never pops up a pinentry mid-recording.
gpg --batch --passphrase '' --quick-gen-key "PassTUI Demo <demo@example.com>" default default never 2>/dev/null
KEY=$(gpg --list-keys --with-colons demo@example.com | awk -F: '/^fpr/{print $10; exit}')

pass init "$KEY" >/dev/null
# XDG_CONFIG_HOME hides your real Git identity, so give the store its own.
git -C "$PASSWORD_STORE_DIR" init -q
git -C "$PASSWORD_STORE_DIR" config user.name "PassTUI Demo"
git -C "$PASSWORD_STORE_DIR" config user.email demo@example.com
pass git init >/dev/null

add() { # path, then the entry's lines
  local path=$1; shift
  printf '%s\n' "$@" | pass insert -m -f "$path" >/dev/null
}
add github.com/personal          'plum-orbit-candle-river-73' 'username: demo-dev' 'url: https://github.com/login'
add github.com/work              'quartz-mellow-signal-ferry' 'username: demo-dev-work' 'url: https://github.com/login' \
  'otpauth://totp/GitHub:demo-dev-work?secret=JBSWY3DPEHPK3PXP&issuer=GitHub' \
  'recovery-code: 4f2a-91c3' 'recovery-code: 7be0-15d8' 'recovery-code-used: 0c6e-a2f4'
add accounts.google.com/personal 'lantern-copper-violet-oak'  'username: demo@example.com' 'url: https://accounts.google.com'
add Work/example.com/admin       'harbor-pixel-thunder-sage'  'username: admin' 'url: https://example.com'
add digitalocean.com             'velvet-anchor-cobalt-fern'  'username: demo-dev' 'url: https://cloud.digitalocean.com'
# History to show off: an edited entry and a deleted one.
add netlify.com                  'saffron-glacier-tempo-mint' 'username: demo-dev'
add netlify.com                  'tundra-maple-orbit-quill'   'username: demo-dev' 'url: https://app.netlify.com'
add aws.amazon.com/console       'old-password-from-2025'     'username: demo-dev'
pass rm -f aws.amazon.com/console >/dev/null

cargo build --release -q
echo "Demo sandbox ready in $DEMO_DIR"
echo "Next: source demo/env.sh && ./target/release/passtui"
echo "      and set the master password to: demo-master-pass-2026"
