use super::{Bits, array_offset};

#[test]
fn integer_constructors_create_bits_directly() {
    assert_eq!(Bits::<1>::from_bool(true).to_u128(), 1);
    assert_eq!(Bits::<8>::from_u8(0xa5).to_u128(), 0xa5);
    assert_eq!(Bits::<16>::from_u16(0xabcd).to_u128(), 0xabcd);
    assert_eq!(Bits::<32>::from_u32(0x89ab_cdef).to_u128(), 0x89ab_cdef);
    assert_eq!(Bits::<64>::from_u64(u64::MAX).to_u128(), u64::MAX.into());
    assert_eq!(Bits::<8>::from_usize(0xa5).to_u128(), 0xa5);
    assert_eq!(Bits::<128>::from_u128(u128::MAX).to_u128(), u128::MAX);
    assert_eq!(Bits::<8>::from_u8(42).to_usize(), 42);
}

#[test]
#[should_panic(expected = "bit vector value exceeds usize")]
fn usize_conversion_overflow_panics() {
    Bits::<129>::from_words(&[0, 0, 1]).to_usize();
}

#[test]
fn arbitrary_width_arithmetic_masks_to_width() {
    let top = Bits::<129>::from_words(&[u64::MAX, u64::MAX, 1]);
    let one = Bits::<129>::from_u8(1);
    let sum = top.add(&one);
    assert_eq!(sum.to_u128(), 0);
    assert!(!sum.bit(128));
    let wide = Bits::<257>::from_u8(42);
    assert_eq!(wide.to_u128(), 42);
    assert_eq!(wide.bits.len(), 257);
    let high = Bits::<129>::from_words(&[0, 0, 1]);
    assert!(high.bit(128));
    assert_eq!(high.to_words(), vec![0, 0, 1]);
    let high_value = Bits::<201>::from_words(&[1, 0, 0, 256]);
    assert!(high_value.bit(200));
    assert!(high_value.bit(0));
}

#[test]
fn division_and_signed_operations_match_fixed_width_rtl() {
    let value = Bits::<8>::from_u8(100);
    assert_eq!(value.div_unsigned(&Bits::from_u8(7)).to_u128(), 14);
    assert_eq!(value.div_unsigned(&Bits::zero()).to_u128(), 255);
    let negative = Bits::<8>::from_u8(0xf2);
    assert_eq!(negative.div_signed(&Bits::from_u8(7)).to_u128(), 0xfe);
    assert_eq!(
        negative
            .shift::<8, 8>(&Bits::from_u8(2), true, true)
            .to_u128(),
        0xfc
    );
}

#[test]
fn selections_and_declared_array_indices_are_lsb_first() {
    let mut value = Bits::<16>::from_u16(0xabcd);
    assert_eq!(value.select::<8>(4).to_u128(), 0xbc);
    value.assign_select(4, 4, &Bits::<4>::from_u8(2));
    assert_eq!(value.to_u128(), 0xab2d);
    assert_eq!(array_offset(2, 2, 0, 8), 0);
    assert_eq!(array_offset(0, 2, 0, 8), 16);
}

#[test]
#[should_panic(expected = "array index 3 is outside declared range [2:0]")]
fn declared_array_index_out_of_bounds_panics() {
    array_offset(3, 2, 0, 8);
}
