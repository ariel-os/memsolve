//! Memsolve is a crate to generate ROM Memory Layouts for microcontrollers and other embedded
//! devices.
//!
//! Microcontrollers have limited flash (ROM) available, separated into pages. This flash can used
//! for multiple purposes, a bootloader, data storage, multiple applications. memsolve allows for
//! describing the different requirements of these sections, whether thats a set number of pages or
//! a minimal size, and uses a linear solver to resolve the layout into addresses for the flash
//! layout. This layout can then be used as basis for the `memory.x` file  in projects such as
//! [Embassy] or [Ariel OS][ariel_os]
//!
//! # Overview
//!
//! This section gives a brief overview of the primary types in this crate:
//!
//! - [`Memory`] is the primary object and represents the memory layout.
//! - [`Chip`] describes the target of the layout, including the total size of the flash and the
//!   page size
//! - [`Section`] describes the requirements on a single section.
//!
//! # Example: simple layout
//!
//! This example shows how to generate a layout for a microcontroller where one section will
//! contain the application and another contains some configuration. The application section must be as
//! large as possible within the flash and the configuration section requires 3 pages, but no
//! specific minimum size.
//!
//! ```
//! use memsolve::{Memory, section::Section, chip::Chip};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Our example chip has 64 KiB Flash with 2048 byte pages
//! let chip = Chip::new(2048, 0x800_000, 64 * 1024)?;
//! let mut memory = Memory::new(chip);
//!
//! // Add the application section
//! memory.add_section(
//!     Section::new("app")?
//!     .set_boot(true)
//!     .set_maximize(true)
//! );
//! // Add the configuration storage section
//! memory.add_section(
//!     Section::new("config")?
//!     .set_pages(3)
//!     );
//!
//! // This layout is fully resolved
//! let layout = memory.resolve_layout()?;
//!
//! assert_eq!(layout[0].address, 0x800_000);
//! assert_eq!(layout[0].pages, 29);
//! assert_eq!(layout[1].address, 0x80E_800);
//! assert_eq!(layout[1].pages, 3);
//! # Ok(())
//! # }
//! ```
//!
//! # Copyright and License
//!
//! Memsolve is licensed under either of
//!
//! - Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
//! - MIT license ([LICENSE-MIT](./LICENSE-MIT) or <https://opensource.org/licenses/MIT>)
//!
//!at your option.
//!
//! Copyright (C) 2026 Freie Universität Berlin, Koen Zandberg
//!
//! [ariel_os]: https://ariel-os.org/

#![warn(clippy::pedantic)]
#![warn(missing_docs)]

use thiserror::Error;

mod bin;
pub mod chip;
#[cfg(feature = "esp")]
pub mod esp;
mod information;
pub mod layout;
pub mod section;
mod solver;

use crate::bin::{Bin, MemoryBin};
use crate::chip::{Chip, PageSize};
use crate::layout::{Layout, ResolvedLayout};
use crate::section::{ResolvedSection, Section};
use crate::solver::{solve, solve_free};

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeMemory {
    chip: Chip,
    #[cfg_attr(feature = "serde", serde(flatten))]
    layout: layout::SerdeLayout,
}

#[cfg(feature = "serde")]
impl From<SerdeMemory> for Memory<()> {
    fn from(value: SerdeMemory) -> Self {
        Memory {
            chip: value.chip,
            layout: value.layout.into(),
        }
    }
}

/// Memory layout description.
#[derive(Debug, Clone, PartialEq)]
pub struct Memory<MetaData: Clone> {
    chip: Chip,
    layout: Layout<MetaData>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Memory<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SerdeMemory::deserialize(deserializer).map(Into::into)
    }
}

/// Memory layout generation errors.
#[derive(Debug, Error, PartialEq)]
pub enum MemoryError {
    /// Multiple sections are marked as bootable.
    #[error("multiple sections defined as bootable")]
    MultipleBootable,

    /// Insufficient memory is available to fit sections.
    #[error("memory is too small to allocate all sections")]
    MemoryTooSmall,

    /// Memory layout can not be resolved.
    #[error("unresolvable layout")]
    UnresolvableLayout,

    /// Address is too large to represent in the solver.
    #[error("address space too large for solver")]
    AddressTooLarge,

    /// Time budget exceeded for solver.
    #[error("time exceeded")]
    TimeExceeded,

    /// Section related error.
    #[error("section error: {0}")]
    SectionError(#[from] section::SectionError),
}

impl From<solver::SolverError> for MemoryError {
    fn from(value: solver::SolverError) -> Self {
        match value {
            solver::SolverError::Solver(_) => MemoryError::UnresolvableLayout,
            solver::SolverError::TooManyFlashPages | solver::SolverError::ConversionError => {
                MemoryError::AddressTooLarge
            }
            solver::SolverError::NoAllocationRegions => MemoryError::MemoryTooSmall,
            solver::SolverError::SectionError(section_error) => {
                MemoryError::SectionError(section_error)
            }
            solver::SolverError::TimeExceeded => MemoryError::TimeExceeded,
        }
    }
}

impl<MetaData: Clone> Memory<MetaData> {
    /// Create a new memory.
    #[must_use]
    pub fn new(chip: Chip) -> Self {
        Memory {
            chip,
            layout: Layout::default(),
        }
    }

