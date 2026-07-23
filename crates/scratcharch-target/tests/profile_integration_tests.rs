use scratcharch_target::profile::{TargetProfile, Endianness, IntegerModel, MemoryModel, AbiVersion, ProfileError};

#[test]
fn test_sa48_defaults() {
    let p = TargetProfile::sa48();
    p.validate().expect("SA48 should be valid");
    assert_eq!(p.name, "sa48");
    assert_eq!(p.cell_width, 48);
    assert_eq!(p.pointer_width, 32);
    assert_eq!(p.endianness, Endianness::Little);
    assert_eq!(p.integer_model, IntegerModel::ModularWrapping);
    assert_eq!(p.memory_model, MemoryModel::FlatByteAddressable);
    assert_eq!(p.abi_version, AbiVersion::V0_1);
}

#[test]
fn test_sa48_max_addressable() {
    let p = TargetProfile::sa48();
    assert_eq!(p.max_addressable_bytes(), 1u64 << 32);
}

#[test]
fn test_sa48_cells() {
    let p = TargetProfile::sa48();
    assert_eq!(p.cells_for_type(1), 1);
    assert_eq!(p.cells_for_type(8), 1);
    assert_eq!(p.cells_for_type(32), 1);
    assert_eq!(p.cells_for_type(48), 1);
    assert_eq!(p.cells_for_type(64), 2);
    assert_eq!(p.cells_for_type(96), 2);
    assert_eq!(p.cells_for_type(0), 0);
}

#[test]
fn test_sa48_display() {
    let p = TargetProfile::sa48();
    let s = format!("{}", p);
    assert!(s.contains("sa48"));
    assert!(s.contains("48"));
    assert!(s.contains("32"));
    assert!(s.contains("little"));
}

#[test]
fn test_invalid_widths() {
    fn make(cell: u32, ptr: u32) -> TargetProfile {
        TargetProfile {
            name: "test",
            cell_width: cell,
            pointer_width: ptr,
            endianness: Endianness::Little,
            integer_model: IntegerModel::ModularWrapping,
            memory_model: MemoryModel::FlatByteAddressable,
            abi_version: AbiVersion::V0_1,
        }
    }

    assert_eq!(make(0, 32).validate(), Err(ProfileError::InvalidCellWidth(0)));
    assert_eq!(make(7, 32).validate(), Err(ProfileError::CellWidthNotByteMultiple(7)));
    assert_eq!(make(32, 0).validate(), Err(ProfileError::InvalidPointerWidth(0)));
    assert_eq!(
        make(32, 64).validate(),
        Err(ProfileError::PointerWiderThanCell { pointer_width: 64, cell_width: 32 })
    );
}
