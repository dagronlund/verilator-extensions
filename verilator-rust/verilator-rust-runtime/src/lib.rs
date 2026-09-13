#[cfg(test)]
mod tests;

/// An owned, fixed-width two-state bit vector. Bit zero is the least-significant bit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bits<const WIDTH: usize> {
    bits: [bool; WIDTH],
}

impl<const WIDTH: usize> Bits<WIDTH> {
    fn new(bits: [bool; WIDTH]) -> Self {
        Self { bits }
    }

    pub fn zero() -> Self {
        Self::new([false; WIDTH])
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
        Self::new(std::array::from_fn(|bit| {
            bit < 128 && ((value >> bit) & 1) != 0
        }))
    }

    /// Constructs a vector from little-endian 64-bit words.
    pub fn from_words(words: &[u64]) -> Self {
        Self::new(std::array::from_fn(|bit| {
            words
                .get(bit / 64)
                .is_some_and(|word| ((word >> (bit % 64)) & 1) != 0)
        }))
    }

    /// Returns little-endian 64-bit words, with unused high bits cleared.
    pub fn to_words(&self) -> Vec<u64> {
        let mut words = vec![0; WIDTH.div_ceil(64)];
        for bit in 0..WIDTH {
            if self.bits[bit] {
                words[bit / 64] |= 1 << (bit % 64);
            }
        }
        words
    }

    pub fn set_bit(&mut self, index: usize, value: bool) {
        if let Some(bit) = self.bits.get_mut(index) {
            *bit = value;
        }
    }

    pub fn to_u128(&self) -> u128 {
        (&self.bits)
            .into_iter()
            .take(128)
            .enumerate()
            .fold(0, |value, (bit, set)| value | (u128::from(*set) << bit))
    }

    pub fn to_usize(&self) -> usize {
        assert!(
            !(&self.bits)
                .into_iter()
                .skip(usize::BITS as usize)
                .any(|bit| *bit),
            "bit vector value exceeds usize"
        );
        self.to_u128() as usize
    }

    pub fn bit(&self, index: usize) -> bool {
        self.bits.get(index).copied().unwrap_or(false)
    }

    pub const fn width() -> usize {
        WIDTH
    }

    pub fn truthy(&self) -> bool {
        (&self.bits).into_iter().any(|bit| *bit)
    }

    pub fn resize<const OUTPUT: usize>(&self, signed: bool) -> Bits<OUTPUT> {
        let fill = signed && self.bits.last().copied().unwrap_or(false);
        Bits::new(std::array::from_fn(|index| {
            self.bits.get(index).copied().unwrap_or(fill)
        }))
    }

    pub fn bit_not(&self) -> Self {
        Self::new(std::array::from_fn(|index| !self.bits[index]))
    }

    pub fn negate(&self) -> Self {
        Self::zero().sub(self)
    }

    pub fn reduce_and(&self) -> Bits<1> {
        Bits::from_bool((&self.bits).into_iter().all(|bit| *bit))
    }

    pub fn reduce_or(&self) -> Bits<1> {
        Bits::from_bool(self.truthy())
    }

    pub fn reduce_xor(&self) -> Bits<1> {
        Bits::from_bool((&self.bits).into_iter().fold(false, |a, b| a ^ b))
    }

    pub fn bitwise<const RHS: usize, const OUTPUT: usize>(
        &self,
        rhs: &Bits<RHS>,
        operation: fn(bool, bool) -> bool,
    ) -> Bits<OUTPUT> {
        Bits::new(std::array::from_fn(|index| {
            operation(self.bit(index), rhs.bit(index))
        }))
    }

