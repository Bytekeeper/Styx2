//
mod boids;
mod build;
mod cherry_vis;
mod cluster;
mod combat_sim;
mod config;
mod duration;
mod gathering;
mod gms;
mod grid;
mod micro;
mod sbase;
mod scouting;
mod skirmish;
mod splayer;
mod squad;
mod sunit;
mod targeting;
mod tracker;
mod train;
mod upgrade;

use cherry_vis::*;
use config::*;
use gathering::*;
use gms::*;
use rsbwapi::sma::*;
pub use rsbwapi::*;
use sbase::Bases;
use scouting::*;
use skirmish::*;
use splayer::*;
use squad::*;
use std::borrow::Cow;
pub use sunit::*;
use targeting::*;
use tracker::*;
use train::*;
use upgrade::*;

#[derive(Default)]
pub struct AttackParams {
    aggression_value: i32,
    min_army: usize,
}

#[derive(Debug)]
pub enum FailureReason {
    InsufficientResources,
    Bwapi(Error),
    Misc(Cow<'static, str>),
}

impl FailureReason {
    pub fn misc(reason: impl Into<Cow<'static, str>>) -> FailureReason {
        FailureReason::Misc(reason.into())
    }
}

pub struct MyModule {
    pub game: Game,
    pub units: Units,
    pub bases: Bases,
    pub skirmishes: Skirmishes,
    pub players: Players,
    pub tracker: Tracker,
    pub map: Map,
    pub strat: &'static dyn Fn(&mut MyModule) -> anyhow::Result<()>,
}

impl MyModule {
    // Relative "value" of an agent regarding other agents
    // TODO should be modified base on game state
    pub fn value_of(&self, unit_type: UnitType, _my_unit: bool) -> i32 {
        // Cost
        let mut res = (unit_type.mineral_price() + 3 * unit_type.gas_price() / 2)
            / (1 + unit_type.is_two_units_in_one_egg() as i32);
        assert!(res >= 0);
        res
    }

    // Find "most forward" of our bases
    pub fn forward_base(&self) -> Option<SUnit> {
        // TODO Something is off here, bot builds "very forward" bases sometimes
        self.units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .max_by_key(|b| {
                self.game
                    .get_start_locations()
                    .iter()
                    .map(|l| self.map.get_path(b.position(), l.center()).1)
                    .min()
            })
            .cloned()
    }

    // Find "main base" - for now it's just any base close to a start position
    pub fn main_base(&self) -> Option<SUnit> {
        self.units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .min_by_key(|b| {
                self.game
                    .get_start_locations()
                    .iter()
                    .map(|l| self.map.get_path(b.position(), l.center()).1)
                    .min()
            })
            .cloned()
    }

    pub fn is_in_narrow_choke(&self, tp: TilePosition) -> bool {
        // TODO
        false
    }

    pub fn has_requirements_for(&self, type_: UnitType) -> bool {
        let self_ = self.game.self_().unwrap();

        for it in type_.required_units() {
            if !self_.has_unit_type_requirement(it.0, it.1) {
                return false;
            }
        }
        if type_.required_tech() != TechType::None && !self_.has_researched(type_.required_tech()) {
            return false;
        }
        true
    }

    pub fn furthest_walkable_position(&self, from: Position, to: Position) -> Option<WalkPosition> {
        let to = to.to_walk_position();
        let mut from = from.to_walk_position();
        let mut last = None;
        let dx = (to.x - from.x).abs();
        let dy = -(to.y - from.y).abs();
        let sx = (from.x < to.x) as i32 * 2 - 1;
        let sy = (from.y < to.y) as i32 * 2 - 1;
        let mut err = dx + dy;
        loop {
            if !from.is_valid(&&self.game) || !self.game.is_walkable(from) {
                return last;
            }
            last = Some(from);
            if to == from {
                return last;
            }
            let e2 = 2 * err;
            if e2 > dy {
                err += dy;
                from.x += sx
            }
            if e2 < dx {
                err += dx;
                from.y += sy
            }
        }
    }

