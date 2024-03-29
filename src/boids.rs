use crate::cherry_vis::*;
use crate::cluster::WithPosition;
use crate::config::*;
use crate::{MyModule, SUnit};
use glam::Vec2;
use ordered_float::OrderedFloat;
use rsbwapi::sma::Altitude;
use rsbwapi::{Position, Rectangle};

#[must_use]
#[derive(Debug)]
pub struct WeightedPosition {
    pub weight: f32,
    pub position: Vec2,
}

impl WeightedPosition {
    pub const ZERO: Self = WeightedPosition {
        weight: 0.0,
        position: Vec2::ZERO,
    };
}

impl MyModule {
    pub fn positioning(&self, unit: &SUnit, requests: &[WeightedPosition]) -> Option<Position> {
        let mut aggregated_position = Vec2::ZERO;
        let mut aggregated_weight = 0.0;
        for &WeightedPosition { weight, position } in requests {
            aggregated_weight += weight;
            aggregated_position += position * weight;
        }
        if DRAW_FORCE_VECTORS {
            cvis().draw_text(
                unit.position().x,
                unit.position().y,
                format!("{:.2} {}", aggregated_weight, aggregated_position),
            );
        }
        if aggregated_weight < 0.0001 {
            return Some(unit.position());
        }
        aggregated_position /= aggregated_weight;
        aggregated_position = aggregated_position.clamp_length_max(unit.top_speed() as f32 * 11.0);
        // dbg!(aggregated_position, aggregated_weight);

        // If we somehow end up "not moving at all" - use the largest weighted position (TODO Probably
        // should use more than just that)
        let delta_position = if aggregated_position.length_squared() < 1.0 {
            let max_weight = requests
                .iter()
                .max_by_key(|wp| OrderedFloat(wp.weight))
                .unwrap()
                .position;
            (max_weight.x.round() as i32, max_weight.y.round() as i32)
        } else {
            (
                aggregated_position.x.round() as i32,
                aggregated_position.y.round() as i32,
            )
        };
        let target_position = unit.position() + delta_position;
        let result = if unit.flying() {
            target_position
        } else {
            let delta_position = pos_to_vec2(Position::new(delta_position.0, delta_position.1));
            [
                delta_position,
                delta_position.perp(),
                delta_position.perp() * -1.0,
            ]
            .into_iter()
            .map(|p| Position::new(p.x as i32, p.y as i32))
            .flat_map(|d| self.furthest_walkable_position(unit, unit.position() + d))
            .min_by_key(|pos| target_position.to_walk_position().distance_squared(*pos))
            .map(|pos| {
                if pos == target_position.to_walk_position() {
                    target_position
                } else {
                    pos.center()
                }
            })
            .unwrap_or(unit.position())
        };
        if DRAW_FORCE_VECTORS {
            cvis().draw_line(
                target_position.x,
                target_position.y,
                unit.position().x,
                unit.position().y,
                rsbwapi::Color::Blue,
            );
            cvis().draw_line(
                unit.position().x,
                unit.position().y,
                result.x,
                result.y,
                rsbwapi::Color::Green,
            );
            cvis().draw_circle(result.x, result.y, 8, rsbwapi::Color::Green);
        }
        if result.distance_squared(unit.position()) > 16 * 16
            || result.distance_squared(target_position) < 8 * 8
        {
            Some(result)
        } else {
            None
        }
    }
}

pub fn avoid(unit: &SUnit, pos: Position, minimum_distance: f32, weight: f32) -> WeightedPosition {
    let unit = pos_to_vec2(unit.position());
    let delta = unit - pos_to_vec2(pos);
    let dist = delta.length();
    if dist > minimum_distance {
        return WeightedPosition::ZERO;
    }
    // "Push" more if we're closer
    let scale = 1.0 - dist / minimum_distance;
    let position = delta * (minimum_distance - dist) / dist;
    if DRAW_FORCE_VECTORS {
        cvis().draw_line(
            unit.x as i32,
            unit.y as i32,
            (unit.x + position.x) as i32,
            (unit.y + position.y) as i32,
            rsbwapi::Color::Red,
        );
    }
    WeightedPosition {
        position,
        weight: weight * scale,
    }
}

