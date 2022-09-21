use crate::sunit::SUnit;
use ahash::*;
use rstar::{RTree, RTreeObject, AABB};

const NOISE: usize = std::usize::MAX;

pub fn dbscan(elements: &RTree<SUnit>, eps: i32, min_pts: usize) -> Vec<Cluster> {
    let mut label = AHashMap::new();
    let mut c = 0;
    for element in elements {
        if label.contains_key(&element) {
            continue;
        }
        let envelope = element.envelope();
        let (lower, upper) = (envelope.lower(), envelope.upper());
        let mut neighbors: Vec<_> = elements
            .locate_in_envelope_intersecting(&AABB::from_corners(
                [lower[0] - eps, lower[1] - eps],
                [upper[0] + eps, upper[1] + eps],
            ))
            // TODO Omitted distance check here (same below) - is it really required for our use
            // case?
            .filter(|u| u != &element)
            .collect();
        // -1 because element is already filtered out
        if neighbors.len() < min_pts - 1 {
            label.insert(element, NOISE);
            continue;
        }
        c += 1;
        label.insert(element, c);
        while let Some(q) = neighbors.pop() {
            if label.get(&q).unwrap_or(&NOISE) == &NOISE {
                label.insert(q, c);

                let envelope = q.envelope();
                let (lower, upper) = (envelope.lower(), envelope.upper());
                let new_neighbors: Vec<_> = elements
                    .locate_in_envelope_intersecting(&AABB::from_corners(
                        [lower[0] - eps, lower[1] - eps],
                        [upper[0] + eps, upper[1] + eps],
                    ))
                    .collect();
                if new_neighbors.len() >= min_pts {
                    neighbors.extend(
                        new_neighbors, // TODO maybe we can get away without this?
                                       // .filter(|u| u.position().distance_squared(element.position()) <= eps * eps),
                    );
                }
            }
        }
    }
    for e in elements {
        if label.get(e).unwrap() == &NOISE {
            c += 1;
            label.insert(e, c);
        }
    }
    let mut clusters = vec![Cluster::default(); c];
    for (k, v) in label.drain() {
        clusters[v - 1].units.push(k.clone());
    }
    for c in clusters.iter_mut() {
        c.units.sort_by_key(|u| u.position().x);
        let my_units: Vec<_> = c.units.iter().filter(|u| u.player().is_me()).collect();
        if my_units.is_empty() {
            continue;
        }
        let n = my_units.len();
        let avg_x = my_units.iter().map(|u| u.position().x).sum::<i32>() as f32 / n as f32;
        let avg_y = my_units.iter().map(|u| u.position().y).sum::<i32>() as f32 / n as f32;
        let avg_x2 = my_units
            .iter()
            .map(|u| u.position().x as f32 * u.position().x as f32)
            .sum::<f32>()
            / n as f32;
        let avg_y2 = my_units
            .iter()
            .map(|u| u.position().y as f32 * u.position().y as f32)
            .sum::<f32>()
            / n as f32;
        let s_x = (my_units
            .iter()
            .map(|u| (u.position().x as f32 - avg_x).powf(2.0))
            .sum::<f32>()
            / n as f32)
            .sqrt();
        let s_y = (my_units
            .iter()
            .map(|u| (u.position().y as f32 - avg_y).powf(2.0))
            .sum::<f32>()
            / n as f32)
            .sqrt();
        let avg_xy = my_units
            .iter()
            .map(|u| u.position().x as f32 * u.position().y as f32)
            .sum::<f32>()
            / n as f32;
        let r_xy = (avg_xy - avg_x * avg_y)
            / ((avg_x2 - avg_x.powf(2.0)) * (avg_y2 - avg_y.powf(2.0))).sqrt();
        let b = r_xy * s_y / s_x;
        c.b = b;
    }
    clusters
}

#[derive(Clone, Default, Debug)]
pub struct Cluster {
    pub units: Vec<SUnit>,
    pub b: f32,
}
