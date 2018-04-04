/// `Eos` contains information abount the position of the end of a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Eos(Option<u64>);
impl Eos {
    /// Makes a new `Eos` instance.
    pub fn new(is_eos: bool) -> Self {
        if is_eos {
            Eos(Some(0))
        } else {
            Eos(None)
        }
    }

    /// TODO: doc
    pub fn with_remaining_bytes(n: u64) -> Self {
        Eos(Some(n))
    }

    /// Returns `true` if the target stream has reached to the end, otherwise `false`.
    pub fn is_eos(&self) -> bool {
        self.0 == Some(0)
    }

    /// Returns `true` if the length of the target stream is known, otherwise `false`.
    pub fn is_finite(&self) -> bool {
        self.0.is_some()
    }

    /// Returns the number of bytes remaining in the target stream.
    ///
    /// If it is unknown, `None` will be returned.
    pub fn remaining_bytes(&self) -> Option<u64> {
        self.0
    }
}
