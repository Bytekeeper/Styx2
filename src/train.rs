use crate::*;

pub struct TrainParam {
    unit_type: UnitType,
}

impl TrainParam {
    pub fn train(unit_type: UnitType) -> Self {
        assert!(!unit_type.is_building());
        Self { unit_type }
    }
}

impl MyModule {
    pub fn ensure_unit_count(
        &mut self,
        unit_type: UnitType,
        amount: usize,
    ) -> Result<(), FailureReason> {
        for _ in 0..amount.saturating_sub(self.count_pending_or_ready(|ut| ut == unit_type)) {
            self.start_train(TrainParam::train(unit_type))?;
        }
        Ok(())
    }

    pub fn start_train(&mut self, param: TrainParam) -> Result<(), FailureReason> {
        assert!(
            !param.unit_type.is_building(),
            "{:?} cannot be trained",
            param.unit_type
        );
        if !self
            .tracker
            .available_gms
            .checked_sub(param.unit_type.price())
        {
            self.tracker.unrealized.push(UnrealizedItem::UnitType(
                self.tracker.available_gms,
                param.unit_type,
            ));
            return Err(FailureReason::InsufficientResources);
        }

        let trainer = self
            .tracker
            .available_units
            .iter()
            .filter(|u| u.idle() && u.get_type() == param.unit_type.what_builds().0)
            .next()
            .ok_or(FailureReason::misc("No trainer found"))?
            .id();
        let trainer = self.tracker.reserve_unit(trainer);
        trainer.train(param.unit_type);
        Ok(())
    }
}
