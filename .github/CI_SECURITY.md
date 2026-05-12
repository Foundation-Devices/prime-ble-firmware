# CI Security

The Rust workflow checked into `master` has two trust zones:

- Pull requests run only the host-protocol build and compatibility tests. These jobs do not load deploy keys or repository
  secrets, including same-repository branch pull requests when the workflow is unchanged.
- Trusted `push` builds on `master` use public HTTPS Cargo dependencies and do not load deploy keys or repository secrets.

Same-repository branch authors who can change workflow files should be treated as trusted CI authors by GitHub. Review workflow
changes carefully and restrict branch or workflow-edit permissions for contributors who should not be able to alter CI behavior.

External GitHub Actions are pinned to full commit SHAs. Update the comment beside each action ref when intentionally moving to a
newer release.
