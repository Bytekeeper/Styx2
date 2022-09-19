//
// mod grid;
//
mod boids;
mod build;
mod cherry_vis;
mod combat_sim;
mod gathering;
mod gms;
mod micro;
mod sbase;
mod scouting;
mod splayer;
mod squad;
mod sunit;
mod targeting;
mod tracker;
mod train;
mod upgrade;

use cherry_vis::*;
use gathering::*;
use gms::*;
use rsbwapi::sma::*;
pub use rsbwapi::*;
use sbase::*;
use scouting::*;
use splayer::*;
use squad::*;
use std::borrow::Cow;
pub use sunit::*;
use targeting::*;
use tracker::*;
use train::*;
use upgrade::*;

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
    pub players: Players,
    pub tracker: Tracker,
    pub map: Map,
}

impl MyModule {
    pub fn base_near(&self, position: Position) -> Option<SBase> {
        // TODO
        None
    }

    pub fn is_in_narrow_choke(&self, tp: TilePosition) -> bool {
        // TODO
        false
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
            || self
                .tracker
                .unrealized
                .iter()
                .any(|u| matches!(u, UnrealizedItem::UnitType(_, ut) if check(*ut)))
    }

    pub fn count_pending_or_ready(&self, check: impl Fn(UnitType) -> bool) -> usize {
        self.units
            .mine_all
            .iter()
            .filter(|u| check(u.build_type()) || check(u.get_type()))
            .count()
            + self
                .tracker
                .unrealized
                .iter()
                .filter(|u| matches!(u, UnrealizedItem::UnitType(_, ut) if check(*ut)))
                .count()
    }

    fn opening_styx(&mut self) -> anyhow::Result<()> {
        if self.count_pending_or_ready(|ut| ut.is_successor_of(UnitType::Zerg_Hatchery)) < 2 {
            self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        }
        // Usually, if we lost a drone its game over anyways
        self.ensure_unit_count(UnitType::Zerg_Drone, 7);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 3);
        self.ensure_building_count(UnitType::Zerg_Hatchery, 2);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 7);
        self.ensure_upgrade(UpgradeType::Metabolic_Boost, 1);
        self.ensure_free_supply(2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 50);
        self.ensure_gathering_gas(GatherParams {
            required_resources: -self.tracker.available_gms.gas,
            ..Default::default()
        });

        if !self.units.enemy.iter().any(|e| e.get_type().is_building()) {
            self.scout(ScoutParams::default());
            let base = self
                .units
                .my_completed
                .iter()
                .filter(|u| u.get_type().is_resource_depot())
                .next() // TODO What if we lost our depot?
                .unwrap();
            let target = self
                .units
                .enemy
                .iter()
                .filter(|u| u.get_type().is_building())
                .min_by_key(|u| self.map.get_path(u.position(), base.position()).1)
                .map(|u| u.position());

            if let Some(target) = target {
                let attackers: Vec<_> = self
                    .tracker
                    .available_units
                    .iter()
                    .filter(|u| {
                        u.get_type().can_attack()
                            && u.get_type().can_move()
                            && !u.get_type().is_worker()
                    })
                    .cloned()
                    .collect();
                if !attackers.is_empty() {
                    self.tracker
                        .available_units
                        .retain(|it| !attackers.contains(it));
                    Squad {
                        target,
                        units: attackers,
                    }
                    .update(self);
                }
            }
        } else {
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
            let target = self
                .units
                .enemy
                .iter()
                .filter(|u| u.get_type().is_building())
                .min_by_key(|u| self.map.get_path(base.position(), u.position()).1)
                .map(|u| u.position())
                .unwrap();
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
                    u.get_type().can_attack()
                        && u.get_type().can_move()
                        && !u.get_type().is_worker()
                })
                .cloned()
                .collect();
            if !attackers.is_empty() {
                Squad {
                    target,
                    units: attackers,
                }
                .update(self);
            }
        }

        Ok(())
    }

    fn opening_10hatch(&mut self) -> anyhow::Result<()> {
        let supply = self.game.self_().unwrap().supply_used() / 2;
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.do_extractor_trick();

        unimplemented!();
        Ok(())
    }

    fn opening_9poolspire(&mut self) -> anyhow::Result<()> {
        self.ensure_unit_count(UnitType::Zerg_Drone, 9);
        self.ensure_building_count(UnitType::Zerg_Spawning_Pool, 1);
        self.ensure_building_count(UnitType::Zerg_Extractor, 1);
        self.ensure_unit_count(UnitType::Zerg_Overlord, 2);
        self.ensure_unit_count(UnitType::Zerg_Zergling, 3);
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
        ((
            if unit.flying() {
                unit.position().distance(target)
            } else {
                self.map.get_path(unit.position(), target).1 as f64
            } + 48.0
            // Some small buffer for acceleration/deceleration and obstacle avoidance
        ) / unit.get_type().top_speed()) as u32
            + 24
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
        println!(
            "{} {}",
            game.get_latency_frames(),
            game.get_remaining_latency_frames()
        );
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

            // Unstick
            for u in &self.units.my_completed {
                u.unstick();
            }

            // self.opening_13_pool_muta();
            self.opening_styx();
            // self.opening_10hatch();
            // self.opening_9poolspire();
            self.ensure_gathering_minerals();
            // dbg!("CP: {}", self.map.choke_points.len());
            for cp in &self.map.choke_points {
                for wp in &cp.walk_positions {
                    let p = wp.to_position();
                    // CVIS.lock().unwrap().draw_circle(p.x, p.y, 4, Color::Yellow);
                    game.draw_circle_map(p, 4, Color::Blue, false);
                }
            }
            Ok(())
        })()
        .unwrap();
    }
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    rsbwapi::start(|game| MyModule {
        game: game.clone(),
        units: Default::default(),
        players: Default::default(),
        tracker: Tracker::default(),
        map: Map::new(game),
        // game_count: 0,
        // units: Units::new(&game),
        // grids: Grids::new(),
    });
}
