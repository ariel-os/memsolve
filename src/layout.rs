//! Memory layout description.
#[cfg(feature = "serde")]
use serde::Deserialize;
use std::ops::{Deref, DerefMut, Index};

use crate::section::{ResolvedSection, Section};

#[cfg(feature = "serde")]
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub(crate) struct SerdeLayout {
    sections: Vec<super::section::SerdeSection>,
}

#[cfg(feature = "serde")]
impl From<SerdeLayout> for Layout<()> {
    fn from(value: SerdeLayout) -> Self {
        Layout::new(value.sections.into_iter().map(Into::into).collect())
    }
}

/// List of sections for a requested layout.
#[derive(Debug, PartialEq, Clone)]
pub struct Layout<MetaData: Clone> {
    pub(crate) sections: Vec<Section<MetaData>>,
}

#[cfg(feature = "serde")]
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

    /// Create an empty set of sections.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Extend the sections with a section.
    pub fn push(&mut self, section: Section<MetaData>) {
        self.sections.push(section);
    }

    pub(crate) fn maximizing_sections(&self) -> impl Iterator<Item = &Section<MetaData>> + Clone {
        self.iter().filter(|s| s.needs_maximizing())
    }

    pub(crate) fn allocatable_sections(&self) -> impl Iterator<Item = &Section<MetaData>> + Clone {
        self.iter().filter(|s| s.needs_allocating())
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
        Self::empty()
    }
}

/// Defines a fully resolved layout.
#[derive(Clone, Debug)]
pub struct ResolvedLayout<MetaData: Clone> {
    pub(crate) sections: Vec<ResolvedSection<MetaData>>,
}

impl<MetaData: Clone> ResolvedLayout<MetaData> {
    /// Extend this resolved layout with other resolved sections.
    pub fn extend(&mut self, extend: impl Iterator<Item = ResolvedSection<MetaData>>) {
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

impl<MetaData: Clone> From<Vec<ResolvedSection<MetaData>>> for ResolvedLayout<MetaData> {
    fn from(value: Vec<ResolvedSection<MetaData>>) -> Self {
        Self { sections: value }
    }
}

impl<MetaData: Clone> Index<usize> for ResolvedLayout<MetaData> {
    type Output = ResolvedSection<MetaData>;

    fn index(&self, index: usize) -> &Self::Output {
        self.sections.index(index)
    }
}

impl<MetaData: Clone> Deref for ResolvedLayout<MetaData> {
    type Target = [ResolvedSection<MetaData>];

    fn deref(&self) -> &Self::Target {
        self.sections.deref()
    }
}

impl<MetaData: Clone> DerefMut for ResolvedLayout<MetaData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sections.deref_mut()
    }
}
