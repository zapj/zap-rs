---
title: "ci: add release workflow (cross build for Linux + FreeBSD) and packaging script"
labels:
  - ci
  - release
assignees: []
reviewers: []
---

This PR adds automated release tooling and documentation to the repository:

- .github/workflows/release.yml
  - Builds Linux targets (x86_64 and aarch64) using cross on Ubuntu runners (Docker required).
  - Builds FreeBSD targets (x86_64 and aarch64) on self-hosted FreeBSD runners (label: freebsd).
  - Packages artifacts (tar.gz), generates sha256 checksums, optionally signs checksum files with a GPG private key from repository secrets, and creates a GitHub Release with assets.

- scripts/release/package.sh
  - Helper script to assemble build artifacts and produce tar.gz + sha256 files.

- docs/RELEASE.md
  - Documentation for configuring FreeBSD self-hosted runner, required repository secrets, triggering releases, and verification steps.

Notes and follow-up tasks:
- You must add the following repository secrets to enable signing (optional):
  - GPG_PRIVATE_KEY (ASCII-armored private key)
  - GPG_PASSPHRASE
- Ensure at least one FreeBSD self-hosted runner is registered with the label `freebsd`.
- cross requires Docker on the ubuntu runner; this workflow installs and uses cross accordingly.