    pub fn is_target_reachable_enemy_base(
        &self,
        target_position: Position,
        vanguard: &SUnit,
    ) -> bool {
        // TODO
        true
    }
    pub fn ensure_free_supply(&mut self, amount: i32) {
        let supply_delta = self.get_pending_supply();
        if supply_delta < amount {
            self.start_train(TrainParam::train(UnitType::Zerg_Overlord));
        }
    }

    pub fn get_pending_supply(&mut self) -> i32 {
        self.units
            .mine_all
            .iter()
            .map(|u| {
                let t = u.future_type();
                t.supply_provided() - t.supply_required()
            })
            .sum()
    }

    pub fn has_pending_or_upgraded(&self, upgrade: UpgradeType, level: i32) -> bool {
        let self_ = self.game.self_().unwrap();
        self_.get_upgrade_level(upgrade) == level - if self_.is_upgrading(upgrade) { 1 } else { 0 }
    }

    pub fn has_pending_upgraded_or_planned(&self, upgrade: UpgradeType, level: i32) -> bool {
        let self_ = self.game.self_().unwrap();
        self_.get_upgrade_level(upgrade)
            == level
                - if self_.is_upgrading(upgrade) { 1 } else { 0 }
                - self
                    .tracker
                    .unrealized
                    .iter()
                    .filter(|u| matches!(u, UnrealizedItem::Upgrade(_, ut) if ut == &upgrade))
                    .count() as i32
    }

    pub fn has_pending_or_ready(&self, check: impl Fn(UnitType) -> bool) -> bool {
        self.units
            .mine_all
            .iter()
            .any(|u| check(u.build_type()) || check(u.get_type()))
    }

    pub fn has_pending_ready_or_planned(&self, check: impl Fn(UnitType) -> bool) -> bool {
        self.has_pending_or_ready(&check)
            || self
                .tracker
                .unrealized
                .iter()
                .any(|u| matches!(u, UnrealizedItem::UnitType(_, ut) if check(*ut)))
    }

    pub fn count_completed(&self, check: impl Fn(UnitType) -> bool) -> usize {
        self.units
            .my_completed
            .iter()
            .filter(|it| check(it.get_type()))
            .count()
    }

    pub fn count_pending_or_ready(&self, check: impl Fn(UnitType) -> bool) -> usize {
        let count_check = |t| {
            if check(t) {
                1 + t.is_two_units_in_one_egg() as usize
            } else {
                0
            }
        };
        let result = self
            .units
            .mine_all
            .iter()
            .map(|u| {
                count_check(u.build_type())
                    + if u.completed() {
                        check(u.get_type()) as usize
                    } else {
                        // Lings have a few frames where they are not yet completed and only one of
                        // the two lings will exist for a short period
                        count_check(u.get_type())
                    }
            })
            .sum::<usize>()
            + self
                .tracker
                .unrealized
                .iter()
                .map(|u| match u {
                    UnrealizedItem::UnitType(_, ut) => count_check(*ut),
                    _ => 0,
                })
                .sum::<usize>();
        result
    }

