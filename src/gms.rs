use derive_more::{Add, AddAssign, Display, Div, DivAssign, Sub, SubAssign};
use rsbwapi::{Unit, UnitType};
use std::cmp::Ordering;
use std::ops::*;

#[derive(
    Eq, Debug, Copy, Clone, PartialEq, Add, AddAssign, Sub, SubAssign, Div, DivAssign, Display,
)]
#[display(fmt = "m: {}, g: {}, s: {}", minerals, gas, supply)]
pub struct GMS {
    pub minerals: i32,
    pub gas: i32,
    pub supply: i32,
}

impl PartialOrd for GMS {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use std::cmp::Ordering::*;
        if self.minerals == other.minerals && self.gas == other.gas && self.supply == other.supply {
            return Some(Equal);
        }
        if self.minerals <= other.minerals && self.gas <= other.gas && self.supply <= other.supply {
            return Some(Less);
        }
        if self.minerals >= other.minerals && self.gas >= other.gas && self.supply >= other.supply {
            return Some(Greater);
        }
        None
    }
}

impl Div for GMS {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        GMS {
            minerals: self.minerals / rhs.minerals,
            gas: self.gas / rhs.gas,
            supply: self.supply / rhs.supply,
        }
    }
}

impl DivAssign for GMS {
    fn div_assign(&mut self, rhs: Self) {
        self.minerals /= rhs.minerals;
        self.gas /= rhs.gas;
        self.supply /= rhs.supply;
    }
}

impl Mul<i32> for GMS {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        GMS {
            minerals: self.minerals * rhs,
            gas: self.gas * rhs,
            supply: self.supply * rhs,
        }
    }
}

impl Mul<GMS> for i32 {
    type Output = GMS;

    fn mul(self, rhs: GMS) -> Self::Output {
        rhs * self
    }
}

impl MulAssign<i32> for GMS {
    fn mul_assign(&mut self, rhs: i32) {
        self.minerals *= rhs;
        self.gas *= rhs;
        self.supply *= rhs;
    }
}

pub trait Price {
    fn price(&self) -> GMS;
}

impl Price for Unit<'_> {
    fn price(&self) -> GMS {
        self.get_type().price()
    }
}

impl Price for UnitType {
    fn price(&self) -> GMS {
        GMS {
            minerals: self.mineral_price(),
            gas: self.gas_price(),
            supply: self.supply_required(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;
    #[test]
    fn gms_greater() {
        assert!(
            GMS {
                minerals: 100,
                gas: 100,
                supply: 101,
            } > GMS {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
    }
    #[test]
    fn gms_less() {
        assert!(
            GMS {
                minerals: 99,
                gas: 100,
                supply: 100,
            } < GMS {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
    }
    #[test]
    fn gms_equal() {
        assert!(
            GMS {
                minerals: 100,
                gas: 100,
                supply: 100,
            } == GMS {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
    }
}
