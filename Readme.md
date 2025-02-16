# Kiomet.com

[![Build](https://github.com/SoftbearStudios/kiomet/actions/workflows/build.yml/badge.svg)](https://github.com/SoftbearStudios/kiomet/actions/workflows/build.yml)
<a href='https://discord.gg/YMheuFQWTX'>
  <img src='https://img.shields.io/badge/Kiomet.com-%23announcements-blue.svg' alt='Kiomet.com Discord' />
</a>

![Logo](/assets/branding/512x340.jpg)

[Kiomet.com](https://kiomet.com) is an online multiplayer real-time strategy game. Command your forces wisely and prepare for intense battles!

## Build Instructions

1. Install `rustup` ([see instructions here](https://rustup.rs/))
2. Install `gmake` and `gcc` if they are not already installed.
3. Install `trunk` (`cargo install --locked trunk --version 0.17.5`)
4. Run `download_makefiles.sh`
5. Install Rust Nightly and the WebAssembly target

```console
make rustup
```

6. Build client

```console
cd client
make release
```

7. Build and run server

```console
cd client
make run_release
```

8. Navigate to `https://localhost:8443/` and play!

## Official Server(s)

To avoid potential visibility-cheating, you are prohibited from using the open-source
client to play on official Kiomet server(s).

## Trademark

Kiomet is a trademark of Softbear, Inc.
