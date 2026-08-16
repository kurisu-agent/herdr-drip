# Fork point

This directory is a fork of **herdr-beads** — <https://github.com/miiraheart/herdr-beads>,
MIT, `LICENSE` here is theirs and travels with the source.

    upstream rev  ccf8381  ("feat: bead edit, status picker, focus; fix ci")
    dated         2026-07-24
    imported      2026-08-16

`src/`, `tests/`, `Cargo.toml` and `LICENSE` were copied **verbatim** in the
import commit, deliberately including the parts we already knew we would
change. That commit is the baseline: `git diff` from it against a fresh
upstream clone is the whole of our divergence, which is what makes it cheap to
take a future upstream release.

What upstream did NOT ship and we added: `Cargo.lock` (upstream `.gitignore`s
it; `rustPlatform.buildRustPackage` requires one) and this file.

Upstream's own `herdr-plugin.toml` and `scripts/` were not imported — this is
one surface of the drip's `beads` plugin, so the manifest and the launchers
live one directory up in `beads/`.