    fn three_hatch_spire(&mut self) -> anyhow::Result<()> {
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Drone, 10);
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 2 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 12);
        }
        self.ensure_base_count(2);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 3 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 13);
        }
        self.ensure_base_count(3);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 6);
        self.ensure_unit_count(UnitType::Zerg_Drone, 13);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 3);
        self.ensure_building_count(UnitType::Zerg_Lair, 1);
        self.ensure_building_count(UnitType::Zerg_Spire, 1);
        self.ensure_building_count(UnitType::Zerg_Hatchery, 4);

        self.ensure_gathering_gas(GatherParams {
            ..Default::default()
        });
        Ok(())
    }

    fn four_pool_aggressive(&mut self) -> anyhow::Result<()> {
        if self.tracker.available_gms.supply >= 0 && self.tracker.available_gms.supply <= 2 {
            self.do_extractor_trick(UnitType::Zerg_Zergling);
        }
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_free_supply(2);
        self.pump(UnitType::Zerg_Zergling);

        self.perform_scouting(ScoutParams {
            max_workers: 0,
            ..ScoutParams::default()
        });
        self.perform_attacking(AttackParams {
            aggression_value: 400,
            ..Default::default()
        });

        Ok(())
    }

    fn five_pool(&mut self) -> anyhow::Result<()> {
        if self.tracker.available_gms.supply >= 0 && self.tracker.available_gms.supply <= 2 {
            self.do_extractor_trick(UnitType::Zerg_Zergling);
        }
        self.ensure_unit_count(UnitType::Zerg_Drone, 5);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_unit_count(UnitType::Zerg_Drone, 6);
        self.ensure_free_supply(2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 100);

        let my_base = self
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next()
            .ok_or(anyhow::anyhow!("Base not found"))?
            .tile_position();
        let scout_target = self.scout_target(my_base);
        if let Some(scout_target) = scout_target {
            self.perform_scouting(ScoutParams {
                max_workers: if self.units.mine_all.iter().any(|it| {
                    it.get_type() == UnitType::Zerg_Spawning_Pool
                        && it.remaining_build_time()
                            < UnitType::Zerg_Zergling.build_time()
                                + (self.map.get_path(my_base.center(), scout_target.center()).1
                                    as f64
                                    / UnitType::Zerg_Drone.top_speed())
                                    as i32
                }) {
                    1
                } else {
                    0
                },
                ..ScoutParams::default()
            });
        }
        self.perform_attacking(AttackParams {
            aggression_value: 400,
            ..Default::default()
        });

        Ok(())
    }

    fn opening_styx(&mut self) -> anyhow::Result<()> {
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 2 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        }
        // Usually, if we lost a drone its game over anyways
        self.ensure_unit_count(UnitType::Zerg_Drone, 7);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 6);
        self.ensure_building_count(UnitType::Zerg_Hatchery, 2);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 14);
        self.ensure_upgrade(UpgradeType::Metabolic_Boost, 1);
        self.ensure_free_supply(2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 100);
        self.ensure_gathering_gas(GatherParams {
            required_resources: -self.tracker.available_gms.gas,
            max_workers: 3,
            ..Default::default()
        });

        self.perform_attacking(AttackParams {
            aggression_value: 50,
            ..Default::default()
        });
        self.perform_scouting(ScoutParams::default());

        Ok(())
    }

    pub fn perform_attacking(&mut self, attack_params: AttackParams) -> anyhow::Result<()> {
        let base = self
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next(); // TODO What if we lost our depot?
        let base = if let Some(base) = base {
            base
        } else {
            anyhow::bail!("No base");
        };
        let Some(target) = self
            .units
            .enemy
            .iter()
            .filter(|u| u.get_type().is_building())
            .min_by_key(|u| self.map.get_path(base.position(), u.position()).1)
            .map(|u| {
                let pos = u.position();
                self.map
                    .bases
                    .iter()
                    .map(|b| b.position.center())
                    .filter(|b| b.distance_squared(pos) < 600 * 600)
                    .next()
                    .unwrap_or(pos)
            })
            .or_else(|| {
                self.units
                    .enemy
                    .iter()
                    .filter(|it| it.get_type().can_move() && it.get_type().can_attack())
                    .min_by_key(|u| {
                            self.estimate_frames_to(u, self.forward_base().unwrap().position())
                    })
                    .map(|u| u.position())
            }) else { anyhow::bail!("No enemies") };

        // let mut x = target;
        // let mut path = self.map.get_path(base.position(), target).0;
        // while let Some(next) = path.pop().map(|it| it.top.center()) {
        //     cvis().draw_line(next.x, next.y, x.x, x.y, Color::Purple);
        //     x = next;
        // }
        let attackers: Vec<_> = self
            .units
            .my_completed
            .iter()
            .filter(|u| {
                u.get_type().can_attack() && u.get_type().can_move() && !u.get_type().is_worker()
            })
            .cloned()
            .collect();
        if !attackers.is_empty() {
            Squad {
                target,
                value_bias: attack_params.aggression_value,
                min_army: attack_params.min_army,
            }
            .execute(self);
        }
        Ok(())
    }

    fn opening_10hatch(&mut self) -> anyhow::Result<()> {
        let supply = self.game.self_().unwrap().supply_used() / 2;
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.do_extractor_trick(UnitType::Zerg_Drone);

        unimplemented!();
        Ok(())
    }

    fn two_hatch_hydra(&mut self) -> anyhow::Result<()> {
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Drone, 10);
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 2 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 12);
        }
        self.ensure_base_count(2);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_unit_count(UnitType::Zerg_Drone, 15);
        self.ensure_building_count(UnitType::Zerg_Hydralisk_Den, 1);
        if self.count_completed(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) >= 2 {
            self.ensure_building_count(
                UnitType::Zerg_Creep_Colony,
                1_usize.min(1_usize.saturating_sub(
                    self.count_pending_or_ready(|ut| ut == UnitType::Zerg_Sunken_Colony),
                )),
            );
        }
        self.ensure_unit_count(UnitType::Zerg_Drone, 16);
        self.ensure_free_supply(5);
        self.ensure_upgrade(UpgradeType::Grooved_Spines, 1);
        self.ensure_building_count(UnitType::Zerg_Sunken_Colony, 1);
        self.ensure_unit_count(UnitType::Zerg_Hydralisk, 12);
        self.ensure_upgrade(UpgradeType::Muscular_Augments, 1);
        self.pump(UnitType::Zerg_Hydralisk);
        self.ensure_building_count(UnitType::Zerg_Evolution_Chamber, 1);
        self.ensure_upgrade(UpgradeType::Zerg_Carapace, 1);
        self.ensure_upgrade(UpgradeType::Zerg_Missile_Attacks, 1);
        self.ensure_gathering_gas(GatherParams {
            max_workers: 0.max(3 - self.game.self_().unwrap().gas() / 200),
            // Researched grooved spines? Full gathering
            required_resources: if self.has_pending_or_upgraded(UpgradeType::Grooved_Spines, 1) {
                999
            } else {
                0.max(UpgradeType::Grooved_Spines.gas_price(1) - self.game.self_().unwrap().gas())
            },
            ..Default::default()
        });
        self.perform_attacking(AttackParams {
            min_army: 12,
            ..Default::default()
        });
        self.perform_scouting(ScoutParams {
            max_workers: self
                .units
                .mine_all
                .iter()
                .any(|ut| ut.get_type() == UnitType::Zerg_Hydralisk)
                as i32,
            max_scouts: 5 - self.units.enemy.iter().any(|u| u.get_type().is_building()) as i32 * 3,
            ..ScoutParams::default()
        });
        Ok(())
    }

    fn three_hatch_zergling(&mut self) -> anyhow::Result<()> {
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 2 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 11);
        }
        self.ensure_base_count(2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 4);
        self.ensure_unit_count(UnitType::Zerg_Drone, 11);
        self.ensure_base_count(3);
        if self.has_pending_or_ready(|ut| ut.is_refinery()) {
            self.ensure_unit_count(UnitType::Zerg_Drone, 13);
        }
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 6);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 3);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 12);
        self.ensure_upgrade(UpgradeType::Metabolic_Boost, 1);
        self.ensure_free_supply(2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 300);

        self.ensure_gathering_gas(GatherParams {
            required_resources: -self.tracker.available_gms.gas,
            max_workers: 3,
            ..Default::default()
        });

        self.perform_attacking(AttackParams {
            min_army: 28,
            ..Default::default()
        });
        self.perform_scouting(ScoutParams {
            max_workers: 0,
            ..ScoutParams::default()
        });
        Ok(())
    }

    fn nine_poolspire(&mut self) -> anyhow::Result<()> {
        todo!();
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 6);
        self.ensure_upgrade(UpgradeType::Metabolic_Boost, 1);
        self.ensure_building_count(UnitType::Zerg_Lair, 1);
        self.ensure_unit_count(UnitType::Zerg_Drone, 17);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 3);
        self.ensure_building_count(UnitType::Zerg_Spire, 1);

        self.ensure_gathering_gas(GatherParams {
            max_workers: 3,
            ..Default::default()
        });
        Ok(())
    }

    pub fn estimate_frames_to(&self, unit: &SUnit, target: Position) -> u32 {
        assert!(
            unit.get_type().top_speed() > 0.0,
            "No! A {:?} really is very very slow!",
            unit.get_type()
        );
        ((if unit.flying() {
            unit.position().distance(target)
        } else {
            self.map.get_path(unit.position(), target).1 as f64
        }) / unit.get_type().top_speed())
        .ceil() as u32
    }
}

