pub mod storage;
#[cfg(test)]
mod tests;

use std::{cmp::Ordering, fmt};

use crate::storage::{Arithmetic, Storage};

/// An owned, fixed-width two-state bit vector. Bit zero is the least-significant bit.
///
/// `S` is selected by the generator: `bool` for one bit, the smallest fitting
/// unsigned primitive through 128 bits, and `num_bigint::BigUint` beyond that.
/// Unused high bits are always cleared. Primitive-backed vectors implement
/// `Copy`; `BigUint`-backed vectors must be cloned when ownership is shared.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bits<const WIDTH: usize, S: Storage> {
    bits: S,
}

macro_rules! primitive_constants {
    ($($ty:ty),*) => {$(
        impl<const WIDTH: usize> Bits<WIDTH, $ty> {
            /// Constructs a constant from storage, clearing unused high bits.
            pub const fn from_raw(value: $ty) -> Self {
                assert!(WIDTH <= <$ty>::BITS as usize, "storage is too narrow for bit vector");
                let mask = if WIDTH == 0 { 0 } else { <$ty>::MAX >> (<$ty>::BITS as usize - WIDTH) };
                Self { bits: value & mask }
            }
        }
    )*};
}

primitive_constants!(u8, u16, u32, u64, u128);

impl<const WIDTH: usize> Bits<WIDTH, bool> {
    /// Constructs a constant boolean vector (or an empty zero-width vector).
    pub const fn from_raw(value: bool) -> Self {
        assert!(WIDTH <= 1, "storage is too narrow for bit vector");
        Self {
            bits: value && WIDTH != 0,
        }
    }
}

impl<const WIDTH: usize, S: Storage> Bits<WIDTH, S> {
    fn from_fn(mut bit: impl FnMut(usize) -> bool) -> Self {
        let mut result = Self::zero();
        for index in 0..WIDTH {
            result.set_bit(index, bit(index));
        }
        result
    }

    pub fn zero() -> Self {
        assert!(WIDTH <= S::CAPACITY, "storage is too narrow for bit vector");
        Self { bits: S::default() }
    }

    pub fn from_bool(value: bool) -> Self {
        Self::from_u128(u128::from(value))
    }

    pub fn from_u8(value: u8) -> Self {
        Self::from_u128(value.into())
    }

    pub fn from_u16(value: u16) -> Self {
        Self::from_u128(value.into())
    }

    pub fn from_u32(value: u32) -> Self {
        Self::from_u128(value.into())
    }

    pub fn from_u64(value: u64) -> Self {
        Self::from_u128(value.into())
    }

    pub fn from_usize(value: usize) -> Self {
        Self::from_u128(value as u128)
    }

    pub fn from_u128(value: u128) -> Self {
        let mut result = Self::zero();
        result.bits = S::from_u128(value);
        result.bits.mask(WIDTH);
        result
    }

    /// Constructs a vector from little-endian 64-bit words.
    pub fn from_words(words: &[u64]) -> Self {
        let mut result = Self::zero();
        for (word_index, word) in words.into_iter().take(WIDTH.div_ceil(64)).enumerate() {
            for bit in 0..64 {
                result.set_bit(word_index * 64 + bit, (word >> bit) & 1 != 0);
            }
        }
        result
    }

    /// Returns little-endian 64-bit words, with unused high bits cleared.
    pub fn to_words(&self) -> Vec<u64> {
        let mut words = vec![0; WIDTH.div_ceil(64)];
        for bit in 0..WIDTH {
            if self.bit(bit) {
                words[bit / 64] |= 1 << (bit % 64);
            }
        }
        words
    }

    pub fn set_bit(&mut self, index: usize, value: bool) {
        if index < WIDTH {
            self.bits.set_bit(index, value);
        }
    }

    pub fn to_u128(&self) -> u128 {
        self.bits.low_u128()
    }

    pub fn to_usize(&self) -> usize {
        assert!(
            !(usize::BITS as usize..WIDTH).any(|bit| self.bit(bit)),
            "bit vector value exceeds usize"
        );
        self.to_u128() as usize
    }

