//! Memory sections.
//!
//! Sections can be maximized, to take up as much flash space as possible within the constraints.

#[cfg(feature = "uom")]
use crate::information::Information;
#[cfg(feature = "serde")]
use crate::information::deser_option_information;
#[cfg(feature = "serde")]
use serde::Deserialize;
use thiserror::Error;
#[cfg(feature = "uom")]
use uom::si::information::byte;

/// A Memory section.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub(crate) struct SerdeSection {
    /// Name of this section
    pub name: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) boot: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) maximize: bool,
    pub(crate) pages: Option<u64>,
    /// The size of the section in bytes
    #[cfg_attr(
        feature = "serde",
        serde(default, deserialize_with = "deser_option_information")
    )]
    pub(crate) size: Option<u64>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) address: Option<u64>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) relative_pages: i64,
    #[cfg_attr(feature = "serde", serde(default))]
    pub(crate) linker_name: Option<String>,
}

impl From<SerdeSection> for Section<()> {
    fn from(value: SerdeSection) -> Self {
        Self {
            name: value.name,
            boot: value.boot,
            maximize: value.maximize,
            pages: value.pages,
            size: value.size,
            address: value.address,
            relative_pages: value.relative_pages,
            linker_name: value.linker_name,
            metadata: (),
        }
    }
}

/// A Memory section.
#[derive(Debug, Clone, PartialEq)]
pub struct Section<MetaData: Clone> {
    /// Name of this section
    pub name: String,
    pub(crate) boot: bool,
    pub(crate) maximize: bool,
    pub(crate) pages: Option<u64>,
    /// The size of the section in bytes
    pub(crate) size: Option<u64>,
    pub(crate) address: Option<u64>,
    pub(crate) relative_pages: i64,
    pub(crate) linker_name: Option<String>,
    pub(crate) metadata: MetaData,
}

impl<'de> Deserialize<'de> for Section<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SerdeSection::deserialize(deserializer).map(Into::into)
    }
}

/// Errors related to sections
#[derive(Error, Debug, PartialEq)]
pub enum SectionError {
    /// Section name cannot be used in linker scripts.
    #[error("incorrect section name")]
    InvalidSectionName,
    /// Section is not fully resolved while it must be already.
    #[error("section not completely resolved")]
    UnresolvedSection,
    /// Section does not have a size bound and can not be resolved.
    #[error("No page and byte size bound for section")]
    NoSizeBound,
}

impl Section<()> {
    /// Create a new section.
    ///
    /// The `name` of the section must be a valid ``LD_MEMORY`` memory name.
    ///
    /// # Errors
    ///
    /// - [`SectionError::InvalidSectionName`]: when the `name` is not valid.
    pub fn new(name: impl Into<String>) -> Result<Self, SectionError> {
        let name = name.into();
        if name.chars().any(|c| !c.is_ascii_alphanumeric()) {
            return Err(SectionError::InvalidSectionName);
        }

        Ok(Self {
            name,
            boot: false,
            maximize: false,
            pages: None,
            size: None,
            address: None,
            relative_pages: 0,
            linker_name: None,
            metadata: (),
        })
    }
}

impl<MetaData: Clone> Section<MetaData> {
    /// Extends the section with the requirements from another.
    ///
    /// boot and maximize are OR'ed between the two instances. number of pages and byte size take
    /// the maximum of both. Address is ignored unless one instance doesn't have it set.
    pub fn merge(&mut self, sec: &Self) {
        self.boot = self.boot || sec.boot;
        self.maximize = self.maximize || sec.maximize;
        if let Some(pages) = sec.pages
            && self.pages.is_none_or(|p| p < pages)
        {
            self.pages = Some(pages);
        }
        if let Some(size) = sec.size
            && self.size.is_none_or(|s| s < size)
        {
            self.size = Some(size);
        }
        if let Some(address) = sec.address
            && self.address.is_none()
        {
            self.address = Some(address);
        }
        if self.relative_pages < sec.relative_pages {
            self.relative_pages = sec.relative_pages;
        }

        if let Some(linker_name) = &sec.linker_name
            && self.linker_name.is_none()
        {
            self.linker_name = Some(linker_name.clone());
        }
    }

    /// Set the boot flag.
    #[must_use]
    pub fn set_boot(mut self, boot: bool) -> Self {
        self.boot = boot;
        self
    }

    /// Set the maximize flag.
    #[must_use]
    pub fn set_maximize(mut self, maximize: bool) -> Self {
        self.maximize = maximize;
        self
    }

    /// Set the minimum number of pages for this section.
    #[must_use]
    pub fn set_pages(mut self, pages: u64) -> Self {
        self.pages = Some(pages);
        self
    }

    /// Clear the minimum number of pages for this section.
    #[must_use]
    pub fn clear_pages(mut self) -> Self {
        self.pages = None;
        self
    }

    /// Set the minimum size in bytes for this section.
    #[must_use]
    pub fn set_size(mut self, bytes: u64) -> Self {
        self.size = Some(bytes);
        self
    }

    /// Set the minimum size for this section.
    #[must_use]
    #[cfg(feature = "uom")]
    pub fn set_size_bytes(self, bytes: impl Into<Information>) -> Self {
        self.set_size(bytes.into().get::<byte>())
    }

    /// Clear the minimum size for this section.
    #[must_use]
    pub fn clear_size(mut self) -> Self {
        self.size = None;
        self
    }

