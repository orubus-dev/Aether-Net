pub const DEFAULT_REPLAY_WINDOW: u64 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayError {
    Stale,
    Duplicate,
}

/// Allocation-free sliding replay window for monotonically numbered packets.
#[derive(Debug, Clone, Copy)]
pub struct ReplayWindow {
    highest: Option<u64>,
    bitmap: u128,
    width: u64,
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self {
            highest: None,
            bitmap: 0,
            width: DEFAULT_REPLAY_WINDOW,
        }
    }

    pub fn with_width(width: u64) -> Self {
        let width = width.clamp(1, 128);
        Self {
            highest: None,
            bitmap: 0,
            width,
        }
    }

    pub fn highest(&self) -> Option<u64> {
        self.highest
    }

    pub fn width(&self) -> u64 {
        self.width
    }

    /// Checks and records a sequence number.
    ///
    /// Sequence numbers are accepted once. Numbers too old for the window
    /// are rejected; future numbers advance the window without allocation.
    pub fn accept(&mut self, sequence: u64) -> Result<(), ReplayError> {
        match self.highest {
            None => {
                self.highest = Some(sequence);
                self.bitmap = 1;
                Ok(())
            }
            Some(highest) if sequence > highest => {
                let shift = sequence - highest;
                if shift >= 128 {
                    self.bitmap = 1;
                } else {
                    self.bitmap = (self.bitmap << shift) | 1;
                    let mask = if self.width == 128 {
                        u128::MAX
                    } else {
                        (1u128 << self.width) - 1
                    };
                    self.bitmap &= mask;
                }
                self.highest = Some(sequence);
                Ok(())
            }
            Some(highest) => {
                let distance = highest - sequence;
                if distance >= self.width {
                    return Err(ReplayError::Stale);
                }
                let bit = 1u128 << distance;
                if self.bitmap & bit != 0 {
                    return Err(ReplayError::Duplicate);
                }
                self.bitmap |= bit;
                Ok(())
            }
        }
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_once_and_rejects_duplicate() {
        let mut window = ReplayWindow::new();
        assert!(window.accept(1).is_ok());
        assert_eq!(window.accept(1), Err(ReplayError::Duplicate));
    }

    #[test]
    fn accepts_reordering_inside_window() {
        let mut window = ReplayWindow::with_width(8);
        assert!(window.accept(10).is_ok());
        assert!(window.accept(8).is_ok());
        assert!(window.accept(9).is_ok());
        assert_eq!(window.accept(8), Err(ReplayError::Duplicate));
    }

    #[test]
    fn rejects_stale_sequences() {
        let mut window = ReplayWindow::with_width(4);
        assert!(window.accept(10).is_ok());
        assert!(window.accept(14).is_ok());
        assert_eq!(window.accept(10), Err(ReplayError::Stale));
    }
}
