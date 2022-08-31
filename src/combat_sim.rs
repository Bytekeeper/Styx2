pub enum SplashType {
    RadialSplash,
    RadialEnemySplash,
    LineSplash,
    Bounce,
    Irrelevant,
}

pub enum DamageType {
    Explosive,
    Concussive,
    Irrelevant,
}

pub enum UnitSize {
    Small,
    Medium,
    Large,
    Irrelevant,
}

pub struct Weapon {
    max_range: i32,
    min_range_squared: i32,
    max_range_squared: i32,
    damage_shifted: i32,
    hits: i32,
    inner_splash_radius: i32,
    inner_splash_radius_squared: i32,
    median_splash_radius_squared: i32,
    outer_splash_radius_squared: i32,
    cooldown: i32,
    damage_type: DamageType,
    splash_type: SplashType,
}

pub struct Agent {
    elevation_level: i32,
    x: i32,
    y: i32,
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
    health_shifted: i32,
    max_health_shifted: i32,
    healed_this_frame: bool,
    stim_timer: i32,
    ensnare_timer: i32,
    hp_construction_rate: i32,
    shields_shifted: i32,
    max_shields_shifted: i32,
    energy_shifted: i32,
    max_energy_shifted: i32,
    attack_counter: i32,
    cooldown: i32,
    cooldown_upgrade: bool,
    sleep_timer: i32,
    stop_frames: i32,
    can_stim: bool,
    plague_damage_per_frame_shifted: i32,
    regeneraties_health: i32,
    is_suicider: bool,
    is_healer: bool,
    is_flyer: bool,
    is_organic: bool,
    is_mechanic: bool,
    is_kiter: bool,
    is_repairer: bool,
    protected_by_dark_swarm: bool,
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
    attack_target: i32,
    restore_target: i32,
    interceptors: Vec<i32>,
}

impl Agent {
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
}
