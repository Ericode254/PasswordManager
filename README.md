# PassTUI

A keyboard-driven terminal interface for an existing `pass` password store.

## Build and run

Requires Rust, `pass`, GnuPG, and a working desktop clipboard (X11 or Wayland).
Git is needed for synchronization; GitHub CLI is optional.

```sh
cargo build --release
./target/release/passtui
./target/release/passtui --pick
```

PassTUI honors `PASSWORD_STORE_DIR`. On first use, press `i` to select or create
a GPG key and initialize a store, then `a` to add an entry. New-key creation uses
GnuPG’s pinentry for a passphrase; a desktop pinentry must be available. See the
[GnuPG key-generation documentation](https://gnupg.org/documentation/manuals/gnupg/OpenPGP-Key-Management.html). Back up your GPG
private key separately from the encrypted store.

## Browser autofill (Firefox, Chrome, and Chromium browsers)

PassTUI manages your encrypted `pass` store; **Browserpass** reads the same store
and fills usernames and passwords in your browser. PassTUI does not need to be
running. There is no export or second vault to synchronize: browser integration
uses your existing entries, rather than running the terminal interface in a tab.

### Supported browsers

The setup helper supports **native Linux installations** with these presets:

| Browser | `--browser` value | Extension source |
| --- | --- | --- |
| Firefox | `firefox` | Firefox Add-ons |
| LibreWolf | `librewolf` | Firefox Add-ons |
| Google Chrome | `chrome` | Chrome Web Store |
| Chromium | `chromium` | Chrome Web Store |
| Brave | `brave` | Chrome Web Store |
| Microsoft Edge | `edge` | Chrome Web Store |
| Vivaldi | `vivaldi` | Chrome Web Store |

Other Chromium-based browsers can use the Chromium preset with a custom native
messaging directory (see below). They must support both the Browserpass extension
and Chrome-compatible native messaging. This does not guarantee every fork or
sandboxed browser package supports it.

### One-time setup

1. Install **Browserpass native**, GnuPG, and a graphical pinentry using your
   distribution's packages or the [upstream installation guide](https://github.com/browserpass/browserpass-native#installation).
   Python 3.10 or newer is needed for this project's setup helper.
   On Omarchy, run `omarchy pkg add browserpass`; on Arch Linux, run
   `sudo pacman -S browserpass`.

2. From this project directory, run the command for **your browser**. For example:

   ```sh
   # Firefox
   python3 scripts/setup-browser.py --browser firefox --configure

   # Google Chrome
   python3 scripts/setup-browser.py --browser chrome --configure

   # Brave (use chromium, edge, vivaldi, or librewolf for the other presets)
   python3 scripts/setup-browser.py --browser brave --configure
   ```

   Run it **as your normal user**, not with `sudo`. The helper selects the
   Firefox or Chromium host manifest, checks its extension permissions and
   executable, verifies an echo handshake, and links it into the browser's
   per-user native messaging directory. It does not read or decrypt entries.
   Rerunning it is safe; a different existing registration is left untouched.
   Run it separately for each browser you use. List the presets and resolved
   directories with `python3 scripts/setup-browser.py --list-browsers`.

3. Install the official extension **in that browser**, then enable/pin it:

   - Firefox / LibreWolf: [Browserpass on Firefox Add-ons](https://addons.mozilla.org/firefox/addon/browserpass-ce/).
   - Chrome / Chromium / Brave / Edge / Vivaldi: [Browserpass on the Chrome Web Store](https://chromewebstore.google.com/detail/browserpass/naepdomgkenhinolocfifgehidddafch).

   Approve the browser's extension-installation prompt. A successful native
   helper check does not install or enable the browser extension.

4. In Browserpass Options, leave **Automatically submit forms after filling**
   disabled if you want to review the fields before signing in. The standard
   store is `~/.password-store`. If you use `PASSWORD_STORE_DIR`, add its absolute
   path under **Custom store locations**: a browser launched from the desktop
   may not inherit your terminal's environment. Each browser has its own
   extension settings, but they can all use the same store.

5. Open a website's login page. Press **Ctrl+Shift+L** to choose a saved login,
   or **Ctrl+Shift+F** to fill the best match. Your GPG key may need unlocking
   through a graphical pinentry dialog. If a shortcut conflicts, change it in
   Firefox's **Add-ons → gear menu → Manage Extension Shortcuts**, or in the
   Chromium browser's extension shortcuts page (`chrome://extensions/shortcuts`,
   `brave://extensions/shortcuts`, or the equivalent for your browser).

The manifest locations follow [Browserpass's upstream browser configuration](https://github.com/browserpass/browserpass-native#configure-browsers).
See [Browserpass usage](https://github.com/browserpass/browserpass-extension#usage)
for matching and shortcut details.

### Other Chromium browsers and custom installations

For a custom Chromium user-data root, use `--user-data-dir`. Supply the root,
not an individual profile such as `Default` or `Profile 1`:

```sh
python3 scripts/setup-browser.py --browser chromium --configure \
  --user-data-dir /absolute/path/to/browser-user-data
```

If a browser uses a different native messaging lookup location, specify that
**complete directory** instead, using the browser vendor's documented path:

```sh
python3 scripts/setup-browser.py --browser chromium --configure \
  --native-host-dir /absolute/path/to/NativeMessagingHosts
```

Use `--browser firefox --native-host-dir /absolute/path/to/native-messaging-hosts`
for another compatible Firefox fork. This selects `allowed_extensions`, while
the Chromium family uses `allowed_origins`; the two manifests are not interchangeable.

The helper looks for packaged manifests in `/usr/lib/browserpass/hosts/`,
`/usr/lib64/browserpass/hosts/`, and `/usr/local/lib/browserpass/hosts/`.
For a different installation prefix, supply the installed, configured manifest:

```sh
python3 scripts/setup-browser.py --browser firefox --configure \
  --host-manifest /absolute/path/to/hosts/firefox/com.github.browserpass.native.json
```

For Chromium browsers, use the corresponding `hosts/chromium/` manifest.
The old `scripts/setup-brave.py --configure` / `--check` commands and their
`--brave-dir` option remain available for existing users.

Flatpak and Snap builds may require a native-messaging portal or host integration
provided by the distribution; changing the directory alone may not work. This
helper does not configure that sandbox integration. For macOS or Windows, use
[Browserpass's platform-specific installer](https://github.com/browserpass/browserpass-native#installation);
this project's registration helper is Linux-only.

### Create a new browser login

For example, to add your personal GitHub login:

1. Start PassTUI with `cargo run --offline` (or your installed `passtui` binary).
2. Press **a** to open **New Entry**.
3. Enter **`github.com/personal`** in **Entry path**. Do not type `.gpg`;
   encryption and the extension are handled automatically.
4. Press **Tab** to reach **Password**. Type your existing website password,
   or press **Ctrl+g**, choose a generated password/passphrase, and press
   **Enter** to use it. Generating a password does **not** change the password
   on the website; use a new one when registering or changing that account.
5. Press **Tab** to reach **Username**, and enter the username or email you
   actually use to sign in.
6. Press **Tab** to reach **URL**, optionally entering `https://github.com/login`.
   Press **Tab** again for optional notes. **Alt+Enter** inserts a note line;
   **Shift+Tab** moves back to the previous field.
7. Press **Enter** from a field other than Entry path to save. **Esc** cancels.
   On a validation or save error, correct the field and retry; the draft remains.
8. Visit the login page in your browser and select this entry in Browserpass.

**The full domain must appear in the entry path for automatic website matching.**
Use `github.com/personal`, `github.com/work`, or `Work/github.com` rather than
`github` or `Work/github`. The URL field alone does not establish the match.
You can still use non-domain names for secrets that are not browser logins.

### Rename or move an existing entry

For example, to change `Work/github` into `github.com/work`:

1. Press **/**, type `github`, then press **Enter** to finish searching.
2. Use **Up/Down** to select the entry (not its folder).
3. Press **r** to open **Rename / Move**.
4. Press **Ctrl+u** to clear the old destination, then type **`github.com/work`**.
   Backspace also removes characters if you only need a small change.
5. Press **Enter** to move it, or **Esc** to cancel. The credentials are retained;
   an existing destination is rejected instead of overwritten.
6. If the entry has no username yet, press **e**, use **Tab** to reach Username,
   enter it, then press **Enter** to save. Use **e** for credential edits and
   **r** for path changes.
7. Reopen Browserpass on GitHub. The renamed entry should now match the domain.

Naming examples:

| Entry path | Use |
| --- | --- |
| `github.com/personal` | Personal GitHub account |
| `github.com/work` | Work GitHub account |
| `accounts.google.com/personal` | Google account |
| `Work/example.com/admin` | An admin login organized under Work |

### Check or troubleshoot browser setup

Choose the same browser and any directory override used during configuration:

```sh
python3 scripts/setup-browser.py --browser firefox --check
python3 scripts/setup-browser.py --browser chrome --check
python3 scripts/setup-browser.py --browser brave --check
```

This checks the **registered** manifest and executable and performs an echo
handshake. It does not prove that the extension is installed/enabled, that GPG
can unlock your key, or that a particular website's form works. Verify those in
the browser after installing the extension.

- **Native messaging host not found:** rerun `--configure` for the correct browser
  and user-data location. Close and reopen the extension popup; restart the
  browser if it still has a stale connection.
- **Host manifest missing:** install Browserpass native, or use `--host-manifest`
  with its actual installed manifest. Choose the correct browser family.
- **No matching entries:** check the full domain in the entry path and the store
  path in Browserpass Options. Adding only a URL is insufficient.
- **Entries appear but will not decrypt:** make sure a graphical pinentry is
  configured for GPG; see [Browserpass's GPG troubleshooting](https://github.com/browserpass/browserpass-native#error-unable-to-fetch-and-parse-login-fields).
- **Existing registration differs:** inspect the reported file before changing
  it. The setup helper preserves custom configurations instead of overwriting them.

## Everyday controls

| Key | Action |
| --- | --- |
| `/` | Fuzzy search across full entry paths, including collapsed folders |
| Up / Down | Navigate, including while searching |
| `j` / `k` | Navigate outside text inputs |
| Enter | Finish search, open entry, or submit a form |
| `y` | Copy selected password |
| `p` | Reveal password for 15 seconds, or hide it immediately |
| Ctrl+L | Clear the session and show the privacy lock screen, including inside forms |
| `t` | Show a TOTP authentication code; `y` copies, `r` refreshes |
| `R` | Manage the selected website's recovery codes |
| Esc | Close details, clear a filter, or cancel a dialog |
| `a` / `e` | Add / edit an entry |
| `r` | Rename or move the selected entry |
| `f` / `F` | Toggle favorite / show favorites only |
| `v` / `H` | Selected entry's history / whole-store history, including deleted entries |
| `u` / `w` | Copy username / URL from the open entry |
| PageUp / PageDown | Scroll long entry details |
| Ctrl+g | Open generator from an add-entry field |
| `g` | Open generator outside a text field |
| `d` | Request deletion; only `y` confirms, Enter cancels |
| `G` | Git menu; Esc returns to browsing during sync |
| `?` | Help |

In `--pick`, every ordinary letter is searchable, including `j` and `k`.
Use arrow keys to navigate and Enter to copy and exit. Favorites appear first;
Ctrl+f toggles the selected entry's favorite status.

Forms retain their draft after validation or save failures. Password input is
masked. Esc in the generator restores the previous draft; Enter accepts the
new password. Existing entry names are rejected when adding. Use `e` to edit instead. The
editor checks for external changes before saving. Use Tab / Shift+Tab to move
between password, username, URL, and notes; Alt+Enter inserts a newline in notes.
Existing unknown fields and notes are retained. Use `r` and enter a new relative
path to rename or move an entry; existing destinations are rejected.

In entry forms, rename, and search, use Left / Right to position the cursor,
Home / End to jump within a line, Delete or Backspace to remove characters,
and Ctrl+u to clear the field. Ctrl+z undoes and Ctrl+y redoes edits within the
current field (up to 100 undo steps). Form undo history is discarded when the
form closes. Passwords remain masked while editing.

Paste using your terminal's shortcut, usually Ctrl+Shift+v. Terminals supporting
bracketed paste insert the entire paste as one undoable edit without treating its
contents as shortcuts. Notes accept multiple lines; other entry fields reject
multiline pastes instead of accidentally saving extra fields or submitting the form.

## Favorites

Select an entry and press **f** to add or remove its star. Press **F** to browse
only favorites, including entries inside collapsed folders; search also works
within that view. Press **F** again or **Esc** from the main screen to show all
entries. The quick picker (`passtui --pick`) shares these favorites and ranks
matching favorites first.

Favorites contain entry paths only, never decrypted credentials. On Linux they
live in `~/.local/share/passtui/favorites.toml`, or
`$XDG_DATA_HOME/passtui/favorites.toml` when set. Each password store has its own
list. Changes are saved automatically, and renaming an entry in PassTUI updates
its favorite path. Deleted entries disappear from the list but retain their
favorite reference, so restoring the same path restores its star. Renames made
outside PassTUI require marking the new path as a favorite manually.

## Entry history and recovery

History requires a Git repository in the password store. If needed, press **G**,
then **i** to initialize one; only versions committed to Git can be recovered.

1. Select an entry and press **v** for its history, or **H** for store history
   including entries that have been deleted or renamed.
2. Select a version with Up / Down and press **Enter** to decrypt its preview.
   The preview shows the path, revision, password length, and metadata counts;
   the password itself stays hidden.
3. Press **r**, then **y** to restore the complete entry at its historical path.
   Any other key cancels confirmation. Esc closes the history view.

Restore re-encrypts the old content for the current recipients through `pass`
and creates a normal new save/commit when content changes. Existing Git history
is retained. A changed store since preview or uncommitted changes to the target
entry block restoration; resolve those changes and preview again before retrying.
History loading, decryption, and restore run in background workers. Decrypted
previews stay in memory and are discarded when you change selection or close
the view.

The view covers the last **100 matching commits** in the current branch's
reachable history. Entry history uses the current path; use **H** to find older
names after a rename. Historical decryption still requires access to the original
GPG key. Versions outside this window can be accessed with Git directly.

Passphrases use the bundled 7,776-word EFF list with a minimum of six words
(approximately 77.5 bits of generation entropy). The generator labels word
count explicitly. See [word-list attribution](assets/README.md).

## Idle protection and session cleanup

PassTUI clears an idle session after **five minutes** by default. Press **Ctrl+L**
to do this immediately, including while editing or waiting for a background action.
The main interface and quick picker both support idle protection.

**Unsaved drafts are discarded when the session clears.** Decrypted details,
history previews, generated passwords, TOTP codes, and editing/undo state are
discarded too. Owned secret buffers use `zeroize` when dropped. This is best-effort
memory cleanup, not a guarantee that terminal buffers, operating-system buffers,
or every temporary copy have been erased.

The privacy screen hides entry names and ignores pasted text. Press **Enter** to
resume browsing or **q** to quit. An already-running operation is allowed to finish;
resuming waits for it, its decrypted result is discarded, and late clipboard copies
are cancelled. A save already in progress can still complete. The refreshed store
shows its outcome after resuming.

This is a **PassTUI privacy lock, not a new authentication boundary**. It does not
lock the desktop, lock Browserpass, or revoke GPG's shared cached credentials.
Resuming does not require a new password; GPG decides whether the next decryption
needs pinentry. See [GPG agent cache settings](https://www.gnupg.org/documentation/manuals/gnupg/Agent-Options.html)
if you also want to control how long GPG remembers an unlock.

In your existing `~/.config/passtui/config.toml`, add this to `[behavior]`:

```toml
[behavior]
idle_lock_seconds = 300              # 0 disables automatic clearing
auto_clear_clipboard_seconds = 45    # 1–86400; also caps TOTP clipboard lifetime
default_reveal_passwords = false
```

Restart PassTUI after changing configuration. Keyboard and paste activity reset
the idle timer; background work does not.

## TOTP authentication codes

PassTUI reads the same `otpauth://totp/...` entries used by
[pass-otp](https://github.com/pass-extension/pass-otp). Code generation requires
[`oathtool` from OATH Toolkit](https://www.nongnu.org/oath-toolkit/man-oathtool.html)
on your PATH. If it is missing, pressing **t** opens the OTP setup dialog.
Press **i** to install it automatically with your system package manager, then
continue to the code dialog. The exact command is shown before installation.
PassTUI temporarily leaves the terminal UI so `sudo` can request administrator
authentication; it never reads or stores your administrator password.

Automatic setup supports these native package managers:

| System | Package manager | Package |
| --- | --- | --- |
| Arch Linux / Omarchy and Arch derivatives | pacman | oath-toolkit |
| Debian / Ubuntu and derivatives | apt-get | oathtool |
| Fedora / RHEL family with the package available | dnf | oathtool |
| macOS with Homebrew installed | brew | oath-toolkit |

Package names follow the official [Arch](https://archlinux.org/packages/extra/x86_64/oath-toolkit/),
[Debian](https://packages.debian.org/bookworm/oathtool),
[Fedora](https://packages.fedoraproject.org/pkgs/oath-toolkit/), and
[Homebrew](https://formulae.brew.sh/formula/oath-toolkit) listings.
Setup installs only the requested package and its dependencies. It does not
upgrade the whole system or install a package manager. Unsupported systems get
manual setup instructions. Network, repository, and permission errors remain
visible, and **i** retries installation. Existing installations are left alone.

You can also install OTP support before opening PassTUI:

```sh
./target/release/passtui --install-otp
```

Or verify an existing installation with:

```sh
oathtool --version
```

To add a token to an existing login, open **e**, move to **Notes**, and paste the
site's setup URI on its own line. Keep the existing password and other fields.
For example, using an illustrative test secret:

```text
otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example
```

Alternatively, if pass-otp is installed, run `pass otp append github.com/work`
and enter the setup URI at its prompt. Existing OTP-only entries created with
`pass otp insert` also work. Use one URI per entry; avoid passing real setup
secrets as shell arguments, where they could enter shell history.

Select a password entry (not its folder) in the full PassTUI interface and press **t**.
If you searched with `/`, press **Enter** to finish searching first. In `--pick`,
`t` remains a search character; open the full interface to view TOTP codes. The dialog shows the current code and
its remaining lifetime. Press **y** to copy it, **r** to generate a fresh code, or
**Esc** to close. Expired codes disappear and cannot be copied; codes with less
than a second remaining require a refresh. Codes do not refresh automatically.
The clipboard expires at the earlier of code expiry and your configured timeout.
Locking the session requests early clipboard cleanup.

SHA1, SHA256, and SHA512, 6 or 8 digits, and custom periods of 1–86400 seconds are
supported. The default is SHA1, six digits, and 30 seconds. A correct system clock
is required. HOTP and multiple tokens per entry are rejected. Generation runs in
a background worker; the setup secret is passed to oathtool through stdin and
is never included in command arguments or diagnostics. Normal details hide OTP
URIs, and password-copy actions reject OTP-only entries so they cannot accidentally
copy the setup secret. Explicit entry editing still exposes the URI in Notes.

## Website recovery codes

Store the backup codes supplied by a website alongside its login:

1. Select the login and press **e** (or **a** for a new entry).
2. Tab to **Recovery codes**, after Notes.
3. Paste one code per line. Alt+Enter inserts another line; the field is masked.
4. Press Enter to save the entry.

Press **R** on the selected entry to manage its codes:

| Key | Action |
| --- | --- |
| Up / Down | Select a code |
| `p` | Reveal the selected code for 15 seconds, or hide it |
| `y` | Copy an available code using the usual clipboard timeout |
| `x`, then `y` | Mark the code used, or available again if marked by mistake |
| Esc | Close the recovery-code list |

Copying does **not** mark a code used. Mark it used after the website accepts it.
Used codes cannot be copied until marked available again. Enter cancels a status
change confirmation. Status updates preserve the password, OTP setup, and other
fields, and reject saves if the entry changed while the recovery list was open.

Recovery codes and their used status live **inside the encrypted pass entry**,
using one `recovery-code: CODE` or `recovery-code-used: CODE` line per code. They
are masked in the editor and recovery list; ordinary details show only counts.
Session locking discards the decrypted recovery view. Used codes remain stored
and survive ordinary edits. To remove one, mark it available, then delete its
line from the editor's Recovery codes field and save.

The website controls whether a code is valid. Changing its local status or
restoring an old Git revision does not reactivate a code the website has consumed.

## Clipboard and feedback

A separate helper process owns the clipboard and clears it after the configured
timeout, including after the picker exits. Secrets are passed through a pipe,
not command arguments or temporary files. Cleanup checks the current clipboard
before clearing so different content copied elsewhere is preserved. Copying
again in the same running app replaces the previous timer. Independent app
instances copying the identical secret can still share the earlier expiration.
Clipboard-history tools may retain their own copies.
Clearing the session asks its active helper to clear the clipboard immediately
if the copied value is still present. Normal application exit preserves the
original expiration timer.

Success messages appear in the footer. Errors require dismissal. Decrypting,
copying, saving, deleting, key creation, initialization, and network sync run in
background workers. Mutating operations cannot be cancelled partway through;
progress remains visible. Below 80 terminal columns, list and details share a
single panel; Esc returns to the list.

## Themes and configuration

Configuration lives at `~/.config/passtui/config.toml` (or
`$XDG_CONFIG_HOME/passtui/config.toml` on Linux when set).

Choose a built-in theme or create your own. For example, save this as
`~/.config/passtui/themes/my-theme.toml`:

```toml
base = "catppuccin-mocha"

[colors]
primary = "#7DD3FC"
background = "#101827"
selection_bg = "#7DD3FC"
selection_fg = "#101827"
```

Then select it in the existing configuration's `[theme]` section:

```toml
[theme]
name = "my-theme"
```

Restart PassTUI to apply it. Omitted colors inherit from the base theme.
A complete example, all color options, and explicit file selection are covered
in the [theme guide](themes/README.md). Invalid themes fall back to the default
with a visible warning, without discarding other settings.

Invalid settings and shortcut conflicts also produce a visible warning. TOTP
uses the fixed `t` shortcut; the old unused `copy_totp` configuration field is
still ignored for compatibility.

## Validation

```sh
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline
python3 scripts/smoke-test.py
python3 scripts/theme-smoke-test.py
python3 scripts/security-smoke-test.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The isolated terminal smoke test exercises add, edit, multiline paste, favorites
shared with the picker, rename/move, search, deletion, and history restoration
(including cancellation and stale-preview protection) using real Git and mock
`pass`/`gpg` executables. Unit tests
exercise form input, draft retention, failed saves, search, reveal timeout,
undo/redo, favorites persistence, historical versions, delete cancellation,
narrow-terminal rendering, and word-list integrity without
accessing your live password store. Live desktop clipboard and GPG/pinentry
integration require a manual check in the target desktop session.
The security smoke test uses mock `pass`, `oathtool`, package-manager and `sudo`
programs to verify automatic installation, terminal restoration, stdin handling,
TOTP dialog/countdown, seed masking, HOTP rejection, late-worker cleanup,
and manual/idle protection in both interfaces. The store smoke test also covers
adding recovery codes, status changes, edit preservation and stale-save rejection.
These tests never install system packages and do not validate a real
authenticator service or desktop clipboard.

## GitHub automation and releases

The workflows in `.github/workflows/` provide:

- **CI:** formatting, Clippy, Rust and Python tests, and all three terminal smoke
  tests on pull requests and pushes to `main`. The runner uses stable Rust and
  Python 3.11; Cargo builds and tests use `--locked`.
- **Dependency security:** `cargo audit` checks `Cargo.lock` on pull requests,
  pushes to `main`, and every Monday. Update the pinned cargo-audit version in
  `security.yml` when upgrading the audit tool.
- **CodeQL:** Rust, Python, and GitHub Actions analysis on pull requests, pushes
  to `main`, and every Tuesday. Private repositories require a GitHub plan with
  code scanning enabled. Use this advanced workflow instead of also enabling
  CodeQL's default setup.
- **Release:** pushing a `v*` tag builds and validates a Linux x86_64 release,
  generates checksums and a build provenance attestation, and creates a **draft**
  GitHub release. The tag must equal `v` followed by the version in `Cargo.toml`.

Dependabot checks Cargo dependencies and pinned action revisions weekly and
opens update pull requests. Minor and patch Cargo updates are grouped together;
major updates remain separate. GitHub Actions updates have their own group.

After pushing these files to GitHub, select **Build and test** as a required
check in the repository's branch rules for `main`. Workflow files do not enable
branch protection themselves. CI, dependency security, and CodeQL can also be
started manually from the Actions tab.

To prepare a release, update `Cargo.toml` and `Cargo.lock` together, commit the
version change, and wait for the checks to pass. Then tag that commit and push
the tag (replace `0.1.0` with the package version):

```sh
git tag -a v0.1.0 -m 'PassTUI v0.1.0'
git push origin v0.1.0
```

Review the generated notes and assets in GitHub Releases before publishing the
draft. Releases contain the binary, this README, sample themes, word-list
attribution, and browser setup helpers. The Linux binary is built on Ubuntu
22.04 and requires glibc 2.35 or newer; it is not a static binary. `pass`, GnuPG,
Git, a desktop clipboard, and Browserpass for browser autofill remain external
dependencies. OTP generation additionally requires `oathtool`.

Download the archive and `SHA256SUMS` into the same directory to verify the
checksum. GitHub CLI can also verify the build provenance (replace `OWNER/REPO`
with the repository containing the release):

```sh
sha256sum --check SHA256SUMS
gh attestation verify passtui-v0.1.0-x86_64-unknown-linux-gnu.tar.gz --repo OWNER/REPO
tar -xzf passtui-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd passtui-v0.1.0-x86_64-unknown-linux-gnu
./passtui
```

Release attestations require a public repository or an eligible GitHub Enterprise
Cloud plan for a private repository. No personal access token is required by
these workflows; they use GitHub's scoped workflow token.
