use crate::*;
use ahash::AHashMap;
use rsbwapi::*;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub struct UnitCluster<'a> {
    pub vanguard: &'a SUnit,
    pub units: &'a [&'a SUnit],
    pub vanguard_dist_to_target: u32,
}

struct Target<'a> {
    unit: &'a SUnit,
    priority: i32,
    health_including_shields: Cell<i32>,
    attacker_count: Cell<i32>,
    cliffed_tank: bool,
}

impl<'a> Target<'a> {
    fn new(module: &MyModule, unit: &'a SUnit, vanguard: &SUnit) -> Self {
        Self {
            unit,
            priority: module.target_priority(unit),
            health_including_shields: Cell::new(unit.hit_points() + unit.shields()),
            attacker_count: Cell::new(0),
            cliffed_tank: false, // TODO
        }
    }

    fn deal_damage(&self, module: &MyModule, attacker: &SUnit) {
        if self.unit.being_healed() {
            return;
        }

        let mut damage = attacker.damage_to(&self.unit);

        if !attacker.flying()
            && attacker.get_type().is_ranged()
            && module.game.get_ground_height(attacker.tile_position())
                < module.game.get_ground_height(self.unit.tile_position())
        {
            damage /= 2;
        }

        self.health_including_shields
            .set(self.health_including_shields.get() - damage);
    }
}

struct Attacker<'a> {
    unit: &'a SUnit,
    targets: Vec<&'a Target<'a>>,
    frames_to_attack: i32,
    close_targets: Vec<&'a Target<'a>>,
}

impl<'a> Attacker<'a> {
    fn new(unit: &'a SUnit) -> Self {
        Self {
            unit,
            targets: vec![],
            frames_to_attack: i32::MAX,
            close_targets: vec![],
        }
    }

    fn is_target_reachable_enemy_base(&self, module: &MyModule, target_position: Position) -> bool {
        // TODO search a base etc
        // let target_base : Some;
        // if let Some(target_base) = target_base {
        //     if !target_base.player.is_enemy() {
        //         return false;
        //     }
        //     if target_base.last_scouted != -1
        //         && target_base
        //             .resource_depot
        //             .map(|d| d.exists())
        //             .unwrap_or(false)
        //     {
        //         return false;
        //     }
        //     true
        // } else {
        false
        // }
    }
}

impl MyModule {
    pub fn target_priority(&self, target: &SUnit) -> i32 {
        let close_to_our_base = false;
        let target_type = target.get_type();

        match target_type {
            UnitType::Zerg_Infested_Terran
            | UnitType::Protoss_High_Templar
            | UnitType::Protoss_Reaver => 15,
            UnitType::Terran_Vulture_Spider_Mine if !target.burrowed() => 15,
            UnitType::Protoss_Observer
                if self
                    .units
                    .my_completed
                    .iter()
                    .filter(|u| u.get_type().has_permanent_cloak() || u.get_type().is_cloakable())
                    .next()
                    .is_some() =>
            {
                15
            }
            UnitType::Protoss_Arbiter | UnitType::Terran_Siege_Tank_Siege_Mode => 14,
            UnitType::Terran_Siege_Tank_Tank_Mode
            | UnitType::Terran_Dropship
            | UnitType::Protoss_Shuttle
            | UnitType::Terran_Science_Vessel
            | UnitType::Zerg_Scourge
            | UnitType::Protoss_Observer
            | UnitType::Zerg_Nydus_Canal => 13,
            _ if target_type.is_building() && close_to_our_base => {
                if target_type.can_attack() {
                    12
                } else {
                    10
                }
            }
            UnitType::Terran_Bunker => 11,
            _ if target_type.is_worker() => {
                if (target.constructing() || target.repairing()) && close_to_our_base {
                    15
                } else
                // Blocking a narrow choke makes you critical.
                if self.is_in_narrow_choke(target.tile_position()) {
                    14
                } else {
                    match target.get_order_target() {
                        // Repairing
                        Some(repair_target)
                            if target.repairing()
                                && repair_target.get_type().ground_weapon() != WeaponType::None =>
                        {
                            14
                        }
                        Some(repair_target)
                            if target.repairing()
                                && repair_target.get_type() == UnitType::Terran_Bunker
                                && repair_target.get_ground_weapon().max_range > 128 =>
                        {
                            13
                        }
                        _ if self.game.get_frame_count() - target.last_attack_frame() < 96 => 11,
                        _ if target.constructing() => 10,
                        _ => 9,
                    }
                }
            }
            _ => {
                if target_type.can_attack() {
                    11
                } else if target_type.is_spellcaster() {
                    10
                } else if target_type.is_resource_depot() {
                    7
                } else if matches!(
                    target_type,
                    UnitType::Protoss_Pylon
                        | UnitType::Zerg_Spawning_Pool
                        | UnitType::Terran_Factory
                        | UnitType::Terran_Armory
                ) {
                    5
                } else if target_type.is_addon() {
                    1
                } else if !target.completed() || (target_type.requires_psi() && !target.powered()) {
                    2
                } else if target_type.gas_price() > 0 {
                    4
                } else if target_type.mineral_price() > 0 {
                    3
                } else {
                    1
                }
            }
        }
    }