    /// Set the exact address for this section.
    #[must_use]
    pub fn set_address(mut self, address: u64) -> Self {
        self.address = Some(address);
        self
    }

    /// Set the number of extra pages required while maximizing this section
    ///
    /// Only used when `Self::maximize` is true
    #[must_use]
    pub fn set_relative_pages(mut self, num: i64) -> Self {
        self.relative_pages = num;
        self
    }

    /// clear the number of extra pages required while maximizing this section
    #[must_use]
    pub fn clear_relative_pages(mut self) -> Self {
        self.relative_pages = 0;
        self
    }

    /// Clear the exact address for this section.
    #[must_use]
    pub fn clear_address(mut self) -> Self {
        self.address = None;
        self
    }

    /// The section must be allocated at an exact address.
    #[must_use]
    pub fn is_fixed(&self) -> bool {
        self.boot || self.address.is_some()
    }

    /// Get the number of pages required for this section.
    ///
    /// Takes the required size into account for calculating the required number of pages.
    #[must_use]
    #[cfg(feature = "uom")]
    pub fn pages_required(&self, page_size: Information) -> u64 {
        let Some(size) = self.size else {
            return self.pages.unwrap_or(0);
        };
        size.div_ceil(page_size.get::<byte>())
    }

    /// The section location is fully defined
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.address.is_some() && self.pages.is_some() && self.size.is_some() && !self.maximize
    }

    /// Returns true if this section needs to be maximized.
    #[must_use]
    pub fn needs_maximizing(&self) -> bool {
        self.maximize
    }

    /// Returns true if this section is not at a fixed position or requires resizing.
    #[must_use]
    pub fn needs_allocating(&self) -> bool {
        !self.maximize && self.address.is_none()
    }

    pub(crate) fn as_resolved(&self) -> Result<ResolvedSection<MetaData>, SectionError> {
        let (Some(pages), Some(size), Some(address)) = (self.pages, self.size, self.address) else {
            return Err(SectionError::UnresolvedSection);
        };
        Ok(ResolvedSection {
            name: self.name.clone(),
            pages,
            size,
            address,
            linker_name: self.linker_name.as_ref().unwrap_or(&self.name).clone(),
            metadata: self.metadata.clone(),
        })
    }

    pub(crate) fn required_pages(&self, page_size: u64) -> Result<u64, SectionError> {
        let page_required = self.pages.unwrap_or(0);
        let size_pages = self.size.map_or(0, |size| size.div_ceil(page_size));
        if page_required == 0 && size_pages == 0 {
            Err(SectionError::NoSizeBound)
        } else {
            Ok(std::cmp::max(page_required, size_pages))
        }
    }
    pub(crate) fn required_pages_in_bin(
        &self,
        bin: &crate::bin::MemoryBin,
    ) -> Result<u64, SectionError> {
        self.required_pages(bin.page_size)
    }

    pub(crate) fn replace_metadata<NewData: Clone>(self, data: NewData) -> Section<NewData> {
        Section {
            name: self.name,
            boot: self.boot,
            maximize: self.maximize,
            pages: self.pages,
            size: self.size,
            address: self.address,
            relative_pages: self.relative_pages,
            linker_name: self.linker_name,
            metadata: data,
        }
    }

    pub(crate) fn clear_metadata(self) -> Section<()> {
        self.replace_metadata(())
    }
}

impl<MetaData: Clone> std::fmt::Display for Section<MetaData> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Section({})", self.name)
    }
}

/// A fully resolved memory section
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSection<MetaData: Clone> {
    /// Name of this section.
    pub name: String,
    /// Number of pages this section uses.
    pub pages: u64,
    /// Size of this section in bytes.
    pub size: u64,
    /// Start address of this section.
    pub address: u64,
    /// Linker script section name.
    pub linker_name: String,

    /// Extra metadata
    pub metadata: MetaData,
}

impl<MetaData: Clone> ResolvedSection<MetaData> {
    pub(crate) fn new(
        name: String,
        pages: u64,
        size: u64,
        address: u64,
        linker_name: Option<String>,
        metadata: MetaData,
    ) -> Self {
        let linker_name = linker_name.unwrap_or(name.clone());
        Self {
            name,
            pages,
            size,
            address,
            linker_name,
            metadata,
        }
    }

    /// Generate a [`ld_memory::MemorySection`] from this section.
    #[must_use]
    pub fn as_memory_section(&self) -> ld_memory::MemorySection {
        ld_memory::MemorySection::new(&self.linker_name, self.address, self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        assert!(Section::new("test").is_ok());
        assert!(Section::new("test2").is_ok());
        assert!(Section::new("test space").is_err());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn deser() {
        let input = r#"
        "name": test
        "boot": false
        "pages": 2
        "size": 3 KiB
        "#;
        let section: Section<_> = yaml_serde::from_str(input).unwrap();
        assert_eq!(section.name, "test");
        assert!(!section.boot);
        assert_eq!(section.pages, Some(2));
        assert_eq!(section.size, Some(3 * 1024));
    }

    #[test]
    fn merge() {
        let mut section = Section::new("t").unwrap().set_size(100);
        let second = Section::new("t").unwrap().set_pages(1);
        section.merge(&second);
        assert_eq!(section.pages, Some(1));
        assert_eq!(section.size, Some(100));
    }
}
