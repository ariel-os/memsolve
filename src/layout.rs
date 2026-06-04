//! Memory layout description.
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut, Index};
use uom::si::information::byte;

use crate::{
    bin::{Bin, MemoryBin},
    chip::{Chip, PageSize},
    section::{ResolvedSection, Section},
};

use crate::information::Information;

/// List of sections for a requested layout.
#[derive(Debug, Default, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Layout {
    sections: Vec<Section>,
}

impl Layout {
    /// Create a new set of sections.
    #[must_use]
    pub fn new(sections: Vec<Section>) -> Self {
        Self { sections }
    }

    /// Extend the sections with a section.
    pub fn push(&mut self, section: Section) {
        self.sections.push(section);
    }

    pub(crate) fn resolved_sections(&self) -> impl Iterator<Item = ResolvedSection> + Clone {
        self.iter().filter_map(|s| s.as_resolved().ok())
    }

    pub(crate) fn maximizing_sections(&self) -> impl Iterator<Item = &Section> + Clone {
        self.iter().filter(|s| s.needs_maximizing())
    }

    pub(crate) fn allocatable_sections(&self) -> impl Iterator<Item = &Section> + Clone {
        self.iter().filter(|s| s.needs_allocating())
    }

    pub(crate) fn memory_bins(&self, chip: &Chip) -> Bin {
        // find bins between fixed sections
        let mut fixed = self.resolved_sections().collect::<Vec<_>>();
        fixed.sort_by_key(|a| a.address);
        let start_address = chip.start_address();
        if fixed
            .first()
            .is_some_and(|first| first.address != start_address)
            || fixed.is_empty()
        {
            fixed.insert(
                0,
                ResolvedSection::new(
                    "a".into(),
                    0,
                    Information::new::<byte>(0),
                    start_address,
                    None,
                ),
            );
        }

        if fixed
            .last()
            .is_some_and(|last| (last.address + last.size.get::<byte>()) != chip.end_address())
            || fixed.is_empty()
        {
            fixed.push(ResolvedSection::new(
                "z".into(),
                0,
                Information::new::<byte>(0),
                chip.end_address(),
                None,
            ));
        }

        let page_size = match chip.page_size {
            PageSize::Uniform(quantity) => quantity,
            PageSize::Heterogeneous(_) => todo!(),
        };
        Bin::new(
            fixed
                .iter()
                .tuple_windows()
                .filter_map(|(s1, s2)| {
                    let space_between = s1.space_between(s2);
                    if space_between == 0 {
                        return None;
                    }
                    let start_address = s1.next_free_address();
                    let end_address = space_between + start_address;
                    Some(MemoryBin {
                        start_address,
                        end_address,
                        page_size,
                    })
                })
                .collect::<Vec<_>>(),
        )
    }
    pub(crate) fn num_pages(&self) -> u64 {
        self.iter().fold(0, |acc, s| acc + s.pages.unwrap_or(0))
    }

    /// Searches for a section with the provided name.
    pub fn find(&self, name: &impl PartialEq<String>) -> Option<&Section> {
        self.iter().find(|s| name.eq(&s.name))
    }

    fn find_mut(&mut self, name: &impl PartialEq<String>) -> Option<&mut Section> {
        self.iter_mut().find(|s| name.eq(&s.name))
    }

    /// Merge anothor layout into this layout.
    ///
    /// Sections in the other layout that do not exist in this layout are added.
    /// Sections that exist in both layouts are merged with [`Section::merge`].
    pub fn merge(&mut self, other: &Self) {
        for s in other.iter() {
            if let Some(existing) = self.find_mut(&s.name) {
                existing.merge(s);
            } else {
                self.push(s.clone());
            }
        }
    }
}

impl Index<usize> for Layout {
    type Output = Section;

    fn index(&self, index: usize) -> &Self::Output {
        self.sections.index(index)
    }
}

impl Deref for Layout {
    type Target = [Section];

    fn deref(&self) -> &Self::Target {
        self.sections.deref()
    }
}

impl DerefMut for Layout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sections.deref_mut()
    }
}
