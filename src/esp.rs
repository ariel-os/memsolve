//! Support for ESP32 partition table generation.
use esp_idf_part::{Flags, Partition, PartitionTable, SubType, Type};

use crate::{
    Memory,
    layout::{Layout, ResolvedLayout},
    section::{ResolvedSection, Section},
};

/// A section with ESP32 metadata.
pub type EspSection = Section<EspMetaData>;
/// A layout with ESP32 metadata in the sections.
pub type EspLayout = Layout<EspMetaData>;
/// A memory with ESP32 metadata in the sections.
pub type EspMemory = Memory<EspMetaData>;

/// ESP32 partition metadata.
#[derive(Clone, Debug)]
pub struct EspMetaData {
    /// The ESP32 partition type.
    pub partition_type: esp_idf_part::Type,
    /// The ESP32 partition subtype.
    pub partition_subtype: esp_idf_part::SubType,
    /// The ESP32 partition flags.
    pub flags: esp_idf_part::Flags,
}

impl EspMetaData {
    /// Creates new partition metadata
    #[must_use]
    pub fn new(partition_type: Type, partition_subtype: SubType, flags: Flags) -> Self {
        Self {
            partition_type,
            partition_subtype,
            flags,
        }
    }

    /// Set the flags on the metadata.
    #[must_use]
    pub fn set_flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }
}

impl Section<()> {
    /// Add ESP partition data to this section.
    #[must_use]
    pub fn add_esp_metadata(self, partition_type: Type, subtype: impl Into<SubType>) -> EspSection {
        let metadata = EspMetaData::new(partition_type, subtype.into(), Flags::empty());
        self.replace_metadata(metadata)
    }
}

impl Memory<()> {
    /// Convert the memory to use [`EspMetaData`]
    ///
    /// # Panics
    ///
    /// Panics when the layout in the memory contains sections
    #[must_use]
    pub fn with_esp_metadata(self) -> EspMemory {
        Memory {
            chip: self.chip,
            layout: self.layout.with_esp_metadata(),
        }
    }
}

impl Layout<()> {
    /// Converts an empty layout to use [`EspMetaData`]
    ///
    /// # Panics
    ///
    /// Panics when the layout contains sections
    #[must_use]
    #[allow(clippy::assert_is_empty)]
    pub fn with_esp_metadata(self) -> EspLayout {
        assert!(self.sections.is_empty());

        Layout::empty()
    }
}

impl Section<EspMetaData> {
    /// set ESP partition flags on this section.
    #[must_use]
    pub fn set_esp_flags(mut self, flags: Flags) -> Section<EspMetaData> {
        self.metadata = self.metadata.set_flags(flags);
        self
    }
}

impl ResolvedSection<EspMetaData> {
    /// Generate a [`esp_idf_part::Partition`] from this section.
    #[cfg(feature = "esp")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn as_esp_partition(&self) -> Partition {
        Partition::new(
            &self.linker_name,
            self.metadata.partition_type,
            self.metadata.partition_subtype,
            self.address as u32,
            self.size as u32,
            self.metadata.flags,
        )
    }
}

impl ResolvedLayout<EspMetaData> {
    /// Generate a [`esp_idf_part::PartitionTable`] from this layout.
    #[must_use]
    pub fn into_esp_partition(&self) -> PartitionTable {
        let partitions = self
            .sections
            .iter()
            .map(ResolvedSection::as_esp_partition)
            .collect();
        PartitionTable::new(partitions)
    }
}

#[cfg(test)]
mod tests {
    use esp_idf_part::{AppType, DataType};

    use crate::{Memory, chip::Chip};

    use super::*;

    #[test]
    fn default_layout() {
        let esp_single_factory_app = "# ESP-IDF Partition Table\n\
        # Name,Type,SubType,Offset,Size,Flags\n\
        nvs,data,nvs,0x9000,0x6000,\n\
        phy_init,data,phy,0xf000,0x1000,\n\
        factory,app,factory,0x10000,0x100000,\n\
        ";

        let nvs = Section::new("nvs")
            .unwrap()
            .set_size(0x6000)
            .set_address(0x9000)
            .add_esp_metadata(Type::Data, DataType::Nvs);
        let phy_init = Section::new("phy_init")
            .unwrap()
            .set_size(0x1000)
            .set_address(0xf000)
            .add_esp_metadata(Type::Data, DataType::Phy);
        let factory = Section::new("factory")
            .unwrap()
            .set_size(1024 * 1024)
            .set_address_align(64 * 1024)
            .add_esp_metadata(Type::App, AppType::Factory);

        let chip = Chip::new(512, 0, 2048 * 1048).unwrap();
        let mut memory = Memory::new(chip);
        memory.add_section(nvs);
        memory.add_section(phy_init);
        memory.add_section(factory);

        let resolved = memory.resolve_layout().unwrap();

        let csv = resolved.into_esp_partition().to_csv().unwrap();
        assert_eq!(&csv, esp_single_factory_app);
    }
}