    pub fn bit(&self, index: usize) -> bool {
        index < WIDTH && self.bits.bit(index)
    }

    pub const fn width() -> usize {
        WIDTH
    }

    pub fn truthy(&self) -> bool {
        (0..WIDTH).any(|bit| self.bit(bit))
    }

    pub fn resize<const OUTPUT: usize, O: Storage>(&self, signed: bool) -> Bits<OUTPUT, O> {
        let fill = signed && self.bit(WIDTH.saturating_sub(1));
        Bits::from_fn(|index| if index < WIDTH { self.bit(index) } else { fill })
    }

    pub fn bit_not(&self) -> Self {
        Self::from_fn(|index| !self.bit(index))
    }

    pub fn negate(&self) -> Self {
        Self::zero().sub(self)
    }

    pub fn reduce_and(&self) -> Bits<1, bool> {
        Bits::from_bool((0..WIDTH).all(|bit| self.bit(bit)))
    }

    pub fn reduce_or(&self) -> Bits<1, bool> {
        Bits::from_bool(self.truthy())
    }

    pub fn reduce_xor(&self) -> Bits<1, bool> {
        Bits::from_bool((0..WIDTH).fold(false, |a, bit| a ^ self.bit(bit)))
    }

    pub fn bitwise<const RHS: usize, const OUTPUT: usize, R: Storage, O: Storage>(
        &self,
        rhs: &Bits<RHS, R>,
        operation: fn(bool, bool) -> bool,
    ) -> Bits<OUTPUT, O> {
        Bits::from_fn(|index| operation(self.bit(index), rhs.bit(index)))
    }

    pub fn add(&self, rhs: &Self) -> Self {
        Self {
            bits: self.bits.arithmetic(&rhs.bits, WIDTH, Arithmetic::Add),
        }
    }

    pub fn sub(&self, rhs: &Self) -> Self {
        Self {
            bits: self.bits.arithmetic(&rhs.bits, WIDTH, Arithmetic::Sub),
        }
    }

    pub fn mul(&self, rhs: &Self) -> Self {
        Self {
            bits: self.bits.arithmetic(&rhs.bits, WIDTH, Arithmetic::Mul),
        }
    }

    fn extended_bit(&self, index: usize, signed: bool) -> bool {
        if index < WIDTH {
            self.bit(index)
        } else {
            signed && self.bit(WIDTH.saturating_sub(1))
        }
    }

