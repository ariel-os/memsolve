//! Chip configuration infomation

#[cfg(feature = "uom")]
use crate::information::Information;
#[cfg(feature = "serde")]
use crate::information::{deser_information, deser_vec_information};
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(feature = "uom")]
use uom::si::information::byte;

/// Describes the memory layout of a chip / microcontroller.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Chip {
    #[cfg_attr(feature = "serde", serde(default))]
    start_address: u64,
    pub(crate) page_size: PageSize,
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_information"))]
    total_size: u64,
}

/// Chip-related errors
#[derive(Error, Debug)]
pub enum ChipError {
    /// Total size of the chip is not a multiple of the page size.
    #[error("total size not a multiple of the page size")]
    TotalSizePageSizeMismatch,
}

impl Chip {
    /// Creates a new chip layout from a description in `Bytes`.
    ///
    /// # Errors
    ///
    /// Will return a `ChipError::TotalSizePageSizeMismatch` when the total size is not a multiple
    /// of the page size.
    pub fn new(page_size: u64, start_address: u64, total_size: u64) -> Result<Self, ChipError> {
        if !total_size.is_multiple_of(page_size) {
            return Err(ChipError::TotalSizePageSizeMismatch);
        }
        Ok(Self {
            start_address,
            page_size: PageSize::Uniform(page_size),
            total_size,
        })
    }

    /// Creates a new chip layout from `u64` interpreted as bytes.
    ///
    /// # Errors
    ///
    /// Will return a `ChipError::TotalSizePageSizeMismatch` when the total size is not a multiple
    /// of the page size.
    #[cfg(feature = "uom")]
    pub fn new_bytes(
        page_size: impl Into<Information>,
        start_address: u64,
        total_size: impl Into<Information>,
    ) -> Result<Self, ChipError> {
        Self::new(
            page_size.into().get::<byte>(),
            start_address,
            total_size.into().get::<byte>(),
        )
    }

    /// Returns the boot aaddress of this chip.
    ///
    /// The section marked as bootable will be allocated on this address.
    #[must_use]
    pub fn start_address(&self) -> u64 {
        self.start_address
    }

    /// Returns the last address of this chip.
    #[must_use]
    pub fn end_address(&self) -> u64 {
        self.start_address + self.total_size
    }
}

/// Chip flash page size layout.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(untagged))]
pub enum PageSize {
    /// Chip contains a uniform flash page layout.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_information"))]
    Uniform(u64),
    /// Chip contains flash pages with different sizes.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_vec_information"))]
    Heterogeneous(Vec<u64>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deser_uniform() {
        let input = r#"{ "page_size": 4 KiB, "total_size": 16 KiB}"#;
        let chip: Chip = yaml_serde::from_str(input).unwrap();
        assert_eq!(chip.page_size, PageSize::Uniform(4 * 1024));
    }

    #[test]
    fn deser_heterogeneous() {
        let input = r#"{ "page_size": [4 KiB, 4 KiB, 2048 B], total_size: 10 KiB}"#;
        let chip: Chip = yaml_serde::from_str(input).unwrap();
        assert_eq!(
            chip.page_size,
            PageSize::Heterogeneous(vec![4 * 1024, 4 * 1024, 2 * 1024,])
        );
    }
}