    /// Set the current layout used by the memory.
    pub fn set_layout(&mut self, layout: Layout<MetaData>) {
        self.layout = layout;
    }

    /// Add a memory section to the layout in the memory.
    pub fn add_section(&mut self, section: Section<MetaData>) {
        self.layout.push(section);
    }

    fn resolved_sections(&self) -> impl Iterator<Item = ResolvedSection<MetaData>> + Clone {
        self.layout
            .iter()
            .filter_map(|s| s.as_resolved(&self.chip).ok())
    }

    fn memory_bins(&self) -> Bin {
        struct FixedSection {
            start: u64,
            end: u64,
        }

        impl FixedSection {
            fn from_resolved<MetaData: Clone>(resolved: &ResolvedSection<MetaData>) -> Self {
                FixedSection {
                    start: resolved.address,
                    end: resolved.address + resolved.size,
                }
            }

            fn space_between(&self, other: &Self) -> u64 {
                other.start - self.end
            }
        }

        // find bins between fixed sections
        let mut fixed = self
            .resolved_sections()
            .map(|s| FixedSection::from_resolved(&s))
            .collect::<Vec<_>>();
        fixed.sort_by_key(|a| a.start);
        let start_address = self.chip.start_address();
        if fixed
            .first()
            .is_some_and(|first| first.start != start_address)
            || fixed.is_empty()
        {
            fixed.insert(
                0,
                FixedSection {
                    start: start_address,
                    end: start_address,
                },
            );
        }

        let end = self.chip.end_address();

        if fixed.last().is_some_and(|last| last.end != end) || fixed.is_empty() {
            fixed.push(FixedSection { start: end, end });
        }

        let page_size = match self.chip.page_size {
            PageSize::Uniform(quantity) => quantity,
            PageSize::Heterogeneous(_) => todo!(),
        };
        Bin::new(
            fixed
                .array_windows()
                .filter_map(|[s1, s2]| {
                    let space_between = s1.space_between(s2);
                    if space_between == 0 {
                        return None;
                    }
                    Some(MemoryBin {
                        start_address: s1.end,
                        end_address: s2.start,
                        page_size,
                    })
                })
                .collect::<Vec<_>>(),
        )
    }

    /// Returns a mutable reference to the layout.
    #[must_use]
    pub fn layout_mut(&mut self) -> &mut Layout<MetaData> {
        &mut self.layout
    }

    /// Returns a reference to the layout.
    #[must_use]
    pub fn layout(&self) -> &Layout<MetaData> {
        &self.layout
    }

    /// Fully resolves the layout.
    ///
    /// # Errors
    ///
    /// This returns the following errors:
    ///
    /// - [`MemoryError::MultipleBootable`]: When multiple sections are marked as bootable
    /// - [`MemoryError::MemoryTooSmall`]: When the chip does not have sufficient memory available to fit all
    ///   sections.
    /// - [`MemoryError::UnresolvableLayout`]: When the requirements can not be resolved into a full layout.
    pub fn resolve_layout(&self) -> Result<ResolvedLayout<MetaData>, MemoryError> {
        // Fix bootable section to first address
        if self.layout.num_bootable() > 1 {
            return Err(MemoryError::MultipleBootable);
        }
        let bins = self.memory_bins();

        let sections = self.layout.allocatable_sections();

        let bin_pages = bins.num_pages();
        let section_pages = self.layout.num_pages();
        if section_pages > bin_pages {
            return Err(MemoryError::MemoryTooSmall);
        }
        // overflow should not occur due to the above check
        let mut free_pages = bin_pages.wrapping_sub(section_pages);
        let maximized_sections = self.layout.maximizing_sections();
        let mut resolved = loop {
            if free_pages == 0 && maximized_sections.clone().count() > 0 {
                return Err(MemoryError::UnresolvableLayout);
            }
            let mut maxed_sections = solve_free(&maximized_sections, free_pages)
                .map_err(|_| MemoryError::UnresolvableLayout)?;
            let next_free_pages = {
                let assigned_pages = maxed_sections
                    .iter()
                    .fold(0, |acc, x| acc + x.pages.unwrap_or(0));
                if assigned_pages > 0 {
                    assigned_pages - 1
                } else {
                    0
                }
            };
            maxed_sections.extend(sections.clone().cloned());

            let res = solve(&bins, &maxed_sections.iter());
            if let Ok(resolved) = res {
                break resolved;
            } else if let Err(e) = res.clone()
                && !matches!(e, solver::SolverError::Solver(microlp::Error::Infeasible))
            {
                return Err(e.into());
            }
            free_pages = next_free_pages;
        };
        resolved.extend(self.resolved_sections());
        resolved.sort_by_key(|s| s.address);

        Ok(resolved)
    }

