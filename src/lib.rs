//! Memsolve is a crate to generate ROM Memory Layouts for microcontrollers and other embedded
//! devices.
//!
//! Microcontrollers have limited flash (ROM) available, separated into pages. This flash can used
//! for multiple purposes, a bootloader, data storage, multiple applications. memsolve allows for
//! describing the different requirements of these sections, whether thats a set number of pages or
//! a minimal size, and uses a linear solver to resolve the layout into addresses for the flash
//! layout. This layout can then be used as basis for the `memory.x` file  in projects such as
//! [Embassy] or [Ariel OS]
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

#![warn(clippy::pedantic)]
#![warn(missing_docs)]
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod bin;
pub mod chip;
mod information;
pub mod layout;
pub mod section;
mod solver;

use crate::chip::Chip;
use crate::layout::Layout;
use crate::section::{ResolvedLayout, Section};
use crate::solver::{solve, solve_free};

/// Memory layout description.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Memory {
    chip: Chip,
    #[cfg_attr(feature = "serde", serde(flatten))]
    layout: Layout,
}

/// Memory layout generation errors.
#[derive(Debug, Error)]
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
}

impl Memory {
    /// Create a new memory.
    #[must_use]
    pub fn new(chip: Chip) -> Self {
        Memory {
            chip,
            layout: Layout::default(),
        }
    }

    /// Set the current layout used by the memory.
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
    }

    /// Add a memory section to the layout in the memory.
    pub fn add_section(&mut self, section: Section) {
        self.layout.push(section);
    }

    /// Returns a mutable reference to the layout.
    #[must_use]
    pub fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    /// Returns a reference to the layout.
    #[must_use]
    pub fn layout(&self) -> &Layout {
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
    pub fn resolve_layout(&self) -> Result<ResolvedLayout, MemoryError> {
        // Fix bootable section to first address
        if self.layout.iter().filter(|s| s.boot).count() > 1 {
            return Err(MemoryError::MultipleBootable);
        }
        let bins = self.layout.memory_bins(&self.chip);

        let sections = self.layout.allocatable_sections();

        let bin_pages = bins.num_pages();
        let section_pages = self.layout.num_pages();
        if section_pages > bin_pages {
            return Err(MemoryError::MemoryTooSmall);
        }
        let mut free_pages = bin_pages - section_pages;
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
                    .fold(0, |acc, x| acc + x.pages.unwrap());
                if assigned_pages > 0 {
                    assigned_pages - 1
                } else {
                    0
                }
            };
            maxed_sections.extend(sections.clone().cloned());

            let res =
                solve(&bins, &maxed_sections.iter()).map_err(|_| MemoryError::UnresolvableLayout);
            if let Ok(resolved) = res {
                break resolved;
            }
            free_pages = next_free_pages;
        };
        resolved.extend(self.layout.resolved_sections());
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

        let bins = memory.layout.memory_bins(&memory.chip);
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
        use itertools::Itertools;

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

        for (prev, next) in resolved.iter().tuple_windows() {
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
}
