# Source this to point gpg, pass and PassTUI at the throwaway demo sandbox.
# Nothing here touches your real keys, store or PassTUI config.
export DEMO_DIR=/tmp/passtui-demo
export GNUPGHOME="$DEMO_DIR/gnupg"
export PASSWORD_STORE_DIR="$DEMO_DIR/store"
export XDG_CONFIG_HOME="$DEMO_DIR/config"
export XDG_DATA_HOME="$DEMO_DIR/data"
export PS1='$ '
