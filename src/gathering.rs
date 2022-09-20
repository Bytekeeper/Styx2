use crate::*;

#[derive(Copy, Clone)]
pub struct GatherParams {
    pub required_resources: i32,
    pub max_workers: i32,
}

impl Default for GatherParams {
    fn default() -> Self {
        Self {
            required_resources: 99999,
            max_workers: 99999,
        }
    }
}

impl MyModule {
    pub fn estimate_gas(&self, frames: u32, sub_workers: u32) -> u32 {
        (self
            .units
            .my_completed
            .iter()
            .filter(|it| it.gathering_gas())
            .count() as u32)
            .saturating_sub(sub_workers)
            * 69
            * frames
            / 1000
    }

    pub fn estimate_minerals(&self, frames: u32, sub_workers: u32) -> u32 {
        (self
            .units
            .my_completed
            .iter()
            .filter(|it| it.gathering_minerals())
            .count() as u32)
            .saturating_sub(sub_workers)
            * 47
            * frames
            / 1000
    }

    pub fn estimate_gms(&self, frames: u32, sub_workers: u32) -> Gms {
        Gms {
            minerals: self.estimate_minerals(frames, sub_workers) as i32,
            // TODO: Only subtract workers from miners?
            gas: self.estimate_gas(frames, 0) as i32,
            supply: 0,
        }
    }

    pub fn ensure_gathering_gas(&mut self, gather_params: GatherParams) {
        let GatherParams {
            required_resources: required,
            max_workers,
        } = gather_params;
        let game = &self.game;
        let units = self.units.all();
        let mut remaining_required = ((0.max(required) + 7) / 8) as usize;
        let mut remaining_workers = max_workers as usize;
        for refinery in self
            .units
            .my_completed
            .iter()
            .filter(|m| m.remaining_build_time() < 24 && m.get_type().is_refinery())
        {
            let mut gas_workers: Vec<_> = self
                .tracker
                .available_units
                .iter()
                .cloned()
                .filter(|u| u.get_type().is_worker())
                .collect();
            gas_workers.sort_by_key(|w| {
                (!w.gathering_gas() as i32) * 1000
                    + w.distance_to(refinery)
                    + (w.carrying_minerals() as i32) * 300
            });
            let gas_workers: Vec<_> = gas_workers
                .iter()
                .take(remaining_required.clamp(0, 4.min(remaining_workers)))
                .cloned()
                .collect();
            // TODO: subtract only 2 for depleted
            remaining_required = remaining_required.saturating_sub(gas_workers.len() * 8);
            remaining_workers = remaining_workers.saturating_sub(gas_workers.len());
            for w in gas_workers {
                if (!w.gathering() || !w.carrying()) && w.target().as_ref() != Some(refinery)
                    || !w.gathering_gas()
                {
                    w.gather(refinery).ok();
                }
                self.tracker.reserve_unit(&w);
            }
        }
    }

    pub fn ensure_gathering_minerals(&mut self) {
        let game = &self.game;
        let units = self.units.all();

        let bases: Vec<_> = self
            .units
            .my_completed
            .iter()
            .filter(|it| it.get_type().is_resource_depot() && it.completed())
            .collect();

        let mut minerals: Vec<_> = units
            .iter()
            .filter(|m| {
                m.get_type().is_mineral_field()
                    && m.visible()
                    && bases
                        .iter()
                        .any(|b| b.tile_position().distance(m.tile_position()) < 9.0 * 9.0)
            })
            .collect();
        for (u, &m) in self
            .tracker
            .available_units
            .iter()
            // TODO implement mineral locking
            .filter(|u| u.get_type().is_worker() && !u.gathering_minerals())
            .filter_map(|u| {
                let m = minerals.iter().enumerate().min_by_key(|(_, m)| {
                    m.position().distance_squared(u.position())
                        + if m.being_gathered() { 90 } else { 0 }
                });
                if let Some((i, &m)) = m {
                    minerals.swap_remove(i);
                    Some((u, m))
                } else {
                    None
                }
            })
        {
            if (!u.gathering() || !u.carrying()) && u.target().as_ref() != Some(m) {
                u.gather(m).ok();
            }
        }
        let workers: Vec<_> = self
            .tracker
            .available_units
            .iter()
            .filter(|w| w.get_type().is_worker())
            .map(|it| it.id())
            .collect();
        for w in workers {
            self.tracker.reserve_unit(w);
        }
    }
}
