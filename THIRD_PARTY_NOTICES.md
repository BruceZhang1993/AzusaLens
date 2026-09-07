# Third-Party Notices

Azusa Lens is licensed under the Apache License 2.0. Third-party components remain subject to their own license terms.

## Slint 1.17.1

Azusa Lens uses Slint for its desktop user interface under the **Slint Royalty-Free Desktop, Mobile, and Web Applications License 2.0**.

Slint is offered under multiple licenses. Azusa Lens selects the royalty-free license rather than GPL-3.0 for distributed desktop builds. The required Slint attribution badge is displayed on the public project page/README as permitted by section 2(b) of that license.

- Project: https://slint.dev/
- Source: https://github.com/slint-ui/slint
- License: https://slint.dev/agreements/slint-royalty-free-license.pdf

## Lucide Icons

Azusa Lens includes icons derived from Lucide. Lucide is licensed under the ISC License, with portions derived from Feather under the MIT License.

The complete notice bundled with the icons is available at [`apps/desktop/ui/icons/LICENSE`](apps/desktop/ui/icons/LICENSE).

## Rust dependencies

Rust dependency versions are recorded in `Cargo.lock`. Their declared license expressions are checked in CI with `cargo-deny` against `deny.toml`. The allow-list intentionally excludes GPL, AGPL, LGPL, SSPL, and other licenses that require separate review before they can enter the dependency graph.

When distributing binaries, preserve all applicable third-party copyright and license notices required by their respective licenses.
