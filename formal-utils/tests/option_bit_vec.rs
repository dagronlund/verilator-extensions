use formal_utils::option_bit_vec::OptionBitVec;

#[test]
fn test_option_bit_vec() {
    let mut bits = OptionBitVec::new(64);

    for index in 0..64 {
        // Check initial
        assert_eq!(bits.get(index), None);

        // Set true and false
        bits.set(index, false);
        assert_eq!(bits.get(index), Some(false));
        bits.set(index, true);
        assert_eq!(bits.get(index), Some(true));

        // Clear
        bits.set(index, true);
        bits.clear(index);
        assert_eq!(bits.get(index), None);

        // Clear with none
        bits.set(index, true);
        bits.set_option(index, None);
        assert_eq!(bits.get(index), None);

        // Clear false and check last
        bits.set(index, false);
        bits.clear(index);
        assert_eq!(bits.get(index), None);
        assert!(!bits.get_last(index));

        // Clear true and check last
        bits.set(index, true);
        bits.clear(index);
        assert_eq!(bits.get(index), None);
        assert!(bits.get_last(index));
    }
}