trait SupplyCounter {
    fn get_provided_supply(&self) -> i32;
}

impl SupplyCounter for &[Unit] {
    fn get_provided_supply(&self) -> i32 {
        self.iter()
            .fold(0, |acc, u| acc + u.get_build_type().supply_provided())
    }
}

impl AiModule for MyModule {
    fn on_start(&mut self, game: &Game) {
        *CVIS.lock().unwrap() = cherry_vis::implementation::CherryVis::new(game);
        self.map = Map::new(game);
        self.bases = Bases::new(self);

        let strategies: &[&dyn Fn(&mut MyModule) -> anyhow::Result<()>] =
            match self.game.enemy().map(|e| e.get_race()) {
                Some(Race::Protoss) => &[&Self::two_hatch_hydra],
                Some(Race::Terran) => &[&Self::four_pool_aggressive],
                Some(Race::Zerg) => &[&Self::opening_styx, &Self::four_pool_aggressive],
                // Some(Race::Zerg) => &[&Self::three_hatch_zergling],
                _ => &[&Self::two_hatch_hydra],
            };
        let time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut rnd = oorandom::Rand32::new(time);
        self.strat = strategies[rnd.rand_range(0..strategies.len() as u32) as usize];
        // for x in 0..50 {
        //     for y in 0..50 {
        //         cvis().draw_text(
        //             x * 32,
        //             y * 32,
        //             match self.map.get_altitude(WalkPosition::new(x * 4, y * 4)) {
        //                 rsbwapi::sma::Altitude::Walkable(i) => i.to_string(),
        //                 _ => "".to_string(),
        //             },
        //         );
        //     }
        // }
    }

