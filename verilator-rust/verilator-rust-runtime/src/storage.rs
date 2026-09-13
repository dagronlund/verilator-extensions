use std::fmt::Debug;

use num_bigint::BigUint;

/// Arithmetic operations evaluated modulo the declared RTL width.
#[derive(Clone, Copy)]
pub enum Arithmetic {
    Add,
    Sub,
    Mul,
    Div,
}

/// Backing storage for a two-state bit vector.
///
/// Primitive storage remains `Copy`; `BigUint` supports arbitrarily wide values.
/// Implementations must preserve only the low `width` bits after arithmetic.
pub trait Storage: Clone + Default + Debug + PartialEq + Eq {
    const CAPACITY: usize;
    fn from_u128(value: u128) -> Self;
    fn low_u128(&self) -> u128;
    fn bit(&self, index: usize) -> bool;
    fn set_bit(&mut self, index: usize, value: bool);
    fn mask(&mut self, width: usize);
    fn arithmetic(&self, rhs: &Self, width: usize, operation: Arithmetic) -> Self;
}

fn mask(width: usize) -> u128 {
    u128::MAX
        .checked_shr(128usize.saturating_sub(width) as u32)
        .unwrap_or(0)
}

fn arithmetic(lhs: u128, rhs: u128, width: usize, operation: Arithmetic) -> u128 {
    let result = match operation {
        Arithmetic::Add => lhs.wrapping_add(rhs),
        Arithmetic::Sub => lhs.wrapping_sub(rhs),
        Arithmetic::Mul => lhs.wrapping_mul(rhs),
        Arithmetic::Div => lhs.checked_div(rhs).unwrap_or_else(|| mask(width)),
    };
    result & mask(width)
}

macro_rules! integer_storage {
    ($($ty:ty),*) => {$(
        impl Storage for $ty {
            const CAPACITY: usize = <$ty>::BITS as usize;
            fn from_u128(value: u128) -> Self { value as Self }
            fn low_u128(&self) -> u128 { *self as u128 }
            fn bit(&self, index: usize) -> bool {
                index < Self::CAPACITY && (*self >> index) & 1 != 0
            }
            fn set_bit(&mut self, index: usize, value: bool) {
                if value { *self |= 1 << index; } else { *self &= !(1 << index); }
            }
            fn mask(&mut self, width: usize) { *self &= mask(width) as Self; }
            fn arithmetic(&self, rhs: &Self, width: usize, operation: Arithmetic) -> Self {
                let result = match operation {
                    Arithmetic::Add => self.wrapping_add(*rhs),
                    Arithmetic::Sub => self.wrapping_sub(*rhs),
                    Arithmetic::Mul => self.wrapping_mul(*rhs),
                    Arithmetic::Div => self.checked_div(*rhs).unwrap_or(Self::MAX),
                };
                result & mask(width) as Self
            }
        }
    )*};
}

integer_storage!(u8, u16, u32, u64, u128);

impl Storage for bool {
    const CAPACITY: usize = 1;
    fn from_u128(value: u128) -> Self {
        value & 1 != 0
    }
    fn low_u128(&self) -> u128 {
        u128::from(*self)
    }
    fn bit(&self, index: usize) -> bool {
        index == 0 && *self
    }
    fn set_bit(&mut self, index: usize, value: bool) {
        assert_eq!(index, 0);
        *self = value;
    }
    fn mask(&mut self, width: usize) {
        *self &= width != 0;
    }
    fn arithmetic(&self, rhs: &Self, width: usize, operation: Arithmetic) -> Self {
        arithmetic(u128::from(*self), u128::from(*rhs), width, operation) != 0
    }
}

impl Storage for BigUint {
    const CAPACITY: usize = usize::MAX;
    fn from_u128(value: u128) -> Self {
        value.into()
    }
    fn low_u128(&self) -> u128 {
        let mut words = self.iter_u64_digits();
        u128::from(words.next().unwrap_or(0)) | (u128::from(words.next().unwrap_or(0)) << 64)
    }
    fn bit(&self, index: usize) -> bool {
        self.bit(index as u64)
    }
    fn set_bit(&mut self, index: usize, value: bool) {
        self.set_bit(index as u64, value);
    }
    fn mask(&mut self, width: usize) {
        if self.bits() > width as u64 {
            *self &= (BigUint::from(1u8) << width) - 1u8;
        }
    }
    fn arithmetic(&self, rhs: &Self, width: usize, operation: Arithmetic) -> Self {
        let mut result = match operation {
            Arithmetic::Add => self + rhs,
            Arithmetic::Sub => {
                if self >= rhs {
                    self - rhs
                } else {
                    (BigUint::from(1u8) << width) + self - rhs
                }
            }
            Arithmetic::Mul => self * rhs,
            Arithmetic::Div => {
                if rhs.bits() == 0 {
                    (BigUint::from(1u8) << width) - 1u8
                } else {
                    self / rhs
                }
            }
        };
        Storage::mask(&mut result, width);
        result
    }
}