pub fn separation(
    unit: &SUnit,
    other: &SUnit,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    let rnd = (unit as *const SUnit as usize) as f32;
    let unit = pos_to_vec2(unit.position());
    let other = pos_to_vec2(other.position());
    let dist = unit.distance(other);
    if dist >= minimum_distance || other == unit {
        WeightedPosition::ZERO
    } else if dist == 0.0 {
        // Poor mans random
        WeightedPosition {
            position: Vec2::from_angle(rnd) * minimum_distance,
            weight,
        }
    } else {
        // "Push" more if we're closer
        let scale = 1.0 - dist / minimum_distance;
        let position = (unit - other) * (minimum_distance - dist) / dist;
        if DRAW_FORCE_VECTORS {
            cvis().draw_line(
                unit.x as i32,
                unit.y as i32,
                (unit.x + position.x) as i32,
                (unit.y + position.y) as i32,
                rsbwapi::Color::Red,
            );
        }
        WeightedPosition {
            position,
            weight: weight * scale,
        }
    }
}

pub fn cohesion(
    unit: &SUnit,
    other: &SUnit,
    prediction_frames: i32,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    goal(
        unit,
        other.predict_position(prediction_frames),
        minimum_distance,
        weight,
    )
}

pub fn goal(
    unit: &SUnit,
    target: Position,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    let target = pos_to_vec2(target);
    let unit = pos_to_vec2(unit.position());
    let dist = target.distance(unit);
    if dist == 0.0 {
        return WeightedPosition::ZERO;
    }
    let scale = 1.0 - dist / minimum_distance;
    WeightedPosition {
        position: target - unit,
        weight,
    }
}

pub fn climb(
    module: &MyModule,
    unit: &SUnit,
    range: i32,
    max_altitude: i16,
    weight: f32,
) -> WeightedPosition {
    let pos = unit.position().to_walk_position();
    let current_altitude = match module.map.get_altitude(pos) {
        Altitude::Walkable(x) => x,
        _ => 0,
    };
    if current_altitude > max_altitude {
        return WeightedPosition::ZERO;
    }
    let range = (range + 7) / 8;
    let highest_nearby_position = Rectangle::new(pos - (range, range), pos + (range, range))
        .into_iter()
        .filter(|wp| wp.is_valid(&&module.game))
        .max_by_key(|wp| match module.map.get_altitude(*wp) {
            Altitude::Walkable(x) => x as u32 * 1000000 + wp.distance_squared(pos),
            _ => 0,
        })
        .unwrap();
    let scale = (max_altitude - current_altitude) as f32 / max_altitude as f32;
    if DRAW_FORCE_VECTORS {
        cvis().draw_line(
            unit.position().x,
            unit.position().y,
            highest_nearby_position.center().x,
            highest_nearby_position.center().y,
            rsbwapi::Color::Brown,
        );
    }
    WeightedPosition {
        position: pos_to_vec2(highest_nearby_position.center() - unit.position()),
        weight: weight * scale,
    }
}

pub fn follow_path(
    module: &MyModule,
    unit: &SUnit,
    target: Position,
    weight: f32,
) -> WeightedPosition {
    let pos = unit.position();
    let result = if unit.flying() {
        WeightedPosition {
            weight,
            position: pos_to_vec2(target - pos),
        }
    } else {
        let path = module.map.get_path(unit.position(), target);
        let mut path_iter = path.0.iter();
        let selected_cp = path
            .0
            .iter()
            .map(|cp| cp.top.center())
            .filter(|pos| pos.distance(unit.position()) >= 63.0)
            .next()
            .unwrap_or(target)
            .to_walk_position();
        let mut target_position = pos.to_walk_position();
        let min_alt = (7 + unit.get_type().width().max(unit.get_type().height())) as i16 / 8;
        for _ in 0..8 {
            let Some(next) = module.altitude_path_next(target_position, selected_cp, min_alt) else { return WeightedPosition::ZERO };
            target_position = next;
        }

        WeightedPosition {
            weight,
            position: pos_to_vec2(target_position.center() - pos),
        }
    };
    if DRAW_FORCE_VECTORS {
        let t = Position {
            x: result.position.x as i32,
            y: result.position.y as i32,
        } + unit.position();
        cvis().draw_line(
            unit.position().x,
            unit.position().y,
            t.x,
            t.y,
            rsbwapi::Color::Cyan,
        );
    }
    result
}

pub fn pos_to_vec2(pos: Position) -> Vec2 {
    Vec2::new(pos.x as f32, pos.y as f32)
}
