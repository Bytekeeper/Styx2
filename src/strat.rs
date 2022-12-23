use crate::MyModule;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct StrategyRecord {
    pub enemy: String,
    pub map: String,
    pub strategy: String,
    pub wins: i32,
    pub losses: i32,
}

pub fn save_strategies(records: &[StrategyRecord]) -> anyhow::Result<()> {
    serde_json::to_writer(
        File::create(Path::new("bwapi-data").join("write").join("learned.json"))?,
        &records,
    );
    Ok(())
}

pub fn load_strategies() -> anyhow::Result<Vec<StrategyRecord>> {
    let file = File::open(Path::new("bwapi-data").join("read").join("learned.json"))?;
    serde_json::from_reader(file).map_err(|e| anyhow::anyhow!(e))
}

pub fn update_strategy_records(
    records: &mut Vec<StrategyRecord>,
    strategy: &Strategy,
    win: bool,
    enemy: &str,
    map: &str,
) {
    let mut record = records
        .iter_mut()
        .find(|s| s.map == map && s.strategy == strategy.name && s.enemy == enemy);
    if record.is_none() {
        records.push(StrategyRecord {
            map: map.to_string(),
            enemy: enemy.to_string(),
            strategy: strategy.name.to_string(),
            wins: 0,
            losses: 0,
        });
        record = Some(records.last_mut().unwrap());
    }
    if win {
        record.unwrap().wins += 1;
    } else {
        record.unwrap().losses += 1;
    }
}

pub struct Strategy {
    pub name: &'static str,
    pub func: &'static dyn Fn(&mut MyModule) -> anyhow::Result<()>,
}

impl Strategy {
    pub fn from_fn<T: Fn(&mut MyModule) -> anyhow::Result<()>>(strat: &'static T) -> Self {
        Self {
            name: std::any::type_name::<T>()
                .split(':')
                .last()
                .expect("Strategy has no name"),
            func: strat,
        }
    }
    pub fn tick(&self, module: &mut MyModule) -> anyhow::Result<()> {
        (self.func)(module)
    }

    pub fn win_probability(&self, records: &[StrategyRecord], enemy: &str, map: &str) -> f32 {
        let rnd = (((enemy as *const str as *const u8 as usize) / 3 + 1337) * 7 % 5) as i32;
        let Some(record) = records
            .iter()
            .filter(|r| &r.strategy == self.name && &r.enemy == enemy && &r.map == map)
            .next()
            .or_else(|| {
                records
                    .iter()
                    .filter(|r| &r.strategy == self.name && (&r.enemy == enemy || &r.map == map))
                    .next()
            })
            .or_else(|| records.iter().filter(|r| &r.strategy == self.name).next())
            else { return 0.5 };
        return (record.wins + rnd) as f32 / (record.losses + record.wins + rnd) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_name_test() {
        assert_eq!(
            Strategy::from_fn(&crate::MyModule::three_hatch_spire).name,
            "three_hatch_spire"
        );
    }
}
