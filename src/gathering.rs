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
    pub fn estimate_gas(&self, frames: i32, sub_workers: i32) -> i32 {
        (self
            .units
            .my_completed
            .iter()
            .filter(|it| it.gathering_gas())
            .count() as i32)
            .saturating_sub(sub_workers)
            * 69
            * frames
            / 1000
    }

    pub fn estimate_minerals(&self, frames: i32, sub_workers: i32) -> i32 {
        (self
            .units
            .my_completed
            .iter()
            .filter(|it| it.gathering_minerals())
            .count() as i32)
            .saturating_sub(sub_workers)
            * 47
            * frames
            / 1000
    }

    pub fn estimate_gms(&self, frames: i32, sub_workers: i32) -> Gms {
        Gms {
            minerals: self.estimate_minerals(frames, sub_workers),
            // TODO: Only subtract workers from miners?
            gas: self.estimate_gas(frames, 0),
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
                // TODO decide 3 or 4 workers based on base <-> refinery distance
                .take(remaining_required.clamp(0, 3.min(remaining_workers)))
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
            .filter(|m| {
                m.get_type().is_mineral_field()
                    && m.visible()
                    && bases
                        .iter()
                        .any(|b| b.tile_position().distance(m.tile_position()) < 9.0 * 9.0)
            })
            .collect();
        let mut miners: Vec<_> = self
            .tracker
            .available_units
            .iter()
            .filter(|u| u.get_type().is_worker())
            .cloned()
            .collect();
        for w in miners.iter() {
            self.tracker.reserve_unit(w);
        }
        miners.retain(|w| {
            if matches!(w.get_order(), Order::MiningMinerals) {
                // let Some(mineral_index) = minerals
                //         .iter()
                //         .position(|m| Some(*m) == w.get_order_target().as_ref())
                //     else {
                //         cvis().log(|| "Worker target mineral not found");
                //         return true;
                //     };
                // let mineral = minerals.swap_remove(mineral_index);
                // if DRAW_GATHERING_TARGET {
                //     cvis().draw_line(
                //         w.position().x,
                //         w.position().y,
                //         mineral.position().x,
                //         mineral.position().y,
                //         Color::Green,
                //     );
                //     // cvis().draw_text(
                //     //     w.position().x,
                //     //     w.position().y,
                //     //     format!("O:{:?}/{:?}", w.get_order(), w.get_secondary_order()),
                //     // );
                // }
                return false;
            } else if w.get_order() == Order::ReturnMinerals {
                return false;
            }
            return true;
        });

        while !miners.is_empty() {
            let miner_mineral = miners
                .iter()
                .enumerate()
                .filter_map(|(i, u)| {
                    minerals
                        .iter()
                        .enumerate()
                        .map(|(j, m)| {
                            (
                                j,
                                m,
                                self.estimate_frames_to(u, m.position())
                                    + m.remaining_mining_frames()
                                    + if u.get_order_target().as_ref() == Some(m) {
                                        0
                                    } else {
                                        12
                                    },
                            )
                        })
                        .min_by_key(|(_, m, d)| *d)
                        .map(|(j, m, d)| (i, u, j, m, d))
                })
                .min_by_key(|(.., d)| *d);
            if let Some((i, w, j, m, _)) = miner_mineral {
                if DRAW_GATHERING_TARGET {
                    cvis().draw_line(
                        w.position().x,
                        w.position().y,
                        m.position().x,
                        m.position().y,
                        Color::Green,
                    );
                    // cvis().draw_text(
                    //     w.position().x,
                    //     w.position().y,
                    //     format!("O:{:?}/{:?}", w.get_order(), w.get_secondary_order()),
                    // );
                }
                w.gather(m).ok();
                miners.swap_remove(i);
                // We might want multiple workers on one mineral
                if miners.len() <= minerals.len() {
                    minerals.swap_remove(j);
                }
            } else {
                // TODO No minerals? Make workers attack as well I guess?
                break;
            }
        }
    }
}
