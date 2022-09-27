use crate::{stop_frames, SUnit};
use fixed::types::I24F8;
use rsbwapi::{ExplosionType, Race, UnitSizeType, UnitType, WeaponType};
use std::cmp::Ordering;

const BURROW_FRAMES: i32 = 24;
const STIM_FRAMES: i32 = 37;
const STIM_HEALTH_COST: I24F8 = I24F8::from_bits(10 << 8);
const MEDIC_HEAL_RANGE_SQUARED: i32 = 30 * 30;
const SCV_REPAIR_RANGE_SQUARED: i32 = 5 * 5;
const COOLDOWN_INTERCEPTOR: i32 = 45;
const COOLDOWN_REAVER: i32 = 60;
const FRAME_SKIP: i32 = 1;

#[derive(Copy, Clone)]
pub enum SplashType {
    RadialSplash,
    RadialEnemySplash,
    LineSplash,
    Bounce,
    Irrelevant,
}

impl Default for SplashType {
    fn default() -> Self {
        Self::Irrelevant
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DamageType {
    Explosive,
    Concussive,
    Irrelevant,
}

impl Default for DamageType {
    fn default() -> Self {
        Self::Irrelevant
    }
}

#[derive(Clone, Copy)]
pub enum UnitSize {
    Small,
    Medium,
    Large,
    Irrelevant,
}

impl Default for UnitSize {
    fn default() -> Self {
        Self::Irrelevant
    }
}

#[derive(Default, Copy, Clone)]
pub struct Weapon {
    max_range: i32,
    min_range_squared: i32,
    max_range_squared: i32,
    damage: I24F8,
    hits: i32,
    inner_splash_radius: i32,
    inner_splash_radius_squared: i32,
    median_splash_radius_squared: i32,
    outer_splash_radius_squared: i32,
    cooldown: i32,
    damage_type: DamageType,
    splash_type: SplashType,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum TargetingPriority {
    Low,
    Medium,
    Highest,
}

impl Default for TargetingPriority {
    fn default() -> Self {
        Self::Highest
    }
}

#[derive(Default, Clone)]
pub struct Agent {
    pub attack_target_priority: TargetingPriority,
    armor: I24F8,
    shield_upgrades: i32,
    elevation_level: i32,
    pub x: i32,
    pub y: i32,
    next_x: i32,
    next_y: i32,
    speed_upgrade: bool,
    base_speed: f32,
    speed_squared: i32,
    speed: f32,
    speed_factor: f32,
    protoss_scout: bool,
    vx: i32,
    vy: i32,
    health: I24F8,
    pub is_alive: bool,
    max_health: I24F8,
    healed_this_frame: bool,
    stim_timer: i32,
    ensnare_timer: i32,
    hp_construction_rate: i32,
    shields: I24F8,
    max_shields: I24F8,
    energy: I24F8,
    max_energy: I24F8,
    pub attack_counter: i32,
    pub cooldown: i32,
    cooldown_upgrade: bool,
    pub sleep_timer: i32,
    stop_frames: i32,
    can_stim: bool,
    plague_damage_per_frame: I24F8,
    health_regen: I24F8,
    is_suicider: bool,
    is_healer: bool,
    is_flyer: bool,
    is_organic: bool,
    is_mechanic: bool,
    is_kiter: bool,
    is_repairer: bool,
    protected_by_dark_swarm: bool,
    can_unburrow: bool,
    burrowed: bool,
    burrowed_attacker: bool,
    detected: bool,
    stasis_timer: i32,
    size: UnitSize,
    is_melee: bool,
    air_weapon: Weapon,
    ground_weapon: Weapon,
    seekable_target: bool,
    ground_seek_range_squared: i32,
    attack_target: Option<usize>,
    restore_target: Option<usize>,
    interceptors: Vec<i32>,
    pub unit_type: UnitType,
    pub id: usize,
}

impl Agent {
    pub fn from_unit(unit: &SUnit) -> Self {
        let unit_type = unit.get_type();
        let ground_weapon = unit.get_ground_weapon();
        let air_weapon = unit.get_air_weapon();
        let player = &unit.player().player;
        let detected = unit.detected();
        // TODO SUnit should have the correct values already!
        let base = Self::from_unit_type(
            unit_type,
            player.get_upgrade_level(ground_weapon.weapon_type.upgrade_type()),
            player.get_upgrade_level(air_weapon.weapon_type.upgrade_type()),
            0,
            0,
            unit.has_speed_upgrade(),
            false,
            false,
        );
        Self {
            id: unit.id(),
            energy: I24F8::from_num(unit.energy()),
            health: I24F8::from_num(unit.hit_points()),
            x: unit.position().x,
            y: unit.position().y,
            cooldown: unit.cooldown(),
            detected,
            burrowed: unit.burrowed(),
            stasis_timer: unit.stasis_timer(),
            sleep_timer: base.sleep_timer.max(
                if !unit.powered() || (unit.exists() && (unit.gathering() || unit.constructing())) {
                    // Just sleep throughout any sim if unpowered, or busy otherwise
                    std::i32::MAX
                } else {
                    unit.lockdown_timer().max(unit.remaining_build_time())
                },
            ),
            ensnare_timer: unit.ensnare_timer(),
            plague_damage_per_frame: I24F8::from_bits(if unit.plagued() {
                (WeaponType::Plague.damage_amount() << 8) / 76
            } else {
                0
            }),
            elevation_level: unit.elevation_level(),
            protected_by_dark_swarm: unit.under_dark_swarm(),
            stim_timer: unit.stim_timer(),
            ..base
        }
    }

    pub fn from_unit_type(
        unit_type: UnitType,
        ground_weapon_upgrades: i32,
        air_weapon_upgrades: i32,
        ground_weapon_range_upgrade: i32,
        air_weapon_range_upgrade: i32,
        speed_upgrade: bool,
        energy_upgrade: bool,
        cooldown_upgrade: bool,
    ) -> Self {
        let mut ground_weapon = unit_type.ground_weapon();
        let mut air_weapon = unit_type.air_weapon();
        let mut range_extension = 0;
        let mut hits_factor = 1;
        let mut max_ground_hits = unit_type.max_ground_hits();
        let mut max_ground_hits = unit_type.max_ground_hits();
        let max_air_hits = unit_type.max_air_hits();
        let ground_cooldown;
        let air_cooldown;
        match unit_type {
            UnitType::Terran_Bunker => {
                ground_weapon = WeaponType::Gauss_Rifle;
                air_weapon = WeaponType::Gauss_Rifle;
                ground_cooldown = ground_weapon.damage_cooldown();
                air_cooldown = air_weapon.damage_cooldown();
                range_extension = 64;
                hits_factor = 4;
            }
            UnitType::Protoss_Interceptor => {
                ground_cooldown = COOLDOWN_INTERCEPTOR;
                air_cooldown = COOLDOWN_INTERCEPTOR;
            }
            UnitType::Protoss_Reaver => {
                ground_weapon = WeaponType::Scarab;
                ground_cooldown = COOLDOWN_REAVER;
                air_cooldown = COOLDOWN_REAVER;
                max_ground_hits = UnitType::Protoss_Scarab.max_ground_hits();
            }
            _ => {
                ground_cooldown = ground_weapon.damage_cooldown();
                air_cooldown = air_weapon.damage_cooldown();
            }
        }
        let max_health = I24F8::from_num(unit_type.max_hit_points());
        let max_shields = I24F8::from_num(unit_type.max_shields());
        Self {
            unit_type,
            can_stim: matches!(
                unit_type,
                UnitType::Terran_Marine | UnitType::Terran_Firebat
            ),
            attack_target_priority: match unit_type {
                UnitType::Protoss_Interceptor => TargetingPriority::Low,
                unit if !unit.can_attack() => TargetingPriority::Medium,
                _ => TargetingPriority::Highest,
            },
            is_flyer: unit_type.is_flyer(),
            is_healer: unit_type == UnitType::Terran_Medic,
            health: max_health,
            is_alive: true,
            shields: max_shields,
            max_health,
            max_shields,
            air_weapon: Self::weapon(
                air_weapon_upgrades,
                range_extension + air_weapon_range_upgrade,
                hits_factor,
                air_weapon,
                max_air_hits,
                air_cooldown,
            ),
            ground_weapon: Self::weapon(
                ground_weapon_upgrades,
                range_extension + ground_weapon_range_upgrade,
                hits_factor,
                ground_weapon,
                max_ground_hits,
                ground_cooldown,
            ),
            is_organic: unit_type.is_organic(),
            health_regen: if !matches!(
                unit_type,
                UnitType::Zerg_Egg | UnitType::Zerg_Lurker_Egg | UnitType::Zerg_Larva,
            ) && unit_type.get_race() == Race::Zerg
            {
                I24F8::from_bits(4)
            } else {
                I24F8::ZERO
            },
            is_suicider: matches!(
                unit_type,
                UnitType::Zerg_Scourge
                    | UnitType::Zerg_Infested_Terran
                    | UnitType::Terran_Vulture_Spider_Mine
                    | UnitType::Protoss_Scarab
            ),
            stop_frames: stop_frames(unit_type),
            size: match unit_type.size() {
                UnitSizeType::Small => UnitSize::Small,
                UnitSizeType::Medium => UnitSize::Medium,
                UnitSizeType::Large => UnitSize::Large,
                _ => UnitSize::Irrelevant,
            },
            armor: I24F8::from_num(unit_type.armor()),
            is_kiter: matches!(
                unit_type,
                UnitType::Terran_Marine
                    | UnitType::Terran_Vulture
                    | UnitType::Zerg_Mutalisk
                    | UnitType::Protoss_Dragoon
            ),
            max_energy: I24F8::from_num(
                unit_type.max_energy() + if energy_upgrade { 50 } else { 0 },
            ),
            detected: true,
            burrowed_attacker: unit_type == UnitType::Zerg_Lurker,
            base_speed: unit_type.top_speed() as f32,
            speed_factor: 1.0,
            protoss_scout: unit_type == UnitType::Protoss_Scout,
            speed_upgrade,
            cooldown_upgrade,
            hp_construction_rate: if unit_type.build_time() == 0 {
                0
            } else {
                let build_time = unit_type.build_time();
                1.max(
                    (unit_type.max_hit_points() - unit_type.max_hit_points() / 10 + build_time - 1)
                        / build_time,
                )
            },
            is_repairer: unit_type == UnitType::Terran_SCV,
            is_mechanic: unit_type.is_mechanical(),
            is_melee: ground_weapon.damage_amount() > 0 && ground_weapon.max_range() <= 32,
            ground_seek_range_squared: unit_type.seek_range() * unit_type.seek_range(),
            seekable_target: !unit_type.is_worker()
                && !unit_type.is_building()
                && !unit_type.is_flyer(),
            ..Default::default()
        }
    }

    pub fn with_y(self, y: i32) -> Agent {
        Self { y, ..self }
    }

    pub fn with_x(self, x: i32) -> Agent {
        Self { x, ..self }
    }

    pub fn with_target_priority(self, attack_target_priority: TargetingPriority) -> Agent {
        Self {
            attack_target_priority,
            ..self
        }
    }

    pub fn with_speed_factor(self, speed_factor: f32) -> Agent {
        Self {
            speed_factor,
            ..self
        }
    }

    fn weapon(
        weapon_upgrades: i32,
        range_extension: i32,
        hits_factor: i32,
        weapon: WeaponType,
        max_hits: i32,
        cooldown: i32,
    ) -> Weapon {
        let max_range = weapon.max_range() + range_extension;
        Weapon {
            cooldown,
            damage: I24F8::from_num(
                hits_factor
                    * (weapon.damage_amount() + weapon.damage_bonus() * weapon_upgrades)
                    * weapon.damage_factor()
                    * max_hits,
            ),
            damage_type: match weapon.damage_type() {
                rsbwapi::DamageType::Concussive => DamageType::Concussive,
                rsbwapi::DamageType::Explosive => DamageType::Explosive,
                _ => DamageType::Irrelevant,
            },
            splash_type: match weapon {
                WeaponType::Subterranean_Spines => SplashType::LineSplash,
                WeaponType::Glave_Wurm => SplashType::Bounce,
                _ => match weapon.explosion_type() {
                    ExplosionType::Enemy_Splash => SplashType::RadialEnemySplash,
                    ExplosionType::Radial_Splash | ExplosionType::Nuclear_Missile => {
                        SplashType::RadialEnemySplash
                    }
                    _ => SplashType::Irrelevant,
                },
            },
            hits: max_hits,
            inner_splash_radius: weapon.inner_splash_radius(),
            inner_splash_radius_squared: weapon.inner_splash_radius()
                * weapon.inner_splash_radius(),
            max_range,
            max_range_squared: max_range * max_range,
            min_range_squared: weapon.min_range() * weapon.min_range(),
            median_splash_radius_squared: weapon.median_splash_radius()
                * weapon.median_splash_radius(),
            outer_splash_radius_squared: weapon.outer_splash_radius()
                * weapon.outer_splash_radius(),
        }
    }

    fn is_stasised(&self) -> bool {
        self.stasis_timer > 0
    }

    fn is_sleeping(&self) -> bool {
        self.sleep_timer > 0
    }

    fn is_stimmed(&self) -> bool {
        self.stim_timer > 0
    }

    pub fn health(&self) -> i32 {
        self.health.min(self.max_health).to_num::<i32>().max(0)
    }

    pub fn shields(&self) -> i32 {
        self.shields.min(self.max_shields).to_num::<i32>().max(0)
    }

    fn weapon_vs(&self, enemy: &Agent) -> &Weapon {
        if enemy.is_flyer {
            &self.air_weapon
        } else {
            &self.ground_weapon
        }
    }

    fn burrow(&mut self) -> bool {
        if !self.can_unburrow {
            return false;
        }
        self.burrowed = true;
        self.sleep_timer = BURROW_FRAMES;
        return true;
    }

    fn unburrow(&mut self) -> bool {
        if !self.can_unburrow {
            return false;
        }
        self.burrowed = false;
        self.sleep_timer = BURROW_FRAMES;
        return true;
    }

    fn regen(&mut self, amount: I24F8) {
        self.health += amount;
    }

    fn stim(&mut self) {
        self.stim_timer = STIM_FRAMES;
        self.consume_health(STIM_HEALTH_COST);
    }

    fn consume_health(&mut self, amount: I24F8) {
        // health can be > max_health, so fix it up here
        self.health = self.health.min(self.max_health) - amount;
    }

    fn consume_energy(&mut self, amount: I24F8) {
        // energy can be > max_energy, so fix it up here
        self.energy = self.energy.min(self.max_energy) - amount;
    }

    fn update_speed(&mut self) {
        self.speed = self.base_speed;
        let mut m = 0;
        if self.stim_timer > 0 {
            m += 1
        }
        if self.speed_upgrade {
            m += 1
        }
        if self.ensnare_timer > 0 {
            m -= 1
        }
        if m < 0 {
            self.speed /= 2.0;
        }
        if m > 0 {
            if self.protoss_scout {
                self.speed = 6.0 + 2.0 / 3.0;
            } else {
                self.speed *= 1.5;
                let min_speed = 3.0 + 1.0 / 3.0;
                self.speed = self.speed.max(min_speed);
            }
        }
        self.speed *= self.speed_factor;
        self.speed_squared = (self.speed * self.speed).round() as i32;
    }

    fn update_position(&mut self, walkability: impl Fn(i32, i32) -> bool) {
        let nx = self.x + self.vx;
        let ny = self.y + self.vy;
        if self.is_flyer || walkability(nx, ny) {
            self.x = nx;
            self.y = ny;
        } else {
            // TODO: We simulate "chokes" by slowing down units, is that ok?
            self.x = (self.x + nx) / 2;
            self.y = (self.y + ny) / 2;
        }
    }
}

impl From<UnitType> for Agent {
    fn from(unit_type: UnitType) -> Self {
        Self::from_unit_type(unit_type, 0, 0, 0, 0, false, false, false)
    }
}

trait Script {
    fn simulate(&mut self, agent_index: usize, allies: &mut [Agent], enemies: &mut [Agent])
        -> bool;
}

struct Suicider;

impl Script for Suicider {
    fn simulate(
        &mut self,
        agent_index: usize,
        allies: &mut [Agent],
        enemies: &mut [Agent],
    ) -> bool {
        let mut agent = &mut allies[agent_index];
        let mut selected_enemy = None;
        let mut selected_distance_squared = if agent.ground_seek_range_squared > 0 {
            agent.ground_seek_range_squared + 1
        } else {
            std::i32::MAX
        };
        for (enemy_index, enemy) in enemies.iter().enumerate() {
            let weapon = agent.weapon_vs(enemy);
            if enemy.health > 0 && weapon.damage > 0 && enemy.detected {
                let distance_squared = distance_squared(agent, enemy);
                if distance_squared < selected_distance_squared
                    && (agent.ground_seek_range_squared == 0 || enemy.seekable_target)
                {
                    selected_distance_squared = distance_squared;
                    selected_enemy = Some(enemy_index);

                    // If we can hit it this frame, we're done searching
                    if selected_distance_squared <= agent.speed_squared {
                        break;
                    }
                }
            }
        }

        let selected_enemy = if let Some(selected_enemy) = selected_enemy {
            selected_enemy
        } else {
            return false;
        };

        agent.detected = true;

        if selected_distance_squared <= agent.speed_squared {
            let weapon = *agent.weapon_vs(&enemies[selected_enemy]);
            attack(agent_index, allies, enemies, weapon, selected_enemy);
        } else {
            let selected_enemy = &enemies[selected_enemy];
            move_toward(
                agent,
                (selected_enemy.x, selected_enemy.y),
                (selected_distance_squared as f32).sqrt(),
                0,
            );
        }
        true
    }
}

struct Repairer;

impl Script for Repairer {
    fn simulate(
        &mut self,
        agent_index: usize,
        allies: &mut [Agent],
        enemies: &mut [Agent],
    ) -> bool {
        let agent = &allies[agent_index];
        if agent.energy < 0 {
            return true;
        }
        let mut selected_ally = None;
        let mut selected_distance_squared = std::i32::MAX;

        if let Some((ally_index, ally)) = agent.restore_target.map(|i| (i, &allies[i])) {
            if ally.is_alive && ally.health < ally.max_health {
                let distance_squared = distance_squared(agent, ally);
                if distance_squared <= SCV_REPAIR_RANGE_SQUARED {
                    selected_ally = Some(ally_index);
                    selected_distance_squared = distance_squared;
                }
            }
        }

        if selected_ally.is_none() {
            for (ally_index, ally) in allies.iter().enumerate().filter(|(ally_index, ally)| {
                ally.is_mechanic
                    && !ally.is_stasised()
                    && ally.health < ally.max_health
                    && !ally.healed_this_frame
                    && *ally_index == agent_index
            }) {
                let distance_squared = distance_squared(agent, ally);
                if distance_squared < selected_distance_squared {
                    selected_distance_squared = distance_squared;
                    selected_ally = Some(ally_index);
                }

                // If we can repair it this frame, we're done searching
                if selected_distance_squared <= SCV_REPAIR_RANGE_SQUARED {
                    break;
                }
            }
        }
        let ally_pos = selected_ally.map(|i| (allies[i].x, allies[i].y));
        let agent = &mut allies[agent_index];
        agent.restore_target = selected_ally;

        if let Some(ally_index) = selected_ally {
            move_toward(
                agent,
                ally_pos.unwrap(),
                (selected_distance_squared as f32).sqrt(),
                0,
            );
            if selected_distance_squared > SCV_REPAIR_RANGE_SQUARED {
                let ally = &mut allies[ally_index];
                ally.regen(I24F8::from_bits(ally.hp_construction_rate * FRAME_SKIP));
                return true;
            }
        }
        false
    }
}

struct Healer;

impl Script for Healer {
    fn simulate(
        &mut self,
        agent_index: usize,
        allies: &mut [Agent],
        enemies: &mut [Agent],
    ) -> bool {
        let agent = &allies[agent_index];
        if agent.energy < 0 {
            return true;
        }
        let mut selected_ally = None;
        let mut selected_distance_squared = std::i32::MAX;

        if let Some((ally_index, ally)) = agent.restore_target.map(|i| (i, &allies[i])) {
            if ally.is_alive && !ally.healed_this_frame && ally.health < ally.max_health {
                let distance_squared = distance_squared(agent, ally);
                if distance_squared <= MEDIC_HEAL_RANGE_SQUARED {
                    selected_ally = Some(ally_index);
                    selected_distance_squared = distance_squared;
                }
            }
        }

        if selected_ally.is_none() {
            for (ally_index, ally) in allies.iter().enumerate().filter(|(ally_index, ally)| {
                ally.is_organic
                    && !ally.is_stasised()
                    && ally.health < ally.max_health
                    && !ally.healed_this_frame
                    && *ally_index == agent_index
            }) {
                let distance_squared = distance_squared(agent, ally);
                if distance_squared < selected_distance_squared {
                    selected_distance_squared = distance_squared;
                    selected_ally = Some(ally_index);
                }

                // If we can heal it this frame, we're done searching
                if selected_distance_squared <= MEDIC_HEAL_RANGE_SQUARED {
                    break;
                }
            }
        }
        let ally_pos = selected_ally.map(|i| (allies[i].x, allies[i].y));
        let agent = &mut allies[agent_index];
        agent.restore_target = selected_ally;

        if let Some(ally_index) = selected_ally {
            move_toward(
                agent,
                ally_pos.unwrap(),
                (selected_distance_squared as f32).sqrt(),
                0,
            );
            if selected_distance_squared > MEDIC_HEAL_RANGE_SQUARED {
                agent.consume_energy(I24F8::from_num(FRAME_SKIP));
                let ally = &mut allies[ally_index];
                ally.healed_this_frame = true;
                ally.regen(I24F8::from_bits(150 * FRAME_SKIP));
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Copy)]
pub struct Retreater;

impl Script for Retreater {
    fn simulate(
        &mut self,
        agent_index: usize,
        allies: &mut [Agent],
        enemies: &mut [Agent],
    ) -> bool {
        let mut selected_enemy: Option<usize> = None;
        let mut selected_distance_squared: i32 = std::i32::MAX;
        let agent = &mut allies[agent_index];
        if agent.speed == 0.0 {
            return Attacker {}.simulate(agent_index, allies, enemies);
        }
        let mut selected_weapon = &agent.ground_weapon;

        if let Some(target_index) = agent.attack_target {
            let enemy = &enemies[target_index];
            if enemy.health > 0 {
                let distance_squared = distance_squared(agent, enemy);
                selected_weapon = enemy.weapon_vs(agent);
                if distance_squared >= selected_weapon.min_range_squared
                    && distance_squared <= selected_weapon.max_range_squared
                {
                    selected_enemy = Some(target_index);
                    selected_distance_squared = distance_squared;
                }
            }
        }

        if selected_enemy.is_none() {
            for (i, enemy) in enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0 && e.detected && !e.is_stasised())
            {
                let weapon = enemy.weapon_vs(agent);
                if weapon.damage == 0 {
                    continue;
                }
                let distance_squared = distance_squared(agent, enemy);
                if distance_squared >= weapon.min_range_squared
                    && distance_squared < selected_distance_squared
                {
                    selected_distance_squared = distance_squared;
                    selected_enemy = Some(i);
                    selected_weapon = weapon;

                    // If it can hit us "now", we stop searching
                    if selected_distance_squared <= weapon.max_range_squared
                        && enemy.attack_target_priority == TargetingPriority::Highest
                    {
                        break;
                    }
                }
            }
        }

        if selected_enemy.is_none() {
            return false;
        }
        return flee(agent, enemies);
    }
}

#[derive(Clone, Copy, Default)]
pub struct Attacker {}

impl Attacker {
    pub fn new() -> Self {
        Self::default()
    }
}

fn distance_squared(a: &Agent, b: &Agent) -> i32 {
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
}

impl Script for Attacker {
    fn simulate(
        &mut self,
        agent_index: usize,
        allies: &mut [Agent],
        enemies: &mut [Agent],
    ) -> bool {
        let mut selected_enemy: Option<usize> = None;
        let mut selected_distance_squared: i32 = std::i32::MAX;
        let agent = &mut allies[agent_index];
        let mut selected_weapon = &agent.ground_weapon;

        if let Some(target_index) = agent.attack_target {
            let enemy = &enemies[target_index];
            if enemy.health > 0 {
                let distance_squared = distance_squared(agent, enemy);
                selected_weapon = agent.weapon_vs(enemy);
                if distance_squared >= selected_weapon.min_range_squared
                    && distance_squared <= selected_weapon.max_range_squared
                {
                    selected_enemy = Some(target_index);
                    selected_distance_squared = distance_squared;
                }
            }
        }

        if selected_enemy.is_none() {
            for (i, enemy) in enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0 && e.detected && !e.is_stasised())
            {
                let weapon = agent.weapon_vs(enemy);
                if weapon.damage == 0 {
                    continue;
                }
                // No enemy selected, or the current one has a higher priority?
                let relative_prio = selected_enemy
                    .map(|i| {
                        enemy
                            .attack_target_priority
                            .cmp(&enemies[i].attack_target_priority)
                    })
                    .unwrap_or(Ordering::Greater);
                if relative_prio == Ordering::Less {
                    continue;
                }
                let distance_squared = distance_squared(agent, enemy);
                if distance_squared >= weapon.min_range_squared
                    && (distance_squared < selected_distance_squared
                        || relative_prio == Ordering::Greater)
                {
                    selected_distance_squared = distance_squared;
                    selected_enemy = Some(i);
                    selected_weapon = weapon;

                    // If we can hit it "now", and it has a high priority, we stop searching
                    if selected_distance_squared <= weapon.max_range_squared
                        && enemy.attack_target_priority == TargetingPriority::Highest
                    {
                        break;
                    }
                }
            }
        }

        let weapon = *selected_weapon;
        agent.attack_target = selected_enemy;
        // eprintln!(
        //     "{:?} {} {}",
        //     agent.unit_type, selected_distance_squared, weapon.max_range_squared
        // );
        if selected_enemy.is_none() {
            return flee(agent, enemies);
        }
        let selected_enemy = selected_enemy.unwrap();

        if selected_distance_squared <= weapon.max_range_squared {
            if agent.burrowed_attacker != agent.burrowed {
                if agent.burrowed {
                    return agent.unburrow();
                } else {
                    return agent.burrow();
                }
            }
            if agent.cooldown <= 0 {
                if agent.can_stim && !agent.is_stimmed() && agent.health > agent.max_health / 2 {
                    agent.stim();
                }
                attack(agent_index, allies, enemies, weapon, selected_enemy);
                return true;
            }
        }

        combat_move(
            agent,
            &enemies[selected_enemy],
            selected_distance_squared,
            &weapon,
        );
        true
    }
}

fn flee(agent: &mut Agent, enemies: &[Agent]) -> bool {
    if agent.burrowed {
        return agent.unburrow();
    }

    let mut selected_enemy = None;
    let mut selected_distance_squared = std::i32::MAX;
    for enemy in enemies {
        let weapon = enemy.weapon_vs(agent);
        if enemy.is_alive && weapon.damage > 0 {
            let distance_squared = distance_squared(agent, enemy);
            if distance_squared >= weapon.min_range_squared
                && distance_squared < selected_distance_squared
            {
                selected_distance_squared = distance_squared;
                selected_enemy = Some(enemy);
            }

            // If the enemy can hit us this frame, we're done searching
            if selected_distance_squared <= weapon.max_range_squared {
                break;
            }
        }
    }

    if let Some(enemy) = selected_enemy {
        move_away_from(
            agent,
            enemy,
            (selected_distance_squared as f32).sqrt(),
            9999,
        );
        return true;
    }
    false
}

fn combat_move(agent: &mut Agent, enemy: &Agent, distance_squared: i32, weapon: &Weapon) {
    let enemy_weapon = enemy.weapon_vs(agent);
    let should_kite = agent.is_kiter
        && agent.cooldown > 0
        && enemy_weapon.max_range_squared <= distance_squared
        && enemy.speed < agent.speed;
    let distance = (distance_squared as f32).sqrt();
    if should_kite {
        move_away_from(agent, enemy, distance, weapon.max_range);
    } else if distance_squared > weapon.max_range_squared || enemy_weapon.min_range_squared > 0 {
        move_toward(agent, (enemy.x, enemy.y), distance, weapon.max_range);
    }
}

fn move_toward(agent: &mut Agent, goal: (i32, i32), actual_distance: f32, wanted_distance: i32) {
    // Try to get a bit closer to prevent being out of range due to float <-> int conversions
    let wanted_distance = wanted_distance - 2;
    let max_reach = (FRAME_SKIP as f32 * agent.speed)
        .min(actual_distance - wanted_distance as f32)
        .max(0.0);
    // dbg!(agent.unit_type, actual_distance, wanted_distance, max_reach);
    if actual_distance == 0.0 {
        // Poor mans random
        let rnd = (agent as *mut Agent as usize) as f32;
        agent.vx = (rnd.cos() * max_reach) as i32;
        agent.vy = (rnd.sin() * max_reach) as i32;
    } else {
        agent.vx = ((goal.0 - agent.x) as f32 * max_reach / actual_distance) as i32;
        agent.vy = ((goal.1 - agent.y) as f32 * max_reach / actual_distance) as i32;
    }
}

fn move_away_from(agent: &mut Agent, enemy: &Agent, enemy_distance: f32, target_distance: i32) {
    // Try to get a bit farther away to prevent being out of range due to float <-> int conversions
    let target_distance = target_distance + 2;
    let max_reach = (FRAME_SKIP as f32 * agent.speed)
        .min(target_distance as f32 - enemy_distance)
        .max(0.0);
    if enemy_distance == 0.0 {
        // Poor mans random
        let rnd = (agent as *mut Agent as usize) as f32;
        agent.vx = (rnd.cos() * max_reach) as i32;
        agent.vy = (rnd.sin() * max_reach) as i32;
    } else {
        agent.vx = ((agent.x - enemy.x) as f32 * max_reach / enemy_distance) as i32;
        agent.vy = ((agent.y - enemy.y) as f32 * max_reach / enemy_distance) as i32;
    }
}

fn attack(
    agent_index: usize,
    allies: &mut [Agent],
    enemies: &mut [Agent],
    weapon: Weapon,
    enemy_index: usize,
) {
    let mut agent = &mut allies[agent_index];
    agent.sleep_timer = agent.stop_frames;

    // Update cooldown
    agent.cooldown = weapon.cooldown;
    let mut m = 0;
    if agent.stim_timer > 0 {
        m += 1
    }
    if agent.cooldown_upgrade {
        m += 1
    }
    if agent.ensnare_timer > 0 {
        m -= 1
    }
    if m < 0 {
        agent.cooldown = 5.max(agent.cooldown * 5 / 4);
    } else if m > 0 {
        agent.cooldown /= 2;
    }
    deal_direct_damage(agent, &weapon, &mut enemies[enemy_index]);
    match weapon.splash_type {
        SplashType::Bounce => deal_bounce_damage(&weapon, enemy_index, allies, enemies),
        SplashType::RadialSplash => {
            deal_radial_splash_damage(&weapon, enemy_index, allies, enemies)
        }
        SplashType::RadialEnemySplash => {
            deal_radial_enemy_splash_damage(&weapon, enemy_index, enemies)
        }
        SplashType::LineSplash => deal_line_splash(agent, &weapon, enemy_index, enemies),
        _ => (), // No splash
    }
}

fn deal_line_splash(source: &Agent, weapon: &Weapon, enemy_index: usize, enemies: &mut [Agent]) {
    let (left, main_target, right) = split_at_mut_ex(enemies, enemy_index);
    let mut dx = main_target.x - source.x;
    let dy = main_target.y - source.y;

    if dx == 0 && dy == 0 {
        // This should hardly happen at all, but if it does, just fire "somewhere"
        dx = 1;
    }

    let delta_squared = dx * dx + dy * dy;
    let range_with_splash_squared = weapon.max_range_squared
        + 2 * weapon.max_range * weapon.inner_splash_radius
        + weapon.inner_splash_radius_squared;
    for enemy in left.into_iter().chain(right) {
        if enemy.is_flyer != main_target.is_flyer {
            continue;
        }

        let enemy_dist_squared = distance_squared(enemy, source);
        if enemy_dist_squared <= range_with_splash_squared {
            let dot = (enemy.x - source.x) * dx + (enemy.y - source.y) * dy;
            if dot >= 0 {
                let proj_dx = source.x + dot * dx / delta_squared - enemy.x;
                let proj_dy = source.y + dot * dy / delta_squared - enemy.y;
                let proj_delta_squared = proj_dx * proj_dx + proj_dy * proj_dy;
                if proj_delta_squared <= weapon.inner_splash_radius_squared {
                    apply_damage(enemy, weapon.damage_type, weapon.damage, weapon.hits);
                }
            }
        }
    }
}

fn deal_radial_splash_damage(
    weapon: &Weapon,
    enemy_index: usize,
    allies: &mut [Agent],
    enemies: &mut [Agent],
) {
    let main_target = &enemies[enemy_index];
    for ally in allies {
        apply_splash_damage(weapon, main_target, ally);
    }
    deal_radial_enemy_splash_damage(weapon, enemy_index, enemies);
}

fn deal_radial_enemy_splash_damage(weapon: &Weapon, enemy_index: usize, enemies: &mut [Agent]) {
    let (left, main_target, right) = split_at_mut_ex(enemies, enemy_index);
    for enemy in left.into_iter().chain(right) {
        apply_splash_damage(weapon, main_target, enemy);
    }
}

fn split_at_mut_ex<T>(slice: &mut [T], index: usize) -> (&mut [T], &mut T, &mut [T]) {
    let (left, right) = slice.split_at_mut(index);
    let (pivot, right) = right.split_at_mut(1);
    (left, &mut pivot[0], right)
}

fn apply_splash_damage(weapon: &Weapon, main_target: &Agent, splash_target: &mut Agent) {
    if splash_target.is_flyer != main_target.is_flyer {
        return;
    }
    let distance_squared = distance_squared(splash_target, main_target);
    if distance_squared <= weapon.inner_splash_radius_squared {
        apply_damage(
            splash_target,
            weapon.damage_type,
            weapon.damage,
            weapon.hits,
        );
    } else if !splash_target.burrowed {
        if distance_squared <= weapon.median_splash_radius_squared {
            apply_damage(
                splash_target,
                weapon.damage_type,
                weapon.damage / 2,
                weapon.hits,
            );
        } else if distance_squared <= weapon.outer_splash_radius_squared {
            apply_damage(
                splash_target,
                weapon.damage_type,
                weapon.damage / 4,
                weapon.hits,
            );
        }
    }
}

fn deal_bounce_damage(
    weapon: &Weapon,
    enemy_index: usize,
    allies: &mut [Agent],
    enemies: &mut [Agent],
) {
    let mut remaining_bounces = 2;
    let mut damage = weapon.damage;
    let mut last_target = (enemies[enemy_index].x, enemies[enemy_index].y);
    for enemy in enemies {
        let dx = (enemy.x - last_target.0).abs();
        let dy = (enemy.y - last_target.1).abs();
        if enemy.is_alive && dx <= 96 && dy <= 96 && (dx > 0 || dy > 0) {
            last_target = (enemy.x, enemy.y);
            damage /= 3;
            apply_damage(enemy, weapon.damage_type, damage, weapon.hits);
            if remaining_bounces > 0 {
                remaining_bounces -= 1;
            } else {
                break;
            }
        }
    }
}

fn deal_direct_damage(agent: &mut Agent, weapon: &Weapon, target: &mut Agent) {
    let mut remaining_damage = weapon.damage;

    if !agent.is_melee {
        // https://liquipedia.net/starcraft/Dark_Swarm
        if target.protected_by_dark_swarm {
            return;
        }

        // http://www.starcraftai.com/wiki/Chance_to_Hit
        if agent.elevation_level >= 0 && agent.elevation_level < target.elevation_level
            || target.elevation_level & 1 == 1
        {
            remaining_damage = remaining_damage * I24F8::from_bits(136);
        }
        remaining_damage = remaining_damage * I24F8::from_bits(255);
    }

    agent.attack_counter += 1;
    apply_damage(target, weapon.damage_type, remaining_damage, weapon.hits);
}

fn apply_damage(target: &mut Agent, damage_type: DamageType, mut damage: I24F8, hits: i32) {
    // Shields can go over max value, so fix here
    let shields = target.max_shields.min(target.shields)
        - (damage - I24F8::from_num(target.shield_upgrades)).max(I24F8::ZERO);

    if shields > 0 {
        target.shields = shields;
        return;
    } else if shields < 0 {
        damage = -shields;
        target.shields = I24F8::ZERO;
    }

    if damage == 0 {
        return;
    }
    damage = reduce_damage_by_target_size_and_damage_type(
        target,
        damage_type,
        damage - target.armor * I24F8::from_num(hits),
    );
    target.consume_health(damage.max(I24F8::from_bits(128)));
}

fn reduce_damage_by_target_size_and_damage_type(
    target: &mut Agent,
    damage_type: DamageType,
    damage: I24F8,
) -> I24F8 {
    match (damage_type, target.size) {
        (DamageType::Concussive, UnitSize::Medium) | (DamageType::Explosive, UnitSize::Small) => {
            damage / 2
        }
        (DamageType::Concussive, UnitSize::Large) | (DamageType::Explosive, UnitSize::Medium) => {
            damage / 4
        }
        _ => damage,
    }
}

#[derive(Clone)]
pub struct Simulator<A, B, W> {
    pub player_a: Player<A>,
    pub player_b: Player<B>,
    pub walkability: W,
}

#[derive(Default, Clone)]
pub struct Player<S> {
    pub agents: Vec<Agent>,
    pub script: S,
}

impl<A: Script, B: Script, W: Fn(i32, i32) -> bool> Simulator<A, B, W> {
    pub fn simulate_for(&mut self, mut frames: i32) -> i32 {
        while frames != 0 {
            // dbg!(frames);
            frames -= FRAME_SKIP;
            if !self.step() || frames < -500 {
                break;
            }
        }
        frames
    }

    fn step(&mut self) -> bool {
        let running_a = self.player_a.step(&mut self.player_b.agents);
        let running_b = self.player_b.step(&mut self.player_a.agents);
        self.player_a.update_stats(&self.walkability);
        self.player_b.update_stats(&self.walkability);
        running_a || running_b
    }
}

impl<S: Script> Player<S> {
    fn step(&mut self, enemies: &mut [Agent]) -> bool {
        let mut running = false;
        for i in 0..self.agents.len() {
            let agent = &self.agents[i];
            if !agent.is_alive {
                continue;
            }
            running |=
                agent.is_stasised() || agent.is_sleeping() || self.simulate_agent(i, enemies);
        }
        running
    }

    fn update_stats(&mut self, walkability: impl Fn(i32, i32) -> bool) {
        for agent in self.agents.iter_mut().filter(|it| it.is_alive) {
            // eprintln!(
            //     "{:?} - ({}, {}) + ({}, {}) - hp: {} shields: {}",
            //     agent.unit_type,
            //     agent.x,
            //     agent.y,
            //     agent.vx,
            //     agent.vy,
            //     agent.health(),
            //     agent.shields()
            // );
            agent.update_position(&walkability);
            agent.vx = 0;
            agent.vy = 0;
            agent.healed_this_frame = false;

            agent.is_alive &= agent.health > 0;
            agent.health = (agent.health - agent.plague_damage_per_frame * FRAME_SKIP)
                .max(I24F8::from_bits(1));
            agent.health += agent.health_regen * I24F8::from_num(FRAME_SKIP);

            // All these values can go below 0, which won't matter
            agent.sleep_timer -= FRAME_SKIP;
            agent.stasis_timer -= FRAME_SKIP;
            agent.cooldown -= FRAME_SKIP;
            agent.shields += I24F8::from_bits(7 * FRAME_SKIP);
            agent.energy += I24F8::from_bits(8 * FRAME_SKIP);
            agent.stim_timer -= FRAME_SKIP;
            agent.ensnare_timer -= FRAME_SKIP;
        }
    }

    fn simulate_agent(&mut self, agent_index: usize, enemies: &mut [Agent]) -> bool {
        self.agents[agent_index].update_speed();
        self.script.simulate(agent_index, &mut self.agents, enemies)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    impl Script for () {
        fn simulate(
            &mut self,
            agent_index: usize,
            allies: &mut [Agent],
            enemies: &mut [Agent],
        ) -> bool {
            true
        }
    }

    #[test]
    fn test_stats_update() {
        let mut player: Player<()> = Player::default();
        player.agents.push(Agent {
            is_alive: true,
            ..Default::default()
        });
        player.update_stats(|x, y| true);
        assert_eq!(player.agents[0].energy, I24F8::from_num(0.03));
    }

    #[test]
    fn move_away_test() {
        let mut agent = Agent {
            speed: 5.0,
            ..Default::default()
        };
        let enemy = Agent::default();
        move_away_from(&mut agent, &enemy, 0.0, 8);
        assert!(agent.vx * agent.vx + agent.vy * agent.vy <= 5 * 5);
        assert!(agent.vx * agent.vx + agent.vy * agent.vy >= 4 * 4);

        let enemy = Agent {
            x: 2,
            y: 0,
            ..Default::default()
        };
        move_away_from(&mut agent, &enemy, 2.0, 8);
        assert_eq!(agent.vx, -5);
        assert_eq!(agent.vy, 0);

        let enemy = Agent {
            x: -7,
            y: 0,
            ..Default::default()
        };
        move_away_from(&mut agent, &enemy, 7.0, 8);
        assert!(agent.vx >= 1);
        assert_eq!(agent.vy, 0);
    }

    #[test]
    fn archon_splash_should_not_affect_own_units() {
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![
                    UnitType::Protoss_Archon.into(),
                    Agent::from(UnitType::Protoss_Zealot).with_x(48),
                ],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Zerg_Zergling).with_x(48)],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        simulator.simulate_for(1);

        assert_eq!(
            simulator.player_a.agents[1].shields,
            I24F8::from_bits(14087)
        );
    }

    #[test]
    fn zealot_should_kill_ling() {
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![UnitType::Zerg_Zergling.into()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Protoss_Zealot)],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(128);

        eprintln!("Finished in {} frames", 128 - frames);
        assert_eq!(simulator.player_a.agents[0].is_alive, false);
        assert_eq!(simulator.player_b.agents[0].is_alive, true);
    }

    #[test]
    fn buildings() {
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![UnitType::Zerg_Zergling.into()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Protoss_Pylon)],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(96);

        assert_eq!(simulator.player_a.agents[0].is_alive, true);
        assert_eq!(simulator.player_b.agents[0].is_alive, true);
    }

