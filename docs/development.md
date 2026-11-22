# Development

## Tests

```sh
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline
python3 scripts/smoke-test.py
python3 scripts/auth-smoke-test.py
python3 scripts/theme-smoke-test.py
python3 scripts/security-smoke-test.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The authentication smoke test checks whitespace rejection, verified password changes,
cancellation, concurrent sessions, first-time setup, password generation
and regeneration, weak passwords, confirmation mismatch, hidden input, persisted hashes, incorrect passwords, startup
and session unlocking in both interfaces, and denial of corrupt credential records.

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
and manual/idle protection in both interfaces, including remembered unlocks,
session expiry, and disabling session reuse. The store smoke test also covers
adding recovery codes, status changes, edit preservation and stale-save rejection.
These tests never install system packages and do not validate a real
authenticator service or desktop clipboard.

## GitHub automation and releases

The workflows in `.github/workflows/` provide:

- **CI:** formatting, Clippy, Rust and Python tests, and all four terminal smoke
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
draft. Releases contain the binary, the README, the MIT license, sample themes, word-list
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

## Recording the README demo

The GIF in the README is recorded with [VHS](https://github.com/charmbracelet/vhs)
against a throwaway GPG key and a fake store, never a real one:

```sh
demo/setup.sh                                   # sandbox in /tmp/passtui-demo
source demo/env.sh && ./target/release/passtui  # set master password once: demo-master-pass-2026
vhs demo/demo.tape                              # writes demo/passtui.gif
rm -rf /tmp/passtui-demo
```

---

[← Back to README](../README.md)
