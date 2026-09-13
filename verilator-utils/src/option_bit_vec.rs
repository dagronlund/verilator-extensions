#[derive(Debug, Clone, PartialEq)]
pub struct OptionBitVec {
    size: usize,
    data: Vec<usize>,
}

impl OptionBitVec {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            // Need two bits for every entry
            data: vec![0; (size * 2).div_ceil(usize::BITS as usize)],
        }
    }

    /// Returns what current bit (or not) is at the index
    pub fn get(&self, index: usize) -> Option<bool> {
        let vec_index = (index * 2) / usize::BITS as usize;
        let word_index = (index * 2) % usize::BITS as usize;
        let pair = self.data[vec_index] >> word_index;
        if (pair & 2) == 2 {
            Some((pair & 1) == 1)
        } else {
            None
        }
    }

    /// Returns what bit was last set (if not set), or currently set, if set,
    /// at the index
    pub fn get_last(&self, index: usize) -> bool {
        let vec_index = (index * 2) / usize::BITS as usize;
        let word_index = (index * 2) % usize::BITS as usize;
        let pair = self.data[vec_index] >> word_index;
        (pair & 1) == 1
    }

    pub fn check(&self, index: usize, value: bool) -> bool {
        if let Some(existing) = self.get(index) {
            existing == value
        } else {
            false
        }
    }

    pub fn set(&mut self, index: usize, value: bool) {
        let vec_index = (index * 2) / usize::BITS as usize;
        let word_index = (index * 2) % usize::BITS as usize;
        let pair = if value { 3 } else { 2 };
        self.data[vec_index] &= !(3 << word_index);
        self.data[vec_index] |= pair << word_index;
    }

    pub fn set_option(&mut self, index: usize, value: Option<bool>) {
        if let Some(value) = value {
            self.set(index, value);
        } else {
            self.clear(index);
        }
    }

    /// Sets the value if not already set, returns None, and returns Some(bool)
    /// indicating if it matched or not if already set
    pub fn set_check(&mut self, index: usize, value: bool) -> Option<bool> {
        if let Some(existing) = self.get(index) {
            self.set(index, value);
            Some(existing == value)
        } else {
            self.set(index, value);
            None
        }
    }

    pub fn clear(&mut self, index: usize) {
        let vec_index = (index * 2) / usize::BITS as usize;
        let word_index = (index * 2) % usize::BITS as usize;
        self.data[vec_index] &= !(2 << word_index);
    }

    pub fn reset(&mut self) {
        for i in 0..self.data.len() {
            self.data[i] = 0;
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<&[Option<bool>]> for OptionBitVec {
    fn from(value: &[Option<bool>]) -> Self {
        let mut vec = Self::new(value.len());
        for (i, v) in value.into_iter().enumerate() {
            if let Some(b) = v {
                vec.set(i, *b);
            }
        }
        vec
    }
}

impl IntoIterator for OptionBitVec {
    type Item = Option<bool>;
    type IntoIter = OptionBitVecIterator;

    fn into_iter(self) -> Self::IntoIter {
        OptionBitVecIterator {
            vec: self,
            index: 0,
        }
    }
}

pub struct OptionBitVecIterator {
    vec: OptionBitVec,
    index: usize,
}

impl Iterator for OptionBitVecIterator {
    type Item = Option<bool>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.vec.len() {
            return None;
        }
        let result = self.vec.get(self.index);
        self.index += 1;
        Some(result)
    }
}

impl<'a> IntoIterator for &'a OptionBitVec {
    type Item = Option<bool>;
    type IntoIter = OptionBitVecRefIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        OptionBitVecRefIterator {
            vec: self,
            index: 0,
        }
    }
}

pub struct OptionBitVecRefIterator<'a> {
    vec: &'a OptionBitVec,
    index: usize,
}

impl<'a> Iterator for OptionBitVecRefIterator<'a> {
    type Item = Option<bool>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.vec.len() {
            return None;
        }
        let result = self.vec.get(self.index);
        self.index += 1;
        Some(result)
    }
}