    fn on_end(&mut self, game: &Game, _winner: bool) {
        #[cfg(feature = "cvis")]
        {
            std::fs::create_dir_all("bwapi-data/write/cvis");
            let encoder = serde_json::to_writer(
                zstd::stream::write::Encoder::new(
                    std::fs::File::create("bwapi-data/write/cvis/trace.json").unwrap(),
                    0,
                )
                .unwrap()
                .auto_finish(),
                &*CVIS.lock().unwrap(),
            );
        }
        // println!(
        //     "{:?}",
        //     std::path::Path::new("bwapi-data/write").canonicalize()
        // );
        // let mut file = std::fs::File::create("bwapi-data/write/out.txt").unwrap();
        // let mut encoder = zstd::stream::write::Encoder::new(file, 0).unwrap();
        // let mut out = json::JsonStream::new(&mut encoder);
        // let mut obj = out.start_object().unwrap();
        // let mut fld = obj.start_field("_version").unwrap();
        // fld.value(0);
        // let mut fld = obj.start_field("type_names").unwrap();
        // let mut types = fld.start_object().unwrap();
        // types.end();
        // fld.end();
        // obj.end();
        // // out.write_object_start();
        // // out.write_object_field("_version");
        // // out.write_val(0);
        // // out.write_more();
        // // out.write_object_field("types_names");
        // // out.write_object_end();
        // encoder.finish().unwrap();
        println!(
            "Times in microseconds:\n{}",
            serde_yaml::to_string(game.get_metrics()).unwrap()
        );
    }

    fn on_unit_destroy(&mut self, _game: &Game, unit: Unit) {
        self.units.mark_dead(&unit);
    }

