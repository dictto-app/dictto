# Contributing to Dictto

Thanks for your interest in contributing!

## Prerequisites

- [Node.js](https://nodejs.org/) (LTS version)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://rustup.rs/) (stable toolchain)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/) (`cargo install tauri-cli`)

## Setup

```bash
git clone https://github.com/dictto-app/dictto.git
cd dictto
pnpm install
pnpm dev
```

### Note for MSYS2/Git Bash users on Windows

If you get linker errors, you may need to create `apps/desktop/src-tauri/.cargo/config.toml` with your local MSVC linker path. This file is gitignored because it's machine-specific. See the [Tauri prerequisites docs](https://v2.tauri.app/start/prerequisites/) for details.

## Making changes

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Test locally with `pnpm dev`
4. Verify the build works: `pnpm build`
5. Open a pull request

## Pull request guidelines

- Keep PRs focused — one feature or fix per PR
- Describe what changed and why
- Include screenshots for UI changes
- Test on Windows (the primary platform)

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0 License](LICENSE).
