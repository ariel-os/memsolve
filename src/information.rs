#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer};
#[cfg(feature = "uom")]
pub use uom::si::{information::byte, u64::Information};

#[cfg(feature = "serde")]
pub(crate) fn deser_information<'de, D>(d: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = <String>::deserialize(d)?;
    s.parse::<Information>()
        .map_err(serde::de::Error::custom)
        .map(|i| i.get::<byte>())
}

#[cfg(feature = "serde")]
pub(crate) fn deser_option_information<'de, D>(d: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(d)?;
    if let Some(s) = s {
        return s
            .parse::<Information>()
            .map_err(serde::de::Error::custom)
            .map(|i| Some(i.get::<byte>()));
    }
    Ok(None)
}

#[cfg(feature = "serde")]
pub(crate) fn deser_vec_information<'de, D>(d: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let l: Vec<String> = Vec::deserialize(d)?;
    l.into_iter()
        .map(|s| {
            s.parse::<Information>()
                .map_err(serde::de::Error::custom)
                .map(|i| i.get::<byte>())
        })
        .collect()
}
