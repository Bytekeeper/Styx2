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
    // 2 Lings count as one unit here!
    pub fn ensure_unit_count(
        &mut self,
        unit_type: UnitType,
        amount: usize,
    ) -> Result<(), FailureReason> {
        let units_per_egg = 1 + unit_type.is_two_units_in_one_egg() as usize;
        for _ in 0..(units_per_egg / 2
            + amount.saturating_sub(self.count_pending_or_ready(|ut| ut == unit_type)))
            / units_per_egg
        {
            self.start_train(TrainParam::train(unit_type))?;
        }
        Ok(())
    }

    pub fn pump(&mut self, unit_type: UnitType) -> Result<(), FailureReason> {
        let trainers = self.count_completed(|ut| ut == unit_type.what_builds().0);
        for _ in 0..trainers {
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
        assert!(
            param.unit_type.price().gas <= 0.max(self.tracker.available_gms.gas)
                || self.has_pending_or_ready(|ut| ut.is_refinery()),
            "Not enough gas to build {:?}, and no refinery planned or built!",
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
        trainer
            .train(param.unit_type)
            .map(|_| ())
            .map_err(|code| FailureReason::Bwapi(code))
    }
}