    pub fn add(&self, rhs: &Self) -> Self {
        let mut carry = false;
        let bits = std::array::from_fn(|index| {
            let lhs = self.bit(index);
            let rhs = rhs.bit(index);
            let result = lhs ^ rhs ^ carry;
            carry = (lhs && rhs) || (carry && (lhs || rhs));
            result
        });
        Self::new(bits)
    }

    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.bit_not().add(&Self::from_u8(1)))
    }

    pub fn mul(&self, rhs: &Self) -> Self {
        let mut result = Self::zero();
        for shift in 0..WIDTH {
            if rhs.bit(shift) {
                result = result.add(&self.shift_left_usize(shift));
            }
        }
        result
    }

    fn extended_bit(&self, index: usize, signed: bool) -> bool {
        if index < WIDTH {
            self.bit(index)
        } else {
            signed && self.bit(WIDTH.saturating_sub(1))
        }
    }

    pub fn cmp_unsigned<const RHS: usize>(&self, rhs: &Bits<RHS>) -> std::cmp::Ordering {
        for index in (0..WIDTH.max(RHS)).rev() {
            match self.bit(index).cmp(&rhs.bit(index)) {
                std::cmp::Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        std::cmp::Ordering::Equal
    }

    pub fn cmp_signed<const RHS: usize>(&self, rhs: &Bits<RHS>) -> std::cmp::Ordering {
        let width = WIDTH.max(RHS);
        let lhs_sign = self.extended_bit(width.saturating_sub(1), true);
        let rhs_sign = rhs.extended_bit(width.saturating_sub(1), true);
        match (lhs_sign, rhs_sign) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                for index in (0..width).rev() {
                    match self
                        .extended_bit(index, true)
                        .cmp(&rhs.extended_bit(index, true))
                    {
                        std::cmp::Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                std::cmp::Ordering::Equal
            }
        }
    }

    pub fn div_unsigned(&self, rhs: &Self) -> Self {
        let mut quotient = Self::zero();
        let mut remainder = vec![false; WIDTH + 1];
        let mut divisor = Vec::with_capacity(WIDTH + 1);
        divisor.extend_from_slice(&rhs.bits);
        divisor.push(false);
        for index in (0..WIDTH).rev() {
            remainder.rotate_right(1);
            remainder[0] = self.bit(index);
            if compare_bits(&remainder, &divisor) != std::cmp::Ordering::Less {
                subtract_bits(&mut remainder, &divisor);
                quotient.bits[index] = true;
            }
        }
        quotient
    }

    pub fn div_signed(&self, rhs: &Self) -> Self {
        if WIDTH == 0 {
            return Self::zero();
        }
        let lhs_negative = self.bit(WIDTH - 1);
        let rhs_negative = rhs.bit(WIDTH - 1);
        let lhs_magnitude = if lhs_negative { self.negate() } else { *self };
        let rhs_magnitude = if rhs_negative { rhs.negate() } else { *rhs };
        let quotient = lhs_magnitude.div_unsigned(&rhs_magnitude);
        if lhs_negative ^ rhs_negative {
            quotient.negate()
        } else {
            quotient
        }
    }

    fn shift_left_usize(&self, distance: usize) -> Self {
        Self::new(std::array::from_fn(|index| {
            index
                .checked_sub(distance)
                .is_some_and(|source| self.bit(source))
        }))
    }

    pub fn shift<const AMOUNT: usize, const OUTPUT: usize>(
        &self,
        amount: &Bits<AMOUNT>,
        right: bool,
        arithmetic: bool,
    ) -> Bits<OUTPUT> {
        let fill = arithmetic && self.bit(OUTPUT.saturating_sub(1));
        let distance = amount.to_usize();
        if distance >= OUTPUT {
            return Bits::new([fill; OUTPUT]);
        }
        let value = self.resize::<OUTPUT>(false);
        if right {
            Bits::new(std::array::from_fn(|index| {
                value.bits.get(index + distance).copied().unwrap_or(fill)
            }))
        } else {
            value.shift_left_usize(distance)
        }
    }

    pub fn concat<const LHS: usize, const RHS: usize>(lhs: &Bits<LHS>, rhs: &Bits<RHS>) -> Self {
        Self::new(std::array::from_fn(|index| {
            if index < RHS {
                rhs.bit(index)
            } else {
                lhs.bit(index - RHS)
            }
        }))
    }

    pub fn replicate<const OUTPUT: usize>(&self, count: usize) -> Bits<OUTPUT> {
        Bits::new(std::array::from_fn(|index| {
            index < WIDTH.saturating_mul(count) && WIDTH != 0 && self.bit(index % WIDTH)
        }))
    }

    pub fn select<const OUTPUT: usize>(&self, offset: usize) -> Bits<OUTPUT> {
        Bits::new(std::array::from_fn(|bit| self.bit(offset + bit)))
    }

    /// Assigns a value to a slice of bits starting at the given offset. If the
    /// assignment would exceed the width of the bit vector, it is skipped.
    pub fn assign_select<const VALUE: usize>(
        &mut self,
        offset: usize,
        width: usize,
        value: &Bits<VALUE>,
    ) {
        let Some(end) = offset.checked_add(width) else {
            return;
        };
        if end > WIDTH {
            return;
        }
        for bit in 0..width {
            self.bits[offset + bit] = value.bit(bit);
        }
    }
}

impl<const WIDTH: usize> Default for Bits<WIDTH> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<const WIDTH: usize> std::fmt::Debug for Bits<WIDTH> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}'b", WIDTH)?;
        for bit in (&self.bits).into_iter().rev() {
            formatter.write_str(if *bit { "1" } else { "0" })?;
        }
        Ok(())
    }
}

fn compare_bits(lhs: &[bool], rhs: &[bool]) -> std::cmp::Ordering {
    for index in (0..lhs.len().max(rhs.len())).rev() {
        match lhs
            .get(index)
            .copied()
            .unwrap_or(false)
            .cmp(&rhs.get(index).copied().unwrap_or(false))
        {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    std::cmp::Ordering::Equal
}

fn subtract_bits(lhs: &mut [bool], rhs: &[bool]) {
    let mut borrow = false;
    for (index, lhs_bit) in lhs.iter_mut().enumerate() {
        let rhs_bit = rhs.get(index).copied().unwrap_or(false);
        let result = *lhs_bit ^ rhs_bit ^ borrow;
        borrow = (!*lhs_bit && (rhs_bit || borrow)) || (rhs_bit && borrow);
        *lhs_bit = result;
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
