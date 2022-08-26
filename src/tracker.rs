use crate::gms::*;
use crate::sunit::*;
use crate::{UnitId, UnitType, UpgradeType};

#[derive(Debug)]
pub enum UnrealizedItem {
    UnitType(Gms, UnitType),
    Upgrade(Gms, UpgradeType),
}

#[derive(Debug, Default)]
pub struct Tracker {
    pub unrealized: Vec<UnrealizedItem>,
    pub available_units: Vec<SUnit>,
    pub available_gms: Gms,
}

impl Tracker {
    pub fn reserve_unit(&mut self, to_id: impl Into<UnitId>) -> SUnit {
        let id = to_id.into();
        let i = self
            .available_units
            .iter()
            .position(|u| u.id() == id)
            .expect("Unit to be available");
        self.available_units.swap_remove(i)
    }
}
