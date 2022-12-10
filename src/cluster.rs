use crate::sunit::SUnit;
use ahash::*;
use rsbwapi::Position;
use rstar::{RTree, RTreeObject, AABB};

const NOISE: usize = std::usize::MAX;

pub trait WithPosition {
    fn position(&self) -> Position;
}

pub fn dbscan(elements: &RTree<SUnit>, eps: i32, min_pts: usize) -> Vec<Cluster> {
    for x in elements.iter() {
        let dim = x.dimensions();
        assert!(dim.tl.x >= 0 && dim.br.x >= 0);
        assert!(dim.tl.y >= 0 && dim.br.y >= 0);
        assert!(dim.br.x > dim.tl.x);
        assert!(dim.br.y > dim.tl.y);
        assert!(dim.br.x - dim.tl.x < 164);
        assert!(dim.br.y - dim.tl.y < 164);
    }
    let (labeled, max_cluster) = label_items(elements, eps, min_pts);
    post_process_clusters(labeled, max_cluster)
}

fn label_items<
    'a,
    T: RTreeObject<Envelope = AABB<[i32; 2]>> + core::hash::Hash + core::cmp::Eq + WithPosition,
>(
    elements: &'a RTree<T>,
    eps: i32,
    min_pts: usize,
) -> (AHashMap<&'a T, usize>, usize) {
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
                    neighbors.extend(new_neighbors);
                }
            }
        }
    }
    // "Noise" still are units we should command
    for e in elements {
        if label.get(e).unwrap() == &NOISE {
            c += 1;
            label.insert(e, c);
        }
    }
    (label, c)
}

fn post_process_clusters<'a>(
    mut labeled: AHashMap<&'a SUnit, usize>,
    max_cluster: usize,
) -> Vec<Cluster> {
    let mut clusters = vec![Cluster::default(); max_cluster];
    for (k, v) in labeled.drain() {
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

#[derive(Clone, Debug)]
pub struct Cluster<T = SUnit> {
    pub units: Vec<T>,
    pub b: f32,
}

impl<T> Default for Cluster<T> {
    fn default() -> Self {
        Self {
            units: vec![],
            b: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Hash, PartialEq, Eq, Debug)]
    struct TestEntity(Position);

    impl WithPosition for TestEntity {
        fn position(&self) -> Position {
            self.0
        }
    }

    impl RTreeObject for TestEntity {
        type Envelope = AABB<[i32; 2]>;

        fn envelope(&self) -> Self::Envelope {
            AABB::from_corners(
                [self.0.x - 10, self.0.y - 10],
                [self.0.x + 10, self.0.y + 10],
            )
        }
    }

    #[test]
    fn function_name_test() {
        let mut rnd = oorandom::Rand32::new(1);
        for _ in 0..1000 {
            let mut test_entities: RTree<TestEntity> = RTree::new();
            for _ in 0..rnd.rand_range(20..40) {
                test_entities.insert(TestEntity(Position::new(
                    100 + rnd.rand_range(0..300) as i32,
                    0 + rnd.rand_range(0..300) as i32,
                )));
            }
            for _ in 0..rnd.rand_range(20..40) {
                test_entities.insert(TestEntity(Position::new(
                    300 + rnd.rand_range(0..300) as i32,
                    1000 + rnd.rand_range(0..300) as i32,
                )));
            }

            let (labels, max_cluster) = label_items(&test_entities, 400, 4);
            assert_eq!(max_cluster, 2, "{:?}", labels.keys());
        }
    }
}