    fn on_frame(&mut self, game: &Game) {
        CVIS.lock().unwrap().set_frame(game.get_frame_count());
        // self.cvis.draw_text(20, 20, "test".to_owned());
        // self.cvis
        // .draw_text_screen(100, 100, "This is a test".to_owned());
        // println!("{:?}", game.get_all_units());
        // if game.get_frame_count() > 3 {
        //     game.leave_game();
        // }
        (move || -> anyhow::Result<()> {
            let me = self.game.self_().unwrap();
            self.players.update(&self.game);
            self.units.update(&self.game, &self.players);
            self.bases.update(&self.game);
            self.skirmishes = Skirmishes::new(self, &self.units.clusters);
            self.tracker.unrealized.clear();
            self.tracker.available_units = self
                .units
                .my_completed
                .iter()
                .filter(|u| u.build_type() == UnitType::None && !u.training())
                .cloned()
                .collect();
            self.tracker.available_gms = Gms {
                minerals: me.minerals(),
                gas: me.gas(),
                supply: me.supply_total() - me.supply_used(),
            };
            self.tracker.available_gms -= self
                .units
                .my_completed
                .iter()
                // Zerg: Workers morph to building and type and build_type will stay the same
                .filter(|u| u.build_type() != u.get_type() && u.build_type().is_building())
                .map(|u| u.build_type().price())
                .sum();
            //     let self_ = game.self_().unwrap();
            //
            for b in self
                .units
                .my_completed
                .iter()
                .filter(|u| u.build_type().is_building())
            {
                let (build_pos, unit_type) = (b.target_position(), b.build_type());
                if let Some(build_pos) = build_pos {
                    CVIS.lock().unwrap().draw_rect(
                        build_pos.x - unit_type.dimension_left(),
                        build_pos.y - unit_type.dimension_up(),
                        build_pos.x + unit_type.dimension_right(),
                        build_pos.y + unit_type.dimension_down(),
                        Color::Purple,
                    );
                }
            }

            // Unstick
            for u in &self.units.my_completed {
                u.unstick();
            }

            for s in self.skirmishes.skirmishes.iter() {
                let c = &s.cluster;
                let mut iter = c.units.iter();
                let head = iter.next().unwrap();
                cvis().draw_text(
                    head.position().x,
                    head.position().y,
                    format!("Flee    : {:?}", s.combat_evaluation.me_fleeing),
                );
                cvis().draw_text(
                    head.position().x,
                    head.position().y + 10,
                    format!("Fight   : {:?}", s.combat_evaluation.both_fighting),
                );
                cvis().draw_text(
                    head.position().x,
                    head.position().y + 20,
                    format!("E-Defend: {:?}", s.combat_evaluation.enemy_defending),
                );
                cvis().draw_text(
                    head.position().x,
                    head.position().y + 30,
                    format!("Eval: {:?}", s.combat_evaluation.to_i32()),
                );
                // cvis().draw_line(
                //     head.position().x - 30,
                //     head.position().y - (30.0 * c.b) as i32,
                //     head.position().x + 30,
                //     head.position().y + (30.0 * c.b) as i32,
                //     Color::White,
                // );
                if DRAW_CLUSTER_CONNECTION {
                    for next in iter {
                        cvis().draw_line(
                            next.position().x,
                            next.position().y,
                            head.position().x,
                            head.position().y,
                            Color::Brown,
                        );
                    }
                }
            }

            // self.opening_13_pool_muta();
            (self.strat)(self);
            // self.opening_styx();
            // self.opening_10hatch();
            // self.opening_9poolspire();

            // Always gather minerals with the remaining drones, can't imagine a situation where
            // this is a bad idea...
            self.ensure_gathering_minerals();
            // for cp in &self.map.choke_points {
            //     for wp in &cp.walk_positions {
            //         let p = wp.to_position();
            //         CVIS.lock().unwrap().draw_circle(p.x, p.y, 4, Color::Yellow);
            //         // game.draw_circle_map(p, 4, Color::Blue, false);
            //     }
            // }
            Ok(())
        })()
        .unwrap();
    }
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    // let guard = pprof::ProfilerGuardBuilder::default()
    //     .frequency(1000)
    //     .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    //     .build()
    //     .unwrap();

    rsbwapi::start(|game| MyModule {
        game: game.clone(),
        bases: Bases::default(),
        units: Default::default(),
        players: Default::default(),
        tracker: Tracker::default(),
        map: Map::new(game),
        skirmishes: Default::default(),
        strat: &MyModule::two_hatch_hydra,
    });
    // if let Ok(report) = guard.report().build() {
    //     let file = std::fs::File::create("flamegraph.svg").unwrap();
    //     report.flamegraph(file).unwrap();
    // };
}
