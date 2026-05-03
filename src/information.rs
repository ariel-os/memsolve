#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer};
pub use uom::si::u64::Information;

#[cfg(feature = "serde")]
pub(crate) fn deser_information<'de, D>(d: D) -> Result<Information, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = <&str>::deserialize(d)?;
    s.parse::<Information>().map_err(serde::de::Error::custom)
}

#[cfg(feature = "serde")]
pub(crate) fn deser_option_information<'de, D>(d: D) -> Result<Option<Information>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(d)?;
    if let Some(s) = s {
        return s
            .parse::<Information>()
            .map_err(serde::de::Error::custom)
            .map(Some);
    }
    Ok(None)
}

#[cfg(feature = "serde")]
pub(crate) fn deser_vec_information<'de, D>(d: D) -> Result<Vec<Information>, D::Error>
where
    D: Deserializer<'de>,
{
    let l: Vec<String> = Vec::deserialize(d)?;
    l.into_iter()
        .map(|s| s.parse::<Information>().map_err(serde::de::Error::custom))
        .collect()
}
