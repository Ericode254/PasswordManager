# PassTUI

A fast, keyboard-driven terminal UI for your existing [`pass`](https://www.passwordstore.org/) password store.

![PassTUI demo](demo/passtui.gif)

PassTUI works on top of the standard `pass` store, so your passwords stay as
GPG-encrypted files you already own. There is no new vault format, no cloud
account and nothing to migrate. The `pass` CLI and Browserpass keep working
alongside it.

## Features

- **Fuzzy search** across every entry, plus a quick picker (`passtui --pick`) that copies a password and exits
- **Master password** stored only as a salted Argon2id hash, with auto-lock after 5 minutes idle
- **Self-clearing clipboard**: copied secrets are cleared after 45 seconds by a separate helper process
- **Password and passphrase generator** using the EFF word list
- **TOTP codes** from `pass-otp` entries, and website **recovery codes** with used/unused tracking
- **History and restore** for any entry, including deleted ones, from the store's Git history
- **Browser autofill** through Browserpass in Firefox, Chrome, Brave, Edge, Vivaldi and more
- **Favorites, themes** and a layout that adapts to narrow terminals

## Requirements

- Linux with an X11 or Wayland clipboard
- [`pass`](https://www.passwordstore.org/) and GnuPG
- Rust (stable) to build from source
- Optional: Git for sync and history, [`oathtool`](https://www.nongnu.org/oath-toolkit/) for TOTP,
  [Browserpass](https://github.com/browserpass/browserpass-extension) for browser autofill

## Install

```sh
cargo install --locked --git https://github.com/Ericode254/PasswordManager passtui
```

Or build from a clone:

```sh
git clone https://github.com/Ericode254/PasswordManager.git
cd PasswordManager
cargo build --release
./target/release/passtui
```

## Quick start

1. Run `passtui`. On first launch, create a **master password** (at least 15
   characters, no spaces), or press **Ctrl+G** to generate a six-word passphrase.
2. If you already use `pass`, your entries appear straight away. PassTUI respects
   `PASSWORD_STORE_DIR`. If you're new to `pass`, press **i** to choose or create a
   GPG key and set up a store.
3. Press **a** to add an entry, **/** to search and **y** to copy a password.
4. Bind `passtui --pick` to a hotkey for a search, copy and exit picker.

The master password controls access through PassTUI; your entries are still
encrypted by GPG. There is no recovery if you forget it. Change it with **Ctrl+K**
or `passtui --change-master-password`.

## Keys

| Key | Action |
| --- | --- |
| `/` | Fuzzy search |
| Up / Down, `j` / `k` | Move |
| Enter | Open entry or finish search |
| `y` | Copy password |
| `u` / `w` | Copy username / URL |
| `p` | Reveal password for 15 seconds |
| `a` / `e` / `r` / `d` | Add / edit / rename / delete |
| `g` | Password generator |
| `t` | TOTP code |
| `R` | Recovery codes |
| `f` / `F` | Toggle favorite / show favorites |
| `v` / `H` | Entry history / whole-store history |
| `G` | Git sync menu |
| Ctrl+L | Lock now |
| `?` | Help with every shortcut |

## Configuration

Settings live in `~/.config/passtui/config.toml` (or under `$XDG_CONFIG_HOME`).
These are the defaults:

```toml
[behavior]
idle_lock_seconds = 300              # 0 disables auto-lock
unlock_session_seconds = 7200        # resume without retyping for 2 hours; 0 always asks
auto_clear_clipboard_seconds = 45
default_reveal_passwords = false

[theme]
name = "catppuccin-mocha"
```

Restart PassTUI after changing it. To make your own theme, see the [theme guide](themes/README.md).

## Documentation

- [Using PassTUI](docs/usage.md): all controls, favorites, history and restore, TOTP, recovery codes, themes
- [Browser autofill](docs/browser-autofill.md): Browserpass setup for each browser, and troubleshooting
- [Security model](docs/security.md): master password, sessions and idle lock, clipboard handling
- [Development](docs/development.md): tests, CI, releases and recording the demo

## Contributing

Issues and pull requests are welcome. Before opening a pull request, run:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

The full test suite, including the terminal smoke tests, is described in
[docs/development.md](docs/development.md). Please report security issues
privately through GitHub's **Report a vulnerability** button on the
Security tab rather than in a public issue.

## Acknowledgements

Built on [`pass`](https://www.passwordstore.org/), [Browserpass](https://github.com/browserpass)
and [Ratatui](https://ratatui.rs/). Passphrases use the
[EFF large word list](assets/README.md).

## License

[MIT](LICENSE) © 2026 Erick Jillo
