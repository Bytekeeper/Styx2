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
    clusters
}

#[derive(Clone, Default, Debug)]
pub struct Cluster {
    pub units: Vec<SUnit>,
}