    pub fn cmp_unsigned<const RHS: usize, R: Storage>(&self, rhs: &Bits<RHS, R>) -> Ordering {
        for index in (0..WIDTH.max(RHS)).rev() {
            match self.bit(index).cmp(&rhs.bit(index)) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }

    pub fn cmp_signed<const RHS: usize, R: Storage>(&self, rhs: &Bits<RHS, R>) -> Ordering {
        let width = WIDTH.max(RHS);
        let lhs_sign = self.extended_bit(width.saturating_sub(1), true);
        let rhs_sign = rhs.extended_bit(width.saturating_sub(1), true);
        match (lhs_sign, rhs_sign) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                for index in (0..width).rev() {
                    match self
                        .extended_bit(index, true)
                        .cmp(&rhs.extended_bit(index, true))
                    {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                Ordering::Equal
            }
        }
    }

    pub fn div_unsigned(&self, rhs: &Self) -> Self {
        Self {
            bits: self.bits.arithmetic(&rhs.bits, WIDTH, Arithmetic::Div),
        }
    }

    pub fn div_signed(&self, rhs: &Self) -> Self {
        if WIDTH == 0 {
            return Self::zero();
        }
        let lhs_negative = self.bit(WIDTH - 1);
        let rhs_negative = rhs.bit(WIDTH - 1);
        let lhs_magnitude = if lhs_negative {
            self.negate()
        } else {
            self.clone()
        };
        let rhs_magnitude = if rhs_negative {
            rhs.negate()
        } else {
            rhs.clone()
        };
        let quotient = lhs_magnitude.div_unsigned(&rhs_magnitude);
        if lhs_negative ^ rhs_negative {
            quotient.negate()
        } else {
            quotient
        }
    }

    fn shift_left_usize(&self, distance: usize) -> Self {
        Self::from_fn(|index| {
            index
                .checked_sub(distance)
                .is_some_and(|source| self.bit(source))
        })
    }

    pub fn shift<const AMOUNT: usize, const OUTPUT: usize, A: Storage, O: Storage>(
        &self,
        amount: &Bits<AMOUNT, A>,
        right: bool,
        arithmetic: bool,
    ) -> Bits<OUTPUT, O> {
        let fill = arithmetic && self.bit(OUTPUT.saturating_sub(1));
        let distance = amount.to_usize();
        if distance >= OUTPUT {
            return Bits::from_fn(|_| fill);
        }
        let value = self.resize::<OUTPUT, O>(false);
        if right {
            Bits::from_fn(|index| {
                if index + distance < OUTPUT {
                    value.bit(index + distance)
                } else {
                    fill
                }
            })
        } else {
            value.shift_left_usize(distance)
        }
    }

    pub fn concat<const LHS: usize, const RHS: usize, L: Storage, R: Storage>(
        lhs: &Bits<LHS, L>,
        rhs: &Bits<RHS, R>,
    ) -> Self {
        Self::from_fn(|index| {
            if index < RHS {
                rhs.bit(index)
            } else {
                lhs.bit(index - RHS)
            }
        })
    }

    pub fn replicate<const OUTPUT: usize, O: Storage>(&self, count: usize) -> Bits<OUTPUT, O> {
        Bits::from_fn(|index| {
            index < WIDTH.saturating_mul(count) && WIDTH != 0 && self.bit(index % WIDTH)
        })
    }

    pub fn select<const OUTPUT: usize, O: Storage>(&self, offset: usize) -> Bits<OUTPUT, O> {
        Bits::from_fn(|bit| offset.checked_add(bit).is_some_and(|index| self.bit(index)))
    }

    /// Assigns a value to a slice of bits starting at the given offset. If the
    /// assignment would exceed the width of the bit vector, it is skipped.
    pub fn assign_select<const VALUE: usize, V: Storage>(
        &mut self,
        offset: usize,
        width: usize,
        value: &Bits<VALUE, V>,
    ) {
        let Some(end) = offset.checked_add(width) else {
            return;
        };
        if end > WIDTH {
            return;
        }
        for bit in 0..width {
            self.set_bit(offset + bit, value.bit(bit));
        }
    }
}

impl<const WIDTH: usize, S: Storage> Default for Bits<WIDTH, S> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<const WIDTH: usize, S: Storage> fmt::Debug for Bits<WIDTH, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}'b", WIDTH)?;
        for bit in (0..WIDTH).rev() {
            formatter.write_str(if self.bit(bit) { "1" } else { "0" })?;
        }
        Ok(())
    }
}

/// Returns the offset of an array element. `left` and `right` are the declared
/// bounds of the array, which may be in either order. `element_width` is the
/// width of each element in bits.
///
/// Panics if `index` is outside the declared range or the offset cannot be
/// represented by `usize`.
pub fn array_offset(index: usize, left: isize, right: isize, element_width: usize) -> usize {
    let index = isize::try_from(index).expect("array index exceeds isize");
    let position = if left <= right {
        assert!(
            index >= left && index <= right,
            "array index {index} is outside declared range [{left}:{right}]"
        );
        index.abs_diff(left)
    } else {
        assert!(
            index <= left && index >= right,
            "array index {index} is outside declared range [{left}:{right}]"
        );
        left.abs_diff(index)
    };
    position
        .checked_mul(element_width)
        .expect("array offset exceeds usize")
}

pub fn bool_and(lhs: bool, rhs: bool) -> bool {
    lhs && rhs
}
pub fn bool_or(lhs: bool, rhs: bool) -> bool {
    lhs || rhs
}
pub fn bool_xor(lhs: bool, rhs: bool) -> bool {
    lhs ^ rhs
}
