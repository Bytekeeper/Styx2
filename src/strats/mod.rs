mod fpm;
use crate::*;
// use anyhow::Result;

impl MyModule {
    pub fn fastest_possible(&mut self) -> anyhow::Result<()> {
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_free_supply(3 * self.count_completed(|ut| ut.is_resource_depot()) as i32);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_building_count(UnitType::Zerg_Hydralisk_Den, 1);
        if self.tracker.available_gms >= UnitType::Zerg_Hatchery.price() {
            self.ensure_building_count(UnitType::Zerg_Hatchery, 12);
        }
        if self.count_pending_or_ready(|ut| ut == UnitType::Zerg_Drone) < 30 {
            self.ensure_ratio((UnitType::Zerg_Drone, 3), (UnitType::Zerg_Hydralisk, 2));
            self.ensure_unit_count(UnitType::Zerg_Drone, 30);
        }
        self.ensure_upgrade(UpgradeType::Grooved_Spines, 1);
        self.ensure_ratio((UnitType::Zerg_Drone, 1), (UnitType::Zerg_Hydralisk, 3));
        self.ensure_gathering_gas(GatherParams::default());
        self.perform_attacking(AttackParams {
            min_army: 12,
            ..Default::default()
        });
        self.perform_scouting(ScoutParams {
            max_scouts: 1,
            ..ScoutParams::default()
        });
        Ok(())
    }
}
