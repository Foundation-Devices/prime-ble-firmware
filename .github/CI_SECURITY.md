# CI Security

The Rust workflow checked into `master` runs the same jobs for pull requests and for `push` builds on `master`:

- `build` builds and tests host-protocol.
- `Firmware images` builds the unsigned production images and the debug images in the Nix build shell (`nix develop .#build`),
  and uploads them as workflow artifacts with their SHA256 checksums.

Neither job loads deploy keys or repository secrets, including for same-repository branch pull requests when the workflow is
unchanged: all Cargo and Nix dependencies are fetched over public HTTPS. Firmware signing needs the signing keys, so it never
runs in CI, and CI never produces a signed image.

Same-repository branch authors who can change workflow files should be treated as trusted CI authors by GitHub. Review workflow
changes carefully and restrict branch or workflow-edit permissions for contributors who should not be able to alter CI behavior.

External GitHub Actions are pinned to full commit SHAs. Update the comment beside each action ref when intentionally moving to a
newer release.
