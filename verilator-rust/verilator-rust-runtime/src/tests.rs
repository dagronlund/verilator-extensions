use std::mem::size_of;

use num_bigint::BigUint;

use super::{Bits, array_offset};

#[test]
fn integer_constructors_create_bits_directly() {
    assert_eq!(Bits::<1, bool>::from_bool(true).to_u128(), 1);
    assert_eq!(Bits::<8, u8>::from_u8(0xa5).to_u128(), 0xa5);
    assert_eq!(Bits::<16, u16>::from_u16(0xabcd).to_u128(), 0xabcd);
    assert_eq!(
        Bits::<32, u32>::from_u32(0x89ab_cdef).to_u128(),
        0x89ab_cdef
    );
    assert_eq!(
        Bits::<64, u64>::from_u64(u64::MAX).to_u128(),
        u64::MAX.into()
    );
    assert_eq!(Bits::<8, u8>::from_usize(0xa5).to_u128(), 0xa5);
    assert_eq!(Bits::<128, u128>::from_u128(u128::MAX).to_u128(), u128::MAX);
    assert_eq!(Bits::<8, u8>::from_u8(42).to_usize(), 42);
}

#[test]
#[should_panic(expected = "bit vector value exceeds usize")]
fn usize_conversion_overflow_panics() {
    Bits::<129, BigUint>::from_words(&[0, 0, 1]).to_usize();
}

#[test]
fn arbitrary_width_arithmetic_masks_to_width() {
    let top = Bits::<129, BigUint>::from_words(&[u64::MAX, u64::MAX, 1]);
    let one = Bits::<129, BigUint>::from_u8(1);
    let sum = top.add(&one);
    assert_eq!(sum.to_u128(), 0);
    assert!(!sum.bit(128));
    let wide = Bits::<257, BigUint>::from_u8(42);
    assert_eq!(wide.to_u128(), 42);
    assert_eq!(Bits::<257, BigUint>::width(), 257);
    let high = Bits::<129, BigUint>::from_words(&[0, 0, 1]);
    assert!(high.bit(128));
    assert_eq!(high.to_words(), vec![0, 0, 1]);
    let high_value = Bits::<201, BigUint>::from_words(&[1, 0, 0, 256]);
    assert!(high_value.bit(200));
    assert!(high_value.bit(0));
}

#[test]
fn division_and_signed_operations_match_fixed_width_rtl() {
    let value = Bits::<8, u8>::from_u8(100);
    assert_eq!(value.div_unsigned(&Bits::from_u8(7)).to_u128(), 14);
    assert_eq!(value.div_unsigned(&Bits::zero()).to_u128(), 255);
    let negative = Bits::<8, u8>::from_u8(0xf2);
    assert_eq!(negative.div_signed(&Bits::from_u8(7)).to_u128(), 0xfe);
    assert_eq!(
        negative
            .shift::<8, 8, u8, u8>(&Bits::from_u8(2), true, true)
            .to_u128(),
        0xfc
    );
}

#[test]
fn selections_and_declared_array_indices_are_lsb_first() {
    let mut value = Bits::<16, u16>::from_u16(0xabcd);
    assert_eq!(value.select::<8, u8>(4).to_u128(), 0xbc);
    value.assign_select(4, 4, &Bits::<4, u8>::from_u8(2));
    assert_eq!(value.to_u128(), 0xab2d);
    assert_eq!(array_offset(2, 2, 0, 8), 0);
    assert_eq!(array_offset(0, 2, 0, 8), 16);
}

#[test]
#[should_panic(expected = "array index 3 is outside declared range [2:0]")]
fn declared_array_index_out_of_bounds_panics() {
    array_offset(3, 2, 0, 8);
}

#[test]
fn storage_layout_matches_selected_primitives() {
    assert_eq!(size_of::<Bits<1, bool>>(), size_of::<bool>());
    assert_eq!(size_of::<Bits<7, u8>>(), size_of::<u8>());
    assert_eq!(size_of::<Bits<9, u16>>(), size_of::<u16>());
    assert_eq!(size_of::<Bits<17, u32>>(), size_of::<u32>());
    assert_eq!(size_of::<Bits<33, u64>>(), size_of::<u64>());
    assert_eq!(size_of::<Bits<65, u128>>(), size_of::<u128>());
    assert_eq!(size_of::<Bits<129, BigUint>>(), size_of::<BigUint>());
    assert_eq!(size_of::<Bits<4096, BigUint>>(), size_of::<BigUint>());
}

