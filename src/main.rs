mod bt;
mod build;
mod gathering;
mod gms;
mod grid;
mod reservation;

use bt::*;
use build::*;
use gathering::*;
use gms::*;
use grid::*;
use reservation::*;
use rsbwapi::*;

pub struct MyModule {
    build_pool: Build,
    build_hatchery: Build,
}

pub struct Frame<'a> {
    game: &'a Game<'a>,
    available_gms: GMS,
    my_units: Vec<Unit<'a>>,
    available_units: Vec<Unit<'a>>,
    incomplete_units: Vec<Unit<'a>>,
}

trait SupplyCounter {
    fn get_provided_supply(&self) -> i32;
}

impl SupplyCounter for Vec<Unit<'_>> {
    fn get_provided_supply(&self) -> i32 {
        self.iter().fold(0, |acc, u| {
            acc + u.get_type().supply_provided() + u.get_build_type().supply_provided()
        })
    }
}

impl AiModule for MyModule {
    fn on_start(&mut self, game: &Game) {}

    fn on_unit_create(&mut self, _game: &Game, unit: Unit) {}

    fn on_unit_destroy(&mut self, _game: &Game, unit: Unit) {}

    fn on_frame(&mut self, game: &Game) {
        if game.get_frame_count() % 2 != 0 {
            return;
        }
        let self_ = game.self_().unwrap();
        let units = game.get_all_units();
        let my_units = self_.get_units();
        let mut frame = Frame {
            game,
            available_gms: GMS {
                minerals: self_.minerals(),
                gas: self_.gas(),
                supply: self_.supply_total() - self_.supply_used(),
            },
            my_units: my_units.clone(),
            available_units: my_units.clone(),
            incomplete_units: my_units
                .iter()
                .filter(|u| !u.is_completed())
                .cloned()
                .collect(),
        };

        let self_ = game.self_().unwrap();
        if self_.supply_used() + 12
            > self_.supply_total() + frame.incomplete_units.get_provided_supply()
        {
            if let Some(larva) = my_units
                .iter()
                .find(|u| u.get_type() == UnitType::Zerg_Larva)
            {
                larva.train(UnitType::Zerg_Overlord).ok();
            }
        } else if let Some(u) = units.iter().find(|u| u.get_type() == UnitType::Zerg_Larva) {
            if game
                .can_make(None, UnitType::Zerg_Zergling)
                .unwrap_or(false)
            {
                u.train(UnitType::Zerg_Zergling).ok();
            } else {
                u.train(UnitType::Zerg_Drone).ok();
            }
        }
        self.build_pool.start_build(&mut frame);
        let result = self.build_hatchery.start_build(&mut frame);
        if result == NodeStatus::Success {
            self.build_hatchery = Build::new(UnitType::Zerg_Hatchery);
        }
        Gathering.go(&frame);
        frame.available_units.retain(|u| {
            if !u.get_type().can_attack() || u.get_type().is_worker() {
                true
            } else {
                u.get_closest_unit(
                    |e: &Unit| {
                        e.get_player() == game.enemy().unwrap()
                            && u.can_attack_unit((e, true, true, true)).unwrap_or(false)
                    },
                    None,
                )
                .map(|e| {
                    u.attack(&e).ok();
                    false
                })
                .unwrap_or(true)
            }
        });

        //        game.cmd().leave_game();
    }
}

impl MyModule {
    fn type_name<T>(v: &T) -> &'static str {
        std::any::type_name::<T>()
    }
}

fn main() {
    rsbwapi::start(MyModule {
        build_pool: Build::new(UnitType::Zerg_Spawning_Pool),
        build_hatchery: Build::new(UnitType::Zerg_Hatchery),
    });
}
