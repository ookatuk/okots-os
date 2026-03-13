## features
* loading screen in **before** `exit_boot_services`
* before exit boot services and later HDR(In UEFI mode, need GOP Bitmask.)
* **True Type Font(ttf)** and **ligatures**  support
###  Enable/disable features during build

#### Runtime checks
* [x] Essentials
* [x] Normal
* [x] Overprotective
* [x] **Boot options**: Memory check, boot, and shutdown.
* [x] **Full memory check**: Built-in check (patterns: addr, 0x00, 0xff, 0x55, 0xAA).
* [ ] **Overprotective**: Built-in but disabled by default.
* [ ] **Ligatures**: Powered by `rustybizz`. Available both before and after `exit_boot_services`.
* [x] **UART and more!**: See `Cargo.toml` for details.

## build/debug dependencies

* qemu-system-x86_64
* xorriso
* mkfs.msdos(dosfstools)
* ovmf

## how to build?

> [!NOTE]
> Primary development is done on Linux.
> While `.bat` files are provided for Windows,
> they are experimental.
> **WSL2 is highly recommended** for a stable build environment.

1. install `cargo-make`
> run `cargo install cargo-make`

2. run `cargo make init_project`
> or `./init.(sh/bat)`

> [!TIP]
> `scripts/internal_init_script` is a common initialization script for Linux builds, not for the entire project. 

* if you need iso,

> run `cargo make iso`

* if you need efi,

> run `cargo make build`\
> or
> run `cargo build`

> [!WARNING]
> **Microcode Notice**: 
> Microcode is prepared during the `cargo make init_project` phase, but it is **not** automatically updated or downloaded during runtime by the OS. 
>
> If you need to manually refresh or fetch the latest microcode after the initial setup, use the following task:
> ```bash
> cargo make update_microcode
> ```

> [!TIP]
> if you need run and use Official Log Viewer,(in native Linux / GUI support version WSL)
>
> run `cargo make run`
> or run `cargo run`
