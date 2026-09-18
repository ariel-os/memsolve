//! Memory sections.
//!
//! Sections can be maximized, to take up as much flash space as possible within the constraints.

use crate::chip::Chip;
#[cfg(feature = "uom")]
use crate::information::Information;
use thiserror::Error;
#[cfg(feature = "uom")]
use uom::si::information::byte;

#[cfg(feature = "serde")]
use crate::information::deser_option_information;

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SerdeSection {
    /// Name of this section
    pub name: String,
    #[serde(default)]
    pub(crate) boot: bool,
    #[serde(default)]
    pub(crate) maximize: bool,
    pub(crate) pages: Option<u64>,
    /// The size of the section in bytes
    #[serde(default, deserialize_with = "deser_option_information")]
    pub(crate) size: Option<u64>,
    #[serde(default)]
    pub(crate) address: Option<u64>,
    #[serde(default)]
    pub(crate) address_align: Option<u64>,
    #[serde(default)]
    pub(crate) relative_pages: i64,
    #[serde(default)]
    pub(crate) linker_name: Option<String>,
}

#[cfg(feature = "serde")]
impl From<SerdeSection> for Section<()> {
    fn from(value: SerdeSection) -> Self {
        Self {
            name: value.name,
            boot: value.boot,
            maximize: value.maximize,
            pages: value.pages,
            size: value.size,
            address: value.address,
            address_align: value.address_align,
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
    pub(crate) address_align: Option<u64>,
    pub(crate) relative_pages: i64,
    pub(crate) linker_name: Option<String>,
    pub(crate) metadata: MetaData,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Section<()> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SerdeSection::deserialize(deserializer).map(Into::into)
    }
}

/// Errors related to sections
#[derive(Error, Clone, Debug, PartialEq)]
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

        Self::check_valid_section_name(&name)?;

        Ok(Self {
            name,
            boot: false,
            maximize: false,
            pages: None,
            size: None,
            address: None,
            address_align: None,
            relative_pages: 0,
            linker_name: None,
            metadata: (),
        })
    }

    /// Check if `name` is a valid linker script section name.
    ///
    /// This is best effort for lack of exact spec.
    fn check_valid_section_name(name: &str) -> Result<(), SectionError> {
        if name.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_') {
            return Err(SectionError::InvalidSectionName);
        }
        Ok(())
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

    /// Set the address alignment restrictions for this section.
    #[must_use]
    pub fn set_address_align(mut self, align: u64) -> Self {
        self.address_align = Some(align);
        self
    }

    /// Clear the address alignment restrictions for this section.
    #[must_use]
    pub fn clear_address_align(mut self) -> Self {
        self.address_align = None;
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

    /// Set the linker script name used for this section.
    /// # Errors
    ///
    /// - [`SectionError::InvalidSectionName`]: when the `name` is not valid.
    pub fn set_linker_name(mut self, name: impl Into<String>) -> Result<Self, SectionError> {
        let name = name.into();

        Section::<()>::check_valid_section_name(&name)?;

        self.linker_name = Some(name);
        Ok(self)
    }

    /// Get the currently used linker name for this section.
    ///
    /// Equals the section name if unset.
    #[must_use]
    fn linker_name(&self) -> String {
        self.linker_name.as_ref().unwrap_or(&self.name).clone()
    }

    /// The section location is fully defined
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        let resolved_size = self.pages.is_some() || self.size.is_some();
        self.is_fixed() && resolved_size && !self.maximize
    }

    /// Returns true if this section needs to be maximized.
    #[must_use]
    pub fn needs_maximizing(&self) -> bool {
        self.maximize
    }

    /// Returns true if this section is not at a fixed position or requires resizing.
    #[must_use]
    pub fn needs_allocating(&self) -> bool {
        !self.needs_maximizing() && !self.is_fixed()
    }

    pub(crate) fn as_resolved(
        &self,
        chip: &Chip,
    ) -> Result<ResolvedSection<MetaData>, SectionError> {
        if !self.is_resolved() {
            return Err(SectionError::UnresolvedSection);
        }
        let address = self.start_address(chip)?;

        let (pages, size) = self.resolve_page_and_size(chip)?;

        Ok(ResolvedSection {
            name: self.name.clone(),
            pages,
            size,
            address,
            linker_name: self.linker_name(),
            metadata: self.metadata.clone(),
        })
    }

    pub(crate) fn start_address(&self, chip: &Chip) -> Result<u64, SectionError> {
        let address = if self.boot {
            chip.start_address()
        } else {
            let Some(address) = self.address else {
                return Err(SectionError::UnresolvedSection);
            };
            address
        };
        Ok(address)
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

    fn resolve_page_and_size(&self, chip: &Chip) -> Result<(u64, u64), SectionError> {
        let address = self.start_address(chip)?;
        let (pages, size) = match (self.pages, self.size) {
            (None, None) => return Err(SectionError::UnresolvedSection),
            (None, Some(size)) => (self.required_pages(chip.page_size(address))?, size),
            (Some(pages), None) => (pages, chip.page_size(address) * pages),
            (Some(pages), Some(size)) => {
                // size of the pages requested
                let size_of_pages = chip.page_size(address) * pages;
                if size > size_of_pages {
                    (self.required_pages(chip.page_size(address))?, size)
                } else {
                    (pages, size_of_pages)
                }
            }
        };
        Ok((pages, size))
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
            address_align: self.address_align,
            relative_pages: self.relative_pages,
            linker_name: self.linker_name,
            metadata: data,
        }
    }

    /// Remove the extra metadata from this section.
    #[must_use]
    pub fn clear_metadata(self) -> Section<()> {
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

    #[test]
    fn page_vs_bytes() {
        let section = Section::new("t")
            .unwrap()
            .set_address(0)
            .set_size(10)
            .set_pages(2);
        let chip = Chip::new(10, 0, 20).unwrap();
        let (pages, size) = section.resolve_page_and_size(&chip).unwrap();
        assert_eq!(pages, 2);
        assert_eq!(size, 20);

        let chip = Chip::new(2, 0, 20).unwrap();
        let (pages, size) = section.resolve_page_and_size(&chip).unwrap();
        assert_eq!(pages, 5);
        assert_eq!(size, 10);

        let chip = Chip::new(5, 0, 20).unwrap();
        let (pages, size) = section.resolve_page_and_size(&chip).unwrap();
        assert_eq!(pages, 2);
        assert_eq!(size, 10);
    }
}