    #[test]
    fn no_combat() {
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![UnitType::Zerg_Overlord.into()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Protoss_Probe)],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(128);

        assert_eq!(simulator.player_a.agents[0].is_alive, true);
        assert_eq!(simulator.player_b.agents[0].is_alive, true);
    }

    #[test]
    fn lings_vs_probes() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_y(76).with_x(246);
        let probe = Agent::from(UnitType::Protoss_Probe).with_y(107).with_x(241);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone(), ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![probe.clone()],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(8 * 24);

        eprintln!(
            "{}",
            simulator
                .player_b
                .agents
                .iter()
                .map(|u| format!(
                    "{:?}, alive: {} h:{}, s: {}\n",
                    u.unit_type,
                    u.is_alive,
                    u.health(),
                    u.shields()
                )
                .split_once('_')
                .unwrap()
                .1
                .to_string())
                .collect::<String>()
        );

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            0
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            2
        );
    }
    #[test]
    fn lings_vs_probes_for_2_seconds() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(200);
        let probe = Agent::from(UnitType::Protoss_Probe);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone(), ling.clone(), ling.clone(), ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![probe.clone(), probe.clone(), probe.clone()],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(55);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            1
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            4
        );
    }

    #[test]
    fn lings_vs_sunkens_and_non_combat() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(200);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone(), ling.clone(), ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![
                    Agent::from(UnitType::Zerg_Hatchery)
                        .with_x(200)
                        .with_y(-100),
                    Agent::from(UnitType::Zerg_Sunken_Colony),
                ],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(256);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            2
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            0
        );
    }

    #[test]
    fn lings_vs_sunkens_and_lings() {
        let ling = Agent::from(UnitType::Zerg_Zergling);
        let sunken = Agent::from(UnitType::Zerg_Sunken_Colony).with_x(400);
        let mut simulator = Simulator {
            player_a: Player {
                agents: (0..11).map(|_| ling.clone()).collect(),
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![
                    sunken.clone(),
                    sunken.clone(),
                    sunken.clone(),
                    sunken.clone(),
                    ling.clone().with_x(370),
                    ling.clone().with_x(370),
                    ling.clone().with_x(370),
                    ling.clone().with_x(370),
                    ling.clone().with_x(370),
                    ling.clone().with_x(370),
                ],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(8 * 24);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            4
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            2
        );
    }

    #[test]
    fn lings_vs_sunkens() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(200);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Zerg_Sunken_Colony)],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(128);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            1
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            0
        );
    }

    #[test]
    fn should_attack_higher_prio_target() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(200);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone(), ling.clone(), ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![
                    Agent::from(UnitType::Protoss_Pylon)
                        .with_x(180)
                        .with_target_priority(TargetingPriority::Medium),
                    Agent::from(UnitType::Protoss_Zealot),
                ],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(192);
        eprintln!("{frames}");

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            1
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            2
        );
    }

    #[test]
    fn ten_lings_kill_3_zealots() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(200);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                    ling.clone(),
                ],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![
                    Agent::from(UnitType::Protoss_Zealot),
                    Agent::from(UnitType::Protoss_Zealot),
                    Agent::from(UnitType::Protoss_Zealot),
                ],
                script: Attacker::new(),
            },
            walkability: |x, y| true,
        };

        let frames = simulator.simulate_for(128);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            0
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            7
        );
    }

    #[test]
    fn slow_move_no_kill() {
        let ling = Agent::from(UnitType::Zerg_Zergling).with_x(64);
        let mut simulator = Simulator {
            player_a: Player {
                agents: vec![ling.clone()],
                script: Attacker::new(),
            },
            player_b: Player {
                agents: vec![Agent::from(UnitType::Protoss_Zealot)],
                script: Attacker::new(),
            },
            walkability: |x, y| false,
        };

        let frames = simulator.simulate_for(52);

        assert_eq!(
            simulator
                .player_b
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            1
        );
        assert_eq!(
            simulator
                .player_a
                .agents
                .iter()
                .filter(|u| u.is_alive)
                .count(),
            1
        );
    }
}
