# memsolve ![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue) [![memsolve on crates.io](https://img.shields.io/crates/v/memsolve)](https://crates.io/crates/memsolve) [![memsolve on docs.rs](https://docs.rs/memsolve/badge.svg)](https://docs.rs/memsolve) [![Source Code Repository](https://img.shields.io/badge/Code-On%20GitHub-blue?logo=GitHub)](https://github.com/ariel-os/memsolve)

Memsolve is a crate to generate ROM Memory Layouts for microcontrollers and other embedded
devices.

Microcontrollers have limited flash (ROM) available, separated into pages. This flash can used
for multiple purposes, a bootloader, data storage, multiple applications. memsolve allows for
describing the different requirements of these sections, whether thats a set number of pages or
a minimal size, and uses a linear solver to resolve the layout into addresses for the flash
layout. This layout can then be used as basis for the `memory.x` file  in projects such as
[Embassy][__link0] or [Ariel OS][__link1]

## Overview

This section gives a brief overview of the primary types in this crate:

* [`Memory`][__link2] is the primary object and represents the memory layout.
* [`Chip`][__link3] describes the target of the layout, including the total size of the flash and the
  page size
* [`Section`][__link4] describes the requirements on a single section.

## Example: simple layout

This example shows how to generate a layout for a microcontroller where one section will
contain the application and another contains some configuration. The application section must be as
large as possible within the flash and the configuration section requires 3 pages, but no
specific minimum size.

```rust
use memsolve::{Memory, section::Section, chip::Chip};

// Our example chip has 64 KiB Flash with 2048 byte pages
let chip = Chip::new(2048, 0x800_000, 64 * 1024)?;
let mut memory = Memory::new(chip);

// Add the application section
memory.add_section(
    Section::new("app")?
    .set_boot(true)
    .set_maximize(true)
);
// Add the configuration storage section
memory.add_section(
    Section::new("config")?
    .set_pages(3)
    );

// This layout is fully resolved
let layout = memory.resolve_layout()?;

assert_eq!(layout[0].address, 0x800_000);
assert_eq!(layout[0].pages, 29);
assert_eq!(layout[1].address, 0x80E_800);
assert_eq!(layout[1].pages, 3);
```

## Copyright and License

Memsolve is licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE][__link5] or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT][__link6] or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (C) 2026 Freie Universität Berlin, Koen Zandberg


 [__cargo_doc2readme_dependencies_info]: ggGmYW0CYXZlMC43LjJhdIQb2o_SNWoR6AAb3_T-k0ODPHwbnQW7uS_D2XsbjVFFtK-lC3BhYvVhcoQbVYCInZLP7Usb0xyGqYkHI-Ub4Dx8kfIJ8_4b-ENpTTGHXwBhZIKCZ0VtYmFzc3n2gmhtZW1zb2x2ZWUwLjIuNA
 [__link0]: https://crates.io/crates/Embassy
 [__link1]: https://ariel-os.org/
 [__link2]: https://docs.rs/memsolve/0.2.4/memsolve/struct.Memory.html
 [__link3]: https://docs.rs/memsolve/0.2.4/memsolve/?search=chip::Chip
 [__link4]: https://docs.rs/memsolve/0.2.4/memsolve/?search=section::Section
 [__link5]: ./LICENSE-APACHE
 [__link6]: ./LICENSE-MIT
