use crate::*;

impl MyModule {
    pub fn ensure_upgrade(&mut self, upgrade: UpgradeType, level: i32) {
        if self.game.self_().unwrap().get_upgrade_level(upgrade) < level {
            self.start_upgrade(upgrade);
        }
    }

    pub fn start_upgrade(&mut self, upgrade: UpgradeType) -> Result<(), FailureReason> {
        let self_ = self.game.self_().unwrap();
        if self_.get_upgrade_level(upgrade) + if self_.is_upgrading(upgrade) { 1 } else { 0 }
            == upgrade.max_repeats()
        {
            return Err(FailureReason::misc("Max upgrade reached or being reached"));
        }
        let price = upgrade.price(
            self.game
                .self_()
                .expect("Self must exist")
                .get_upgrade_level(upgrade),
        );
        if !self.tracker.available_gms.checked_sub(price) {
            self.tracker
                .unrealized
                .push(UnrealizedItem::Upgrade(self.tracker.available_gms, upgrade));
            return Err(FailureReason::InsufficientResources);
        }

        let researcher = self
            .tracker
            .available_units
            .iter()
            .filter(|u| u.get_type() == upgrade.what_upgrades())
            .cloned()
            .next()
            .ok_or(FailureReason::misc("No upgrader found"))?;
        self.tracker.reserve_unit(&researcher);
        researcher.upgrade(upgrade);
        Ok(())
    }
}
