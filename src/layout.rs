//! Memory layout description.
use itertools::Itertools;
#[cfg(feature = "serde")]
use serde::Deserialize;
use std::ops::{Deref, DerefMut, Index};

use crate::{
    bin::{Bin, MemoryBin},
    chip::{Chip, PageSize},
    section::{ResolvedSection, Section, SerdeSection},
};

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub(crate) struct SerdeLayout {
    sections: Vec<SerdeSection>,
}

impl From<SerdeLayout> for Layout<()> {
    fn from(value: SerdeLayout) -> Self {
        Layout::new(value.sections.into_iter().map(Into::into).collect())
    }
}

/// List of sections for a requested layout.
#[derive(Debug, PartialEq, Clone)]
pub struct Layout<MetaData: Clone> {
    sections: Vec<Section<MetaData>>,
}

impl<'de> Deserialize<'de> for Layout<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SerdeLayout::deserialize(deserializer).map(Into::into)
    }
}

impl<MetaData: Clone> Layout<MetaData> {
    /// Create a new set of sections.
    #[must_use]
    pub fn new(sections: Vec<Section<MetaData>>) -> Self {
        Self { sections }
    }

    /// Extend the sections with a section.
    pub fn push(&mut self, section: Section<MetaData>) {
        self.sections.push(section);
    }

    pub(crate) fn resolved_sections(&self) -> impl Iterator<Item = ResolvedSection> + Clone {
        self.iter().filter_map(|s| s.as_resolved().ok())
    }

    pub(crate) fn maximizing_sections(&self) -> impl Iterator<Item = &Section<MetaData>> + Clone {
        self.iter().filter(|s| s.needs_maximizing())
    }

    pub(crate) fn allocatable_sections(&self) -> impl Iterator<Item = &Section<MetaData>> + Clone {
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
                ResolvedSection::new("a".into(), 0, 0, start_address, None),
            );
        }

        if fixed
            .last()
            .is_some_and(|last| (last.address + last.size) != chip.end_address())
            || fixed.is_empty()
        {
            fixed.push(ResolvedSection::new(
                "z".into(),
                0,
                0,
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

    pub(crate) fn num_bootable(&self) -> usize {
        self.iter().filter(|s| s.boot).count()
    }

    /// Searches for a section with the provided name.
    pub fn find(&self, name: &impl PartialEq<String>) -> Option<&Section<MetaData>> {
        self.iter().find(|s| name.eq(&s.name))
    }

    fn find_mut(&mut self, name: &impl PartialEq<String>) -> Option<&mut Section<MetaData>> {
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

impl<MetaData: Clone> Index<usize> for Layout<MetaData> {
    type Output = Section<MetaData>;

    fn index(&self, index: usize) -> &Self::Output {
        self.sections.index(index)
    }
}

impl<MetaData: Clone> Deref for Layout<MetaData> {
    type Target = [Section<MetaData>];

    fn deref(&self) -> &Self::Target {
        self.sections.deref()
    }
}

impl<MetaData: Clone> DerefMut for Layout<MetaData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sections.deref_mut()
    }
}

impl<MetaData: Clone> Default for Layout<MetaData> {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
        }
    }
}

/// Defines a fully resolved layout.
#[derive(Clone, Debug)]
pub struct ResolvedLayout {
    sections: Vec<ResolvedSection>,
}

impl ResolvedLayout {
    /// Extend this resolved layout with other resolved sections.
    pub fn extend(&mut self, extend: impl Iterator<Item = ResolvedSection>) {
        self.sections.extend(extend);
    }

    /// Generate a [`ld_memory::Memory`] from this layout.
    #[must_use]
    pub fn into_memory(&self) -> ld_memory::Memory {
        let mut memory = ld_memory::Memory::new();
        for s in &self.sections {
            memory = memory.add_section(s.as_memory_section());
        }
        memory
    }
}

impl From<Vec<ResolvedSection>> for ResolvedLayout {
    fn from(value: Vec<ResolvedSection>) -> Self {
        Self { sections: value }
    }
}

impl Index<usize> for ResolvedLayout {
    type Output = ResolvedSection;

    fn index(&self, index: usize) -> &Self::Output {
        self.sections.index(index)
    }
}

impl Deref for ResolvedLayout {
    type Target = [ResolvedSection];

    fn deref(&self) -> &Self::Target {
        self.sections.deref()
    }
}

impl DerefMut for ResolvedLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sections.deref_mut()
    }
}
