# Security model

## Master password

On first launch, PassTUI asks you to create and confirm a **master password**.
Use at least **15 characters**, with **no spaces or other whitespace**, and a
strong, unpredictable password; common or repetitive passwords are rejected.
Join six or more randomly chosen words with hyphens instead of spaces.
Input is masked, and you can paste a password. Trailing clipboard line endings
are ignored; embedded line breaks are rejected. Press Esc to exit without opening
the store. Every later launch, including `--pick`, requires this password before
showing entries. Existing users also complete this setup once after upgrading.

To generate a master password during setup, press **Ctrl+G**. PassTUI shows a
random six-word passphrase: record it somewhere safe, then press **Enter** to use
it and retype it in the confirmation field. **Ctrl+G** generates another candidate;
**Esc** returns to your previous draft. The generated phrase is visible only in
the preview; setup fields remain masked. Generation is available during first-time
setup and when choosing a replacement password, not on the unlock screen.

To change your master password, press **Ctrl+K** on the main browsing screen
(close any form or popup first), or run:

```sh
passtui --change-master-password
```

Enter your **current password**, then enter and confirm a strong new password
without whitespace, or use **Ctrl+G** to generate one. Esc cancels without changing
the saved password. The old password is replaced only after the new record has
been written successfully. Other open sessions use the new password the next
time they unlock. Existing passwords containing spaces still work for unlocking
and verifying the current password; the whitespace rule applies to newly saved
passwords. This changes only the PassTUI master password, not your GPG passphrase
or saved website passwords.

Only a randomly salted Argon2id hash is saved, in
`~/.config/passtui/master-password` (under `$XDG_CONFIG_HOME` when set), with
owner-only file permissions on Unix. It is shared across stores opened with that
configuration directory. Incorrect attempts incur a delay. An unreadable or
invalid record prevents access instead of silently starting setup again.

This password controls access through PassTUI. Vault encryption still uses GPG,
which may separately ask for your key passphrase. Browserpass and the `pass` CLI
continue to use GPG independently. Someone who can modify your local configuration
can remove this application gate; it does not replace GPG or your desktop lock.
Keep the master password safe: there is no in-app password recovery.

PassTUI honors `PASSWORD_STORE_DIR`. After master-password setup, press `i` to select or create
a GPG key and initialize a store, then `a` to add an entry. New-key creation uses
GnuPG’s pinentry for a passphrase; a desktop pinentry must be available. See the
[GnuPG key-generation documentation](https://gnupg.org/documentation/manuals/gnupg/OpenPGP-Key-Management.html). Back up your GPG
private key separately from the encrypted store.

## Idle protection and session cleanup

PassTUI clears an idle session after **five minutes** by default. Press **Ctrl+L**
to do this immediately, including while editing or waiting for a background action.
The main interface and quick picker both support idle protection.

**Unsaved drafts are discarded when the session clears.** Decrypted details,
history previews, generated passwords, TOTP codes, and editing/undo state are
discarded too. Owned secret buffers use `zeroize` when dropped. This is best-effort
memory cleanup, not a guarantee that terminal buffers, operating-system buffers,
or every temporary copy have been erased.

After you authenticate, PassTUI remembers the unlock for **two hours in that
running app**. The five-minute idle screen still hides entries and clears
decrypted details. Press **Enter** to resume without retyping your master password
during those two hours, or **q** to quit. Resuming does not extend the two-hour
window. After it expires, the next idle-screen resume asks for your password;
active work is not interrupted just because the window expired.

**Ctrl+L always requires a password to unlock**, even during the two-hour window
or from the idle screen. Changing the master password always verifies the current
one; a successful change starts a new two-hour window. Another app instance's
password change invalidates your remembered unlock. Closing the app also ends
the session: reopening either the main app or quick picker requires authentication.
Only the authentication time is remembered in memory, never the plaintext password
or an unlock token on disk. Set `unlock_session_seconds = 0` to require your
password after every idle screen again.

The idle screen ignores pasted text. An already-running operation is allowed to finish;
resuming waits for it, its decrypted result is discarded, and late clipboard copies
are cancelled. A save already in progress can still complete. The refreshed store
shows its outcome after resuming.

This session policy applies to the **PassTUI master password**, including in the
quick picker. It does not lock the desktop, lock Browserpass, or revoke GPG's shared cached
credentials. GPG decides whether the next decryption also needs pinentry.
See [GPG agent cache settings](https://www.gnupg.org/documentation/manuals/gnupg/Agent-Options.html)
if you also want to control how long GPG remembers an unlock.

In your existing `~/.config/passtui/config.toml`, add this to `[behavior]`:

```toml
[behavior]
idle_lock_seconds = 300              # 0 disables automatic clearing
unlock_session_seconds = 7200        # 0–86400; 0 asks after every idle screen
auto_clear_clipboard_seconds = 45    # 1–86400; also caps TOTP clipboard lifetime
default_reveal_passwords = false
```

Restart PassTUI after changing configuration. Keyboard and paste activity reset
the idle timer; background work does not.

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

---

[← Back to README](../README.md)
