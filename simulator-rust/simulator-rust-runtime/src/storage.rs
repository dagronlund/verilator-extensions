use std::fmt::Debug;

use ruint::Uint;

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
/// Primitive and arbitrary-width `Uint` storage are both `Copy`.
/// Implementations must preserve only the low `width` bits after arithmetic.
pub trait Storage: Copy + Default + Debug + PartialEq + Eq {
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

impl<const BITS: usize, const LIMBS: usize> Storage for Uint<BITS, LIMBS> {
    const CAPACITY: usize = BITS;
    fn from_u128(value: u128) -> Self {
        Self::wrapping_from(value)
    }
    fn low_u128(&self) -> u128 {
        let limbs = self.as_limbs();
        u128::from(limbs.first().copied().unwrap_or(0))
            | (u128::from(limbs.get(1).copied().unwrap_or(0)) << 64)
    }
    fn bit(&self, index: usize) -> bool {
        self.bit(index)
    }
    fn set_bit(&mut self, index: usize, value: bool) {
        self.set_bit(index, value);
    }
    fn mask(&mut self, width: usize) {
        if width == 0 {
            *self = Self::ZERO;
        } else if width < BITS {
            *self &= Self::MAX.wrapping_shr(BITS - width);
        }
    }
    fn arithmetic(&self, rhs: &Self, width: usize, operation: Arithmetic) -> Self {
        let mut result = match operation {
            Arithmetic::Add => self.wrapping_add(*rhs),
            Arithmetic::Sub => self.wrapping_sub(*rhs),
            Arithmetic::Mul => self.wrapping_mul(*rhs),
            Arithmetic::Div => self.checked_div(*rhs).unwrap_or(Self::MAX),
        };
        Storage::mask(&mut result, width);
        result
    }
}
