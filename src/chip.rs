//! Chip configuration infomation

use crate::information::Information;
#[cfg(feature = "serde")]
use crate::information::{deser_information, deser_vec_information};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uom::si::information::byte;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Chip {
    #[cfg_attr(feature = "serde", serde(default))]
    start_address: u64,
    page_size: PageSize,
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_information"))]
    total_size: Information,
}

#[derive(Error, Debug)]
enum ChipError {
    #[error("total size not a multiple of the page size")]
    TotalSizePageSizeMismatch,
}

impl Chip {
    fn new(
        page_size: Information,
        start_address: u64,
        total_size: Information,
    ) -> Result<Self, ChipError> {
        if (total_size % page_size) != Information::new::<byte>(0) {
            return Err(ChipError::TotalSizePageSizeMismatch);
        }
        Ok(Self {
            start_address,
            page_size: PageSize::Uniform(page_size),
            total_size,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(untagged))]
pub enum PageSize {
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_information"))]
    Uniform(Information),
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deser_vec_information"))]
    Heterogeneous(Vec<Information>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::information::byte;

    #[test]
    fn deser_uniform() {
        let input = r#"{ "page_size": 4 KiB, total_size: 16 KiB}"#;
        let chip: Chip = yaml_serde::from_str(input).unwrap();
        assert_eq!(
            chip.page_size,
            PageSize::Uniform(Information::new::<byte>(4 * 1024))
        );
    }

    #[test]
    fn deser_heterogeneous() {
        let input = r#"{ "page_size": [4 KiB, 4 KiB, 2048 B], total_size: 10 KiB}"#;
        let chip: Chip = yaml_serde::from_str(input).unwrap();
        assert_eq!(
            chip.page_size,
            PageSize::Heterogeneous(vec![
                Information::new::<byte>(4 * 1024),
                Information::new::<byte>(4 * 1024),
                Information::new::<byte>(2 * 1024),
            ])
        );
    }
}
