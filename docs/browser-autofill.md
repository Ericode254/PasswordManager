# Browser autofill

PassTUI manages your encrypted `pass` store; **Browserpass** reads the same store
and fills usernames and passwords in your browser. PassTUI does not need to be
running. There is no export or second vault to synchronize: browser integration
uses your existing entries, rather than running the terminal interface in a tab.

## Supported browsers

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

## One-time setup

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

## Other Chromium browsers and custom installations

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

## Create a new browser login

For example, to add your personal GitHub login:

1. Start PassTUI with `passtui` (or your installed `passtui` binary).
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

## Rename or move an existing entry

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

## Check or troubleshoot browser setup

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

---

[← Back to README](../README.md)
