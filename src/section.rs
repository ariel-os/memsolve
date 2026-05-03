//! Memory sections.
use crate::information::{deser_option_information, Information};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A Memory section.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Section {
    name: String,
    #[cfg_attr(feature = "serde", serde(default))]
    boot: bool,
    pages: Option<u64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, deserialize_with = "deser_option_information")
    )]
    size: Option<Information>,
    #[cfg_attr(
        feature = "serde",
        serde(default, deserialize_with = "deser_option_information")
    )]
    address: Option<Information>,
}

#[derive(Error, Debug)]
pub enum SectionError {
    #[error("incorrect section name")]
    InvalidSectionName,
}

impl Section {
    /// Create a new section
    pub fn new(name: impl Into<String>) -> Result<Self, SectionError> {
        let name = name.into();
        if name.chars().any(|c| !c.is_ascii_alphanumeric()) {
            return Err(SectionError::InvalidSectionName);
        }

        Ok(Self {
            name,
            boot: false,
            pages: None,
            size: None,
            address: None,
        })
    }

    /// Set the boot flag.
    pub fn set_boot(mut self, boot: bool) -> Self {
        self.boot = boot;
        self
    }

    /// Set the minimum number of pages for this section.
    pub fn num_pages(mut self, pages: u64) -> Self {
        self.pages = Some(pages);
        self
    }

    /// Clear the minimum number of pages for this section.
    pub fn clear_pages(mut self) -> Self {
        self.pages = None;
        self
    }

    /// Set the minimum size for this section.
    pub fn set_size(mut self, bytes: impl Into<Information>) -> Self {
        self.size = Some(bytes.into());
        self
    }

    /// Clear the minimum size for this section.
    pub fn clear_size(mut self) -> Self {
        self.size = None;
        self
    }

    /// Set the exact address for this section.
    pub fn set_address(mut self, address: impl Into<Information>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Clear the exact address for this section.
    pub fn clear_address(mut self) -> Self {
        self.address = None;
        self
    }

    pub fn is_fixed(&self) -> bool {
        self.boot || self.address.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::information::byte;

    #[test]
    fn name() {
        assert!(Section::new("test").is_ok());
        assert!(Section::new("test2").is_ok());
        assert!(Section::new("test space").is_err());
    }

    #[test]
    fn deser() {
        let input = r#"
        name: test
        boot: false
        pages: 2
        size: 3 KiB
        "#;
        let section: Section = yaml_serde::from_str(input).unwrap();
        assert_eq!(section.name, "test");
        assert!(!section.boot);
        assert_eq!(section.pages, Some(2));
        assert_eq!(section.size, Some(Information::new::<byte>(3 * 1024)));
    }
}
