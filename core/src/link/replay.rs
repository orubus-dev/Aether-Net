#[derive(Debug, Clone, Copy)]
pub struct ReplayWindow {
    highest: Option<u64>,
    bitmap: u64,
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayWindow {
    pub const WINDOW_SIZE: u64 = 64;

    pub const fn new() -> Self {
        Self {
            highest: None,
            bitmap: 0,
        }
    }

    pub fn reset(&mut self) {
        self.highest = None;
        self.bitmap = 0;
    }

    pub fn accept(
        &mut self,
        counter: u64,
    ) -> bool {
        let Some(highest) = self.highest else {
            self.highest = Some(counter);
            self.bitmap = 1;
            return true;
        };

        if counter > highest {
            let shift =
                counter - highest;

            if shift >= Self::WINDOW_SIZE {
                self.bitmap = 1;
            } else {
                self.bitmap =
                    (self.bitmap << shift) | 1;
            }

            self.highest = Some(counter);

            return true;
        }

        let distance =
            highest - counter;

        if distance >= Self::WINDOW_SIZE {
            return false;
        }

        let mask =
            1u64 << distance;

        if self.bitmap & mask != 0 {
            return false;
        }

        self.bitmap |= mask;

        true
    }

    pub fn highest(&self) -> Option<u64> {
        self.highest
    }
}
