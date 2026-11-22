# Using PassTUI

## Everyday controls

| Key | Action |
| --- | --- |
| `/` | Fuzzy search across full entry paths, including collapsed folders |
| Up / Down | Navigate, including while searching |
| `j` / `k` | Navigate outside text inputs |
| Enter | Finish search, open entry, or submit a form |
| `y` | Copy selected password |
| `p` | Reveal password for 15 seconds, or hide it immediately |
| Ctrl+L | Clear the session and lock with the master password, including inside forms |
| Ctrl+K | Change the master password from the main browsing screen |
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
count explicitly. See [word-list attribution](../assets/README.md).

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
passtui --install-otp
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
in the [theme guide](../themes/README.md). Invalid themes fall back to the default
with a visible warning, without discarding other settings.

Invalid settings and shortcut conflicts also produce a visible warning. TOTP
uses the fixed `t` shortcut; the old unused `copy_totp` configuration field is
still ignored for compatibility.

---

[← Back to README](../README.md)
