use derive_more::{Add, AddAssign, Display, Div, DivAssign, Sub, SubAssign, Sum};
use rsbwapi::{Unit, UnitType, UpgradeType};
use std::cmp::Ordering;
use std::ops::*;

#[derive(
    Default,
    Eq,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Div,
    DivAssign,
    Display,
    Sum,
)]
#[display(fmt = "m: {}, g: {}, s: {}", minerals, gas, supply)]
pub struct Gms {
    pub minerals: i32,
    pub gas: i32,
    pub supply: i32,
}

impl Gms {
    pub fn checked_sub(&mut self, rhs: Gms) -> bool {
        if rhs <= *self {
            *self -= rhs;
            true
        } else {
            *self -= rhs;
            false
        }
    }
}

fn le_or_0(a: i32, b: i32) -> bool {
    a <= 0 || a <= b
}

impl PartialOrd for Gms {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use std::cmp::Ordering::*;
        if self.minerals == other.minerals && self.gas == other.gas && self.supply == other.supply {
            return Some(Equal);
        }
        if le_or_0(self.minerals, other.minerals)
            && le_or_0(self.gas, other.gas)
            && le_or_0(self.supply, other.supply)
        {
            return Some(Less);
        }
        if le_or_0(other.minerals, self.minerals)
            && le_or_0(other.gas, self.gas)
            && le_or_0(other.supply, self.supply)
        {
            return Some(Greater);
        }
        None
    }
}

impl Div for Gms {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            minerals: self.minerals / rhs.minerals,
            gas: self.gas / rhs.gas,
            supply: self.supply / rhs.supply,
        }
    }
}

impl DivAssign for Gms {
    fn div_assign(&mut self, rhs: Self) {
        self.minerals /= rhs.minerals;
        self.gas /= rhs.gas;
        self.supply /= rhs.supply;
    }
}

impl Mul<i32> for Gms {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        Self {
            minerals: self.minerals * rhs,
            gas: self.gas * rhs,
            supply: self.supply * rhs,
        }
    }
}

impl Mul<Gms> for i32 {
    type Output = Gms;

    fn mul(self, rhs: Gms) -> Self::Output {
        rhs * self
    }
}

impl MulAssign<i32> for Gms {
    fn mul_assign(&mut self, rhs: i32) {
        self.minerals *= rhs;
        self.gas *= rhs;
        self.supply *= rhs;
    }
}

pub trait Price {
    fn price(&self) -> Gms;
}

impl Price for Unit {
    fn price(&self) -> Gms {
        self.get_type().price()
    }
}

impl Price for UnitType {
    fn price(&self) -> Gms {
        Gms {
            minerals: self.mineral_price(),
            gas: self.gas_price(),
            supply: self.supply_required(),
        }
    }
}

pub trait LeveledPrice {
    fn price(&self, level: i32) -> Gms;
}

impl LeveledPrice for UpgradeType {
    fn price(&self, level: i32) -> Gms {
        Gms {
            minerals: self.mineral_price(level),
            gas: self.gas_price(level),
            supply: 0,
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
            Gms {
                minerals: 100,
                gas: 100,
                supply: 101,
            } > Gms {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
        assert!(
            Gms {
                minerals: 50,
                gas: 0,
                supply: 8,
            } > Gms {
                minerals: -266,
                gas: 0,
                supply: 8,
            }
        );
        assert!(
            !(Gms {
                minerals: 50,
                gas: -10,
                supply: 0,
            } > Gms {
                minerals: -266,
                gas: 0,
                supply: 8,
            })
        );
        assert!(
            Gms {
                minerals: 150,
                gas: -10,
                supply: 2,
            } > Gms {
                minerals: 50,
                gas: 0,
                supply: 2,
            }
        );
    }
    #[test]
    fn gms_less() {
        assert!(
            !(Gms {
                minerals: 50,
                gas: 0,
                supply: 0,
            } < Gms {
                minerals: -400,
                gas: 0,
                supply: 100,
            }),
        );
        assert!(
            Gms {
                minerals: 99,
                gas: 100,
                supply: 100,
            } < Gms {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
        assert!(
            Gms {
                minerals: -266,
                gas: 0,
                supply: 8,
            } < Gms {
                minerals: 50,
                gas: 0,
                supply: 8,
            },
        );
        assert!(
            Gms {
                minerals: 50,
                gas: 0,
                supply: 0,
            } <= Gms {
                minerals: 50,
                gas: -50,
                supply: 0,
            },
        );
    }
    #[test]
    fn gms_equal() {
        assert!(
            Gms {
                minerals: 100,
                gas: 100,
                supply: 100,
            } == Gms {
                minerals: 100,
                gas: 100,
                supply: 100,
            },
        );
    }
}