fn check_arithmetic<const WIDTH: usize, S: super::storage::Storage>() {
    let mask = if WIDTH == 128 {
        u128::MAX
    } else {
        (1u128 << WIDTH) - 1
    };
    let samples = [0, 1, 2, 3, mask / 2, mask / 2 + 1, mask - 1, mask];
    for lhs in samples {
        for rhs in samples {
            let a = Bits::<WIDTH, S>::from_u128(lhs);
            let b = Bits::<WIDTH, S>::from_u128(rhs);
            let lhs = lhs & mask;
            let rhs = rhs & mask;
            assert_eq!(a.add(&b).to_u128(), lhs.wrapping_add(rhs) & mask);
            assert_eq!(a.sub(&b).to_u128(), lhs.wrapping_sub(rhs) & mask);
            assert_eq!(a.mul(&b).to_u128(), lhs.wrapping_mul(rhs) & mask);
            assert_eq!(
                a.div_unsigned(&b).to_u128(),
                lhs.checked_div(rhs).unwrap_or(mask)
            );
        }
    }
}

#[test]
fn arithmetic_wraps_at_storage_and_rtl_boundaries() {
    check_arithmetic::<1, bool>();
    check_arithmetic::<7, u8>();
    check_arithmetic::<8, u8>();
    check_arithmetic::<9, u16>();
    check_arithmetic::<16, u16>();
    check_arithmetic::<17, u32>();
    check_arithmetic::<32, u32>();
    check_arithmetic::<33, u64>();
    check_arithmetic::<64, u64>();
    check_arithmetic::<65, u128>();
    check_arithmetic::<127, u128>();
    check_arithmetic::<128, u128>();
}

#[test]
fn biguint_arithmetic_and_cross_storage_operations() {
    type Wide = Bits<257, BigUint>;
    let top = Wide::from_words(&[u64::MAX; 5]);
    assert_eq!(
        top.to_words(),
        vec![u64::MAX, u64::MAX, u64::MAX, u64::MAX, 1]
    );
    let one = Wide::from_u8(1);
    assert_eq!(top.add(&one), Wide::zero());
    assert_eq!(Wide::zero().sub(&one), top);
    assert_eq!(top.mul(&top), one);
    assert_eq!(one.div_unsigned(&Wide::zero()), top);
    let high = Wide::from_words(&[0, 0, 0, 0, 1]);
    assert_eq!(
        high.div_unsigned(&Wide::from_u8(2)).to_words(),
        vec![0, 0, 0, 1 << 63, 0]
    );
    assert_eq!(top.div_signed(&one), top);
    assert_eq!(high.div_signed(&top), high); // minimum signed value / -1 wraps
    let negative = Bits::<7, u8>::from_u8(0x7e);
    let extended = negative.resize::<257, BigUint>(true);
    assert_eq!(extended, top.sub(&one));
    assert_eq!(extended.resize::<7, u8>(false), negative);
    assert_eq!(extended.select::<128, u128>(127).to_u128(), u128::MAX);
    let joined = Bits::<129, BigUint>::concat(
        &Bits::<1, bool>::from_bool(true),
        &Bits::<128, u128>::from_u128(42),
    );
    assert_eq!(joined.to_words(), vec![42, 0, 1]);
    let mut copy = joined.clone();
    copy.assign_select(120, 9, &Bits::<9, u16>::zero());
    assert!(joined.bit(128));
    assert!(!copy.bit(128));
    assert_eq!(format!("{:?}", Bits::<3, u8>::from_u8(5)), "3'b101");
}

#[test]
fn zero_width_is_empty_and_masks_values() {
    let zero = Bits::<0, bool>::from_u8(255);
    assert_eq!(zero.to_u128(), 0);
    assert_eq!(zero.bit_not(), zero);
    assert_eq!(zero.div_unsigned(&zero), zero);
    assert!(zero.reduce_and().truthy());
    assert!(!zero.reduce_or().truthy());
    assert_eq!(zero.resize::<129, BigUint>(true), Bits::zero());
}

#[test]
#[should_panic(expected = "storage is too narrow")]
fn rejects_insufficient_storage() {
    let _ = Bits::<9, u8>::zero();
}