    pub fn select_targets(
        &self,
        cluster: UnitCluster,
        mut target_units: Vec<&SUnit>,
        target_position: Position,
        static_position: bool,
    ) -> Vec<(SUnit, Option<SUnit>)> {
        let mut result = vec![];
        let mut dist_to_target_position: AHashMap<&SUnit, u32> = AHashMap::new();

        if !cluster.vanguard.flying() {
            target_units.retain(|unit| {
                if !unit.flying() {
                    let path = self.map.get_path(unit.position(), target_position);
                    if !path.0.is_empty() {
                        dist_to_target_position.insert(unit, path.1);
                        if (path.1.saturating_sub(cluster.vanguard_dist_to_target)) > 700
                            && !unit.is_in_weapon_range(&cluster.vanguard)
                        {
                            CVIS.lock().unwrap().log(format!(
                                "Drop target {:?} {}",
                                unit.get_type(),
                                unit.id()
                            ));
                            return false;
                        }
                    }
                }
                true
            });
        }

        let target_is_reachable_enemy_base = !static_position
            && self.is_target_reachable_enemy_base(target_position, cluster.vanguard);

        let targets: Vec<Target> = target_units
            .iter()
            .filter(|u| u.exists())
            .map(|target_unit| Target::new(self, target_unit, cluster.vanguard))
            .collect();

        let mut attackers = Vec::with_capacity(cluster.units.len());
        let get_current_target = |unit: &SUnit| {
            unit.get_order_target()
                .map(|tu| targets.iter().find(|t| t.unit == &tu))
                .flatten()
        };

        for unit in cluster.units {
            if unit.sleeping() {
                let target = get_current_target(unit);
                if let Some(target) = target {
                    if unit.cooldown() <= self.game.get_latency_frames() + 2 {
                        target.deal_damage(self, unit);
                    }
                }

                result.push(((*unit).clone(), target.map(|t| t.unit.clone())));
                continue;
            }

            let is_ranged = unit.get_type().is_ranged();
            let distance_to_target_position = unit.position().distance(target_position);

            let mut has_non_building = false;
            let mut filtered_targets = vec![];

            for target in targets.iter() {
                CVIS.lock().unwrap().log_unit_frame(
                    unit,
                    format!(
                        "Start filtering: {:?} {}",
                        target.unit.get_type(),
                        target.unit.id(),
                    ),
                );
                if target.unit.get_type() == UnitType::Zerg_Larva
                    || target.unit.get_type() == UnitType::Zerg_Egg
                    || !unit.detected()
                    || unit.hit_points() <= 0
                    || !unit.can_attack(target.unit)
                {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(unit, "Drop".to_string());
                    continue;
                }

                if (is_ranged || unit.get_type().is_worker())
                    && unit.get_type() != UnitType::Protoss_Reaver
                    && target.unit.under_dark_swarm()
                {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(unit, "Drop".to_string());
                    continue;
                }

                if !is_ranged && (target.unit.under_disruption_web() || target.unit.under_storm()) {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(unit, "Drop".to_string());
                    continue;
                }

                let range = unit.distance_to(target.unit) as i32;
                let dist_to_range = 0.max(range - unit.weapon_against(target.unit).max_range);

                // In static position mode, units only attack what they are in range of
                if static_position && dist_to_range > 0 {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(unit, "Drop".to_string());
                    continue;
                }

                // Cliffed tanks can only be attacked by units in range with vision
                if target.cliffed_tank && (dist_to_range > 0 || !target.unit.visible()) {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(unit, "Drop".to_string());
                    continue;
                }

                // The next checks apply if we are not close to our target position
                if distance_to_target_position > 500.0 {
                    // Skip targets that are out of range and moving away from us
                    if dist_to_range > 0 {
                        let predicted_target_position = target.unit.predict_position(1);
                        if predicted_target_position.is_valid(&&self.game)
                            && unit.position().distance(predicted_target_position) as i32 > range
                        {
                            CVIS.lock()
                                .unwrap()
                                .log_unit_frame(unit, "Drop".to_string());
                            continue;
                        }
                    }

                    // Skip targets that are further away from the target position and are either:
                    // - Out of range
                    // - In our range, but we aren't in their range, and we are on cooldown
                    if unit.flying() == target.unit.flying()
                        && (dist_to_range > 0
                            || unit.cooldown() > 0 && !target.unit.is_in_weapon_range(unit))
                    {
                        if let (Some(unit_dist), Some(target_dist)) = (
                            dist_to_target_position.get(unit),
                            dist_to_target_position.get(&target.unit),
                        ) {
                            if unit_dist < target_dist {
                                CVIS.lock()
                                    .unwrap()
                                    .log_unit_frame(unit, "Drop".to_string());
                                continue;
                            }
                        }
                    }
                }
                // This is a suitable target
                filtered_targets.push((target, dist_to_range));

                has_non_building |= target.priority > 7;
            }

            let mut attacker = Attacker::new(unit);
            attacker.targets.reserve(filtered_targets.len());
            attacker.close_targets.reserve(filtered_targets.len());
            for target_and_dist_to_range in filtered_targets.iter() {
                let target = target_and_dist_to_range.0;

                // If we are targeting an enemy base, ignore outlying buildings (except static defense) unless we have a higher-priority target
                // Rationale: When we have a non-building target, we want to consider buildings since they might be blocking us from attacking them
                if target.priority < 7
                    && (has_non_building
                        || target.unit.flying()
                            && target_is_reachable_enemy_base
                            && distance_to_target_position > 200.0)
                {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(&attacker.unit, "Drop".to_string());
                    continue;
                }

                attacker.targets.push(target);

                let frames_to_attack = unit.cooldown().max(
                    (target_and_dist_to_range.1 as f64 / unit.top_speed()) as i32
                        + self.game.get_remaining_latency_frames()
                        + 2,
                );
                if frames_to_attack < attacker.frames_to_attack {
                    attacker.frames_to_attack = frames_to_attack;
                    attacker.close_targets.clear();
                    attacker.close_targets.push(target);
                } else if frames_to_attack == attacker.frames_to_attack {
                    attacker.close_targets.push(target);
                }
            }

            for close_target in attacker.close_targets.iter() {
                close_target
                    .attacker_count
                    .set(close_target.attacker_count.get() + 1);
            }
            attackers.push(attacker);
        }

        attackers.sort_by(|a, b| {
            a.frames_to_attack
                .cmp(&b.frames_to_attack)
                .then_with(|| a.close_targets.len().cmp(&b.close_targets.len()))
                .then_with(|| a.targets.len().cmp(&b.targets.len()))
                .then_with(|| a.unit.id().cmp(&b.unit.id()))
        });

        // Now assign each unit a target, skipping any that are simulated to already be dead
        for attacker in attackers {
            let unit = &attacker.unit;

            let mut best_target = None;
            let mut best_score = -999999;
            let mut best_attacker_count = 0;
            let mut best_dist = i32::MAX;

            let is_ranged = unit.get_type().is_ranged();
            let cooldown_move_frames =
                0.max(unit.cooldown() - self.game.get_remaining_latency_frames() - 2);

            let distance_to_target_position = unit.position().distance(target_position);
            for potential_target in attacker.targets {
                CVIS.lock().unwrap().log_unit_frame(
                    &attacker.unit,
                    format!(
                        "Start Eval: {:?} {}",
                        potential_target.unit.get_type(),
                        potential_target.unit.id(),
                    ),
                );
                if potential_target.health_including_shields.get() <= 0 {
                    CVIS.lock()
                        .unwrap()
                        .log_unit_frame(&attacker.unit, "Health dropout".to_string());
                    continue;
                }
                // Initialize the score as a formula of the target priority and how far outside our attack range it is
                // Each priority step is equivalent to 2 tiles
                // If the unit is on cooldown, we assume it can move towards the target before attacking
                let target_dist = unit.distance_to(potential_target.unit) as i32;
                let range = unit.weapon_against(potential_target.unit).max_range;
                let mut score = 2 * 32 * potential_target.priority
                    - 0.max(
                        target_dist
                            - (cooldown_move_frames as f64 - unit.top_speed()) as i32
                            - range,
                    );

                // Now adjust the score according to some rules

                // Give a bonus to units that are already in range
                // Melee units get an extra bonus, as they have a more difficult time getting around blocking things
                if target_dist <= range {
                    score += if is_ranged { 64 } else { 160 };
                }

                // Give a bonus to injured targets
                // This is what provides some focus fire behaviour, as we simulate previous attackers' hits
                let health_percentage = potential_target.health_including_shields.get() as f64
                    / (potential_target.unit.get_type().max_hit_points()
                        + potential_target.unit.get_type().max_shields())
                        as f64;
                score += (160.0 * (1.0 - health_percentage)) as i32;

                // Penalize ranged units fighting uphill
                if is_ranged
                    && !unit.flying()
                    && self.game.get_ground_height(unit.tile_position())
                        < self
                            .game
                            .get_ground_height(potential_target.unit.tile_position())
                {
                    score -= 2 * 32;
                }

                // Avoid defensive matrix
                if potential_target.unit.defense_matrixed() {
                    score -= 4 * 32;
                }

                // Give a bonus for enemies that are closer to our target position (usually the enemy base)
                if potential_target.unit.position().distance(target_position)
                    < distance_to_target_position
                {
                    score += 2 * 32;
                }

                // Give bonus to units under dark swarm
                // Ranged units skip these targets earlier
                if potential_target.unit.under_dark_swarm() {
                    score += 4 * 32;
                }

                // Adjust based on the threat level of the enemy unit to us
                if potential_target.unit.can_attack(unit) {
                    if potential_target.unit.is_in_weapon_range(unit) {
                        score += 6 * 32;
                    } else if unit.is_in_weapon_range(potential_target.unit) {
                        score += 4 * 32;
                    } else {
                        score += 3 * 32;
                    }
                }

                // Give a bonus to non-moving or braking targets, and a penalty to units that are faster than us
                if !potential_target.unit.moving() {
                    if potential_target.unit.sieged()
                        || potential_target.unit.get_order() == Order::Sieging
                        || potential_target.unit.get_order() == Order::Unsieging
                    {
                        score += 48;
                    } else {
                        score += 24;
                    }
                } else if potential_target.unit.braking() {
                    score += 16;
                } else if potential_target.unit.top_speed() >= unit.top_speed() {
                    score -= 4 * 32;
                }

                // Take the damage type into account
                let damage = unit
                    .weapon_against(potential_target.unit)
                    .weapon_type
                    .damage_type();
                if damage == DamageType::Explosive {
                    if potential_target.unit.get_type().size() == UnitSizeType::Large {
                        score += 32;
                    }
                } else if damage == DamageType::Concussive {
                    if potential_target.unit.get_type().size() == UnitSizeType::Small {
                        score += 32;
                    } else if potential_target.unit.get_type().size() == UnitSizeType::Large {
                        score -= 32;
                    }
                }

                // Give a big bonus to SCVs repairing/constructing a bunker that we can attack without coming into range of the bunker
                if (potential_target.unit.repairing() || potential_target.unit.constructing())
                    && potential_target
                        .unit
                        .get_order_target()
                        .map(|ot| ot.get_type() == UnitType::Terran_Bunker)
                        .unwrap_or(false)
                    && unit.position().distance(potential_target.unit.position()) as i32 <= range
                {
                    let bunker = potential_target.unit.get_order_target().unwrap();
                    if !bunker.is_in_weapon_range(unit) {
                        score += 256;
                    }
                }

                CVIS.lock()
                    .unwrap()
                    .log_unit_frame(&attacker.unit, format!("Score {}", score,));
                // See if this is the best target
                // Criteria:
                // - Score is higher
                // - Attackers is higher
                // - Distance is lower
                if score
                    .cmp(&best_score)
                    .then_with(|| {
                        potential_target
                            .attacker_count
                            .get()
                            .cmp(&best_attacker_count)
                    })
                    .then_with(|| best_dist.cmp(&target_dist))
                    .then(Ordering::Equal)
                    == Ordering::Greater
                {
                    best_score = score;
                    best_attacker_count = potential_target.attacker_count.get();
                    best_dist = target_dist;
                    best_target = Some(potential_target);
                }
            }

            CVIS.lock().unwrap().log_unit_frame(
                &attacker.unit,
                format!(
                    "Result {} {:?}",
                    best_score,
                    best_target.map(|t| (t.unit.id(), t.unit.get_type()))
                ),
            );
            // For carriers, avoid frequently switching targets
            if unit.get_type() == UnitType::Protoss_Carrier {
                let current_target = get_current_target(unit);
                if let Some(current_target) = current_target {
                    if (unit.position().distance(current_target.unit.position()) as i64) < 11 * 32
                        && unit.last_command_frame() > self.game.get_frame_count() - 96
                    {
                        best_target = Some(current_target)
                    }
                }
            }
            if let Some(best_target) = best_target {
                if unit.is_in_weapon_range(best_target.unit) {
                    best_target.deal_damage(self, unit);
                }
                result.push((attacker.unit.clone(), Some(best_target.unit.clone())));
            } else {
                result.push((attacker.unit.clone(), None));
            }
        }

        result
    }
}