    /// Returns true if the memory layout is fully defined.
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.layout.iter().all(section::Section::is_resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins() {
        let chip = crate::Chip::new(1000, 0, 10000).unwrap();
        let mut memory = Memory::new(chip);
        [0, 2000, 3000, 7000]
            .into_iter()
            .map(|i| {
                Section::new(format!("sec{i}"))
                    .unwrap()
                    .set_address(i)
                    .set_size(1000)
                    .set_pages(1)
            })
            .for_each(|s| memory.add_section(s));

        let bins = memory.memory_bins();
        assert_eq!(bins.len(), 3);
        assert_eq!(bins[0].start_address, 1000);
        assert_eq!(bins[0].end_address, 2000);
        assert_eq!(bins[1].start_address, 4000);
        assert_eq!(bins[1].end_address, 7000);
        assert_eq!(bins[2].start_address, 8000);
        assert_eq!(bins[2].end_address, 10000);
    }

    #[test]
    fn solver() {
        let chip = crate::Chip::new(1000, 0, 20000).unwrap();

        let mut memory = Memory::new(chip);

        [1000, 5000] // 1 page, 3 page and 4 page bins
            .into_iter()
            .map(|i| {
                Section::new(format!("fixed{i}"))
                    .unwrap()
                    .set_address(i)
                    .set_size(1000)
                    .set_pages(1)
            })
            .for_each(|s| memory.add_section(s));

        [1, 5, 1]
            .into_iter()
            .enumerate()
            .map(|(i, pages)| Section::new(format!("flex{i}")).unwrap().set_pages(pages))
            .for_each(|s| memory.add_section(s));
        let resolved = memory.resolve_layout().unwrap();

        for [prev, next] in resolved.array_windows() {
            // No overlap between sections
            assert!(prev.address + prev.size <= next.address);
        }
    }

    #[test]
    fn solve_boot_maximize() {
        let chip = crate::Chip::new(1000, 0, 20000).unwrap();

        let mut memory = Memory::new(chip);

        [4000] // 1 page, 3 page and 4 page bins
            .into_iter()
            .map(|i| {
                Section::new(format!("fixed{i}"))
                    .unwrap()
                    .set_address(i)
                    .set_size(1000)
                    .set_pages(1)
            })
            .for_each(|s| memory.add_section(s));

        memory.add_section(
            Section::new("flash")
                .unwrap()
                .set_boot(true)
                .set_maximize(true),
        );
        let resolved = memory.resolve_layout().unwrap();

        let boot = &resolved[0];
        assert_eq!(boot.name, "flash");
        assert_eq!(boot.address, 0);
    }

    #[test]
    fn min_byte_size() {
        let chip = crate::Chip::new(1, 0, 30).unwrap();

        let mut memory = Memory::new(chip);
        memory.add_section(Section::new("test").unwrap().set_size(20));
        let resolved = memory.resolve_layout().unwrap();
        let sec = &resolved[0];
        assert_eq!(sec.size, 20);
        assert_eq!(sec.pages, 20);
    }

    #[test]
    fn min_page_size() {
        let chip = crate::Chip::new(1, 0, 30).unwrap();

        let mut memory = Memory::new(chip);
        memory.add_section(Section::new("test").unwrap().set_pages(20));
        let resolved = memory.resolve_layout().unwrap();
        let sec = &resolved[0];
        assert_eq!(sec.size, 20);
        assert_eq!(sec.pages, 20);
    }

    #[test]
    fn too_small_memory() {
        let chip = crate::Chip::new(2, 0, 30).unwrap();

        let mut memory = Memory::new(chip);
        memory.add_section(Section::new("test").unwrap().set_pages(20));
        let err = memory.resolve_layout().unwrap_err();
        assert_eq!(err, MemoryError::MemoryTooSmall);
    }

    #[test]
    fn resolve_rp() {
        let chip = crate::Chip::new(256, 0x1000_0000, 8192 * 256).unwrap();
        let mut layout = Memory::new(chip);
        layout.add_section(Section::new("BOOT2").unwrap().set_size(256).set_boot(true));
        layout.add_section(
            Section::new("FLASH")
                .unwrap()
                .set_maximize(true)
                .set_address(0x1000_0100),
        );
        let resolved = layout.resolve_layout().unwrap();
        let sec = &resolved[0];
        assert_eq!(sec.size, 256);
        let sec = &resolved[1];
        assert_eq!(sec.address, 0x1000_0100);
        assert_eq!(sec.size, 2_096_896);
    }

    #[test]
    fn linker_name() {
        let chip = crate::Chip::new(1, 0, 1).unwrap();

        let mut memory = Memory::new(chip);
        memory.add_section(
            Section::new("test")
                .unwrap()
                .set_pages(1)
                .set_linker_name("test_linker_name")
                .unwrap(),
        );
        let resolved = memory.resolve_layout().unwrap();
        assert_eq!(resolved[0].linker_name, "test_linker_name");
    }
}
