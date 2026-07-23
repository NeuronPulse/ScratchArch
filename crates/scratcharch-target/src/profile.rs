use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Endianness {
    #[default]
    Little,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum IntegerModel {
    #[default]
    ModularWrapping,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MemoryModel {
    #[default]
    FlatByteAddressable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AbiVersion {
    #[default]
    V0_1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProfile {
    pub name: &'static str,
    pub cell_width: u32,
    pub pointer_width: u32,
    pub endianness: Endianness,
    pub integer_model: IntegerModel,
    pub memory_model: MemoryModel,
    pub abi_version: AbiVersion,
}

impl TargetProfile {
    pub fn sa48() -> Self {
        TargetProfile {
            name: "sa48",
            cell_width: 48,
            pointer_width: 32,
            endianness: Endianness::Little,
            integer_model: IntegerModel::ModularWrapping,
            memory_model: MemoryModel::FlatByteAddressable,
            abi_version: AbiVersion::V0_1,
        }
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.cell_width == 0 {
            return Err(ProfileError::InvalidCellWidth(self.cell_width));
        }
        if !self.cell_width.is_multiple_of(8) {
            return Err(ProfileError::CellWidthNotByteMultiple(self.cell_width));
        }
        if self.pointer_width == 0 {
            return Err(ProfileError::InvalidPointerWidth(self.pointer_width));
        }
        if !self.pointer_width.is_multiple_of(8) {
            return Err(ProfileError::PointerWidthNotByteMultiple(self.pointer_width));
        }
        if self.pointer_width > self.cell_width {
            return Err(ProfileError::PointerWiderThanCell {
                pointer_width: self.pointer_width,
                cell_width: self.cell_width,
            });
        }
        if self.abi_version != AbiVersion::V0_1 {
            return Err(ProfileError::UnsupportedAbiVersion);
        }
        if self.integer_model != IntegerModel::ModularWrapping {
            return Err(ProfileError::UnsupportedIntegerModel);
        }
        if self.memory_model != MemoryModel::FlatByteAddressable {
            return Err(ProfileError::UnsupportedMemoryModel);
        }
        Ok(())
    }

    pub fn max_addressable_bytes(&self) -> u64 {
        1u64 << self.pointer_width
    }

    pub fn cells_for_type(&self, bit_width: u32) -> u32 {
        if bit_width == 0 {
            return 0;
        }
        bit_width.div_ceil(self.cell_width)
    }
}

impl fmt::Display for TargetProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScratchArch {}\n  cell width: {}\n  pointer width: {}\n  endianness: {}",
            self.name,
            self.cell_width,
            self.pointer_width,
            match self.endianness {
                Endianness::Little => "little",
                Endianness::Big => "big",
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    InvalidCellWidth(u32),
    CellWidthNotByteMultiple(u32),
    InvalidPointerWidth(u32),
    PointerWidthNotByteMultiple(u32),
    PointerWiderThanCell { pointer_width: u32, cell_width: u32 },
    UnsupportedAbiVersion,
    UnsupportedIntegerModel,
    UnsupportedMemoryModel,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::InvalidCellWidth(w) => write!(f, "invalid cell width: {} (must be > 0)", w),
            ProfileError::CellWidthNotByteMultiple(w) => {
                write!(f, "cell width {} is not a multiple of 8", w)
            }
            ProfileError::InvalidPointerWidth(w) => {
                write!(f, "invalid pointer width: {} (must be > 0)", w)
            }
            ProfileError::PointerWidthNotByteMultiple(w) => {
                write!(f, "pointer width {} is not a multiple of 8", w)
            }
            ProfileError::PointerWiderThanCell { pointer_width, cell_width } => {
                write!(
                    f,
                    "pointer width {} exceeds cell width {}",
                    pointer_width, cell_width
                )
            }
            ProfileError::UnsupportedAbiVersion => {
                write!(f, "unsupported ABI version (only v0.1 is supported)")
            }
            ProfileError::UnsupportedIntegerModel => {
                write!(f, "unsupported integer model (only modular wrapping is supported)")
            }
            ProfileError::UnsupportedMemoryModel => {
                write!(f, "unsupported memory model (only flat byte-addressable is supported)")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sa48_creation() {
        let p = TargetProfile::sa48();
        assert_eq!(p.name, "sa48");
        assert_eq!(p.cell_width, 48);
        assert_eq!(p.pointer_width, 32);
        assert_eq!(p.endianness, Endianness::Little);
    }

    #[test]
    fn test_sa48_validation_passes() {
        let p = TargetProfile::sa48();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_sa48_cells_for_type() {
        let p = TargetProfile::sa48();
        assert_eq!(p.cells_for_type(32), 1);
        assert_eq!(p.cells_for_type(48), 1);
        assert_eq!(p.cells_for_type(64), 2);
        assert_eq!(p.cells_for_type(1), 1);
    }

    #[test]
    fn test_sa48_display() {
        let p = TargetProfile::sa48();
        let s = p.to_string();
        assert!(s.contains("sa48"));
        assert!(s.contains("48"));
        assert!(s.contains("little"));
    }

    #[test]
    fn test_invalid_zero_cell_width() {
        let p = TargetProfile {
            name: "invalid",
            cell_width: 0,
            pointer_width: 32,
            endianness: Endianness::Little,
            integer_model: IntegerModel::ModularWrapping,
            memory_model: MemoryModel::FlatByteAddressable,
            abi_version: AbiVersion::V0_1,
        };
        assert_eq!(p.validate(), Err(ProfileError::InvalidCellWidth(0)));
    }

    #[test]
    fn test_pointer_wider_than_cell() {
        let p = TargetProfile {
            name: "invalid",
            cell_width: 32,
            pointer_width: 64,
            endianness: Endianness::Little,
            integer_model: IntegerModel::ModularWrapping,
            memory_model: MemoryModel::FlatByteAddressable,
            abi_version: AbiVersion::V0_1,
        };
        assert_eq!(
            p.validate(),
            Err(ProfileError::PointerWiderThanCell {
                pointer_width: 64,
                cell_width: 32
            })
        );
    }

    #[test]
    fn test_cell_width_not_byte_multiple() {
        let p = TargetProfile {
            name: "invalid",
            cell_width: 7,
            pointer_width: 32,
            endianness: Endianness::Little,
            integer_model: IntegerModel::ModularWrapping,
            memory_model: MemoryModel::FlatByteAddressable,
            abi_version: AbiVersion::V0_1,
        };
        assert_eq!(
            p.validate(),
            Err(ProfileError::CellWidthNotByteMultiple(7))
        );
    }
}
