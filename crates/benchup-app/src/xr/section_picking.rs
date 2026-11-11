//! Retained controller-ray queries for the exact rendered section-cap triangles.
use super::world_picking::Bounds;
use benchup_core::{DVec3, Ray};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub owner: Option<u64>,
    pub point: DVec3,
    pub normal: DVec3,
    pub distance: f64,
}

struct Triangle {
    points: [DVec3; 3],
    owner: Option<u64>,
}

struct Node {
    bounds: Bounds,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}

#[derive(Default)]
pub(crate) struct Index {
    generation: Option<u64>,
    triangles: Vec<Triangle>,
    nodes: Vec<Node>,
}

impl Index {
    pub fn prepare(&mut self, generation: u64, triangles: &[[DVec3; 3]], owners: &[Option<u64>]) {
        if self.generation == Some(generation) {
            return;
        }
        assert_eq!(triangles.len(), owners.len());
        self.generation = Some(generation);
        self.triangles = triangles
            .iter()
            .zip(owners)
            .filter_map(|(points, owner)| {
                ((points[1] - points[0])
                    .cross(points[2] - points[0])
                    .length_squared()
                    > 1e-24)
                    .then_some(Triangle {
                        points: *points,
                        owner: *owner,
                    })
            })
            .collect();
        self.nodes.clear();
        if !self.triangles.is_empty() {
            self.build(0..self.triangles.len());
        }
    }

    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let mut bounds = Bounds::empty();
        let mut lo = DVec3::INFINITY;
        let mut hi = DVec3::NEG_INFINITY;
        for triangle in &self.triangles[range.clone()] {
            for point in triangle.points {
                bounds.include(point);
            }
            let center = (triangle.points[0] + triangle.points[1] + triangle.points[2]) / 3.;
            lo = lo.min(center);
            hi = hi.max(center);
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 8 {
            let size = hi - lo;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let middle = range.len() / 2;
            self.triangles[range.clone()].select_nth_unstable_by(middle, |a, b| {
                let center = |triangle: &Triangle| {
                    triangle.points.iter().map(|point| point[axis]).sum::<f64>()
                };
                center(a).total_cmp(&center(b))
            });
            let split = range.start + middle;
            let a = self.build(range.start..split);
            let b = self.build(split..range.end);
            self.nodes[index].children = Some((a, b));
        }
        index
    }

    pub fn nearest(&self, ray: Ray, limit: f64) -> Option<Hit> {
        if self.nodes.is_empty() || !ray.origin.is_finite() || !ray.direction.is_finite() {
            return None;
        }
        let mut best = None;
        let mut limit = limit;
        // Median splitting bounds traversal depth by the machine word size.
        let mut stack = [0usize; usize::BITS as usize];
        let mut pending = 1;
        while pending > 0 {
            pending -= 1;
            let node = &self.nodes[stack[pending]];
            if node.bounds.padded_entry(ray, 1e-9, limit).is_none() {
                continue;
            }
            if let Some((a, b)) = node.children {
                let a_hit = self.nodes[a].bounds.padded_entry(ray, 1e-9, limit);
                let b_hit = self.nodes[b].bounds.padded_entry(ray, 1e-9, limit);
                match (a_hit, b_hit) {
                    (Some(x), Some(y)) => {
                        let (near, far) = if x <= y { (a, b) } else { (b, a) };
                        stack[pending] = far;
                        stack[pending + 1] = near;
                        pending += 2;
                    }
                    (Some(_), None) => {
                        stack[pending] = a;
                        pending += 1;
                    }
                    (None, Some(_)) => {
                        stack[pending] = b;
                        pending += 1;
                    }
                    _ => {}
                }
            } else {
                for triangle in &self.triangles[node.range.clone()] {
                    if let Some(hit) = triangle.hit(ray, limit) {
                        limit = hit.distance;
                        best = Some(hit);
                    }
                }
            }
        }
        best
    }

    pub fn unoccluded(&self, origin: DVec3, point: DVec3) -> bool {
        let distance = point.distance(origin);
        if distance < 1e-12 {
            return false;
        }
        self.nearest(
            Ray {
                origin,
                direction: (point - origin) / distance,
            },
            distance,
        )
        .is_none_or(|hit| hit.distance + 1e-5 >= distance)
    }
}

impl Triangle {
    fn hit(&self, ray: Ray, limit: f64) -> Option<Hit> {
        let [a, b, c] = self.points;
        let normal = (b - a).cross(c - a);
        let denominator = ray.direction.dot(normal);
        if denominator.abs() < normal.length() * 1e-12 {
            return None;
        }
        let distance = (a - ray.origin).dot(normal) / denominator;
        if distance < 1e-8 || distance > limit {
            return None;
        }
        let point = ray.origin + ray.direction * distance;
        let u = (point - a).cross(c - a).dot(normal) / normal.length_squared();
        let v = (b - a).cross(point - a).dot(normal) / normal.length_squared();
        if u < -1e-9 || v < -1e-9 || u + v > 1. + 1e-9 {
            return None;
        }
        let mut normal = normal.normalize();
        let size = normal.abs();
        let axis = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        // Cuts retain the lower half-space; their outward surface normal is +axis.
        if normal[axis] < 0. {
            normal = -normal;
        }
        Some(Hit {
            owner: self.owner,
            point,
            normal,
            distance,
        })
    }
}

#[cfg(test)]
mod section_cap_tests {
    use super::*;

    #[test]
    fn picks_nearest_caps_on_both_sides_and_clears_when_fill_is_removed() {
        let triangles = [
            [DVec3::ZERO, DVec3::X, DVec3::Y],
            [DVec3::Z, DVec3::Z + DVec3::X, DVec3::Z + DVec3::Y],
        ];
        let mut index = Index::default();
        index.prepare(1, &triangles, &[None, Some(7)]);
        let origin = DVec3::new(0.2, 0.2, 3.);
        let ray = Ray {
            origin,
            direction: -DVec3::Z,
        };
        let hit = index.nearest(ray, f64::INFINITY).unwrap();
        assert_eq!(hit.owner, Some(7));
        assert_eq!(hit.point, DVec3::new(0.2, 0.2, 1.));
        assert_eq!(hit.normal, DVec3::Z);
        assert!(!index.unoccluded(origin, DVec3::new(0.2, 0.2, 0.5)));
        assert!(index.unoccluded(origin, hit.point));
        assert!(index.nearest(ray, 1.).is_none());
        let hit = index
            .nearest(
                Ray {
                    origin: DVec3::new(0.2, 0.2, -1.),
                    direction: DVec3::Z,
                },
                f64::INFINITY,
            )
            .unwrap();
        assert_eq!(hit.owner, None);
        assert_eq!(hit.normal, DVec3::Z);
        let nodes = index.nodes.as_ptr();
        index.prepare(1, &triangles, &[None, Some(7)]);
        assert_eq!(index.nodes.as_ptr(), nodes);
        index.prepare(2, &[], &[]);
        assert!(index.nearest(ray, f64::INFINITY).is_none());
    }

    #[test]
    fn hierarchy_matches_triangle_queries_and_preserves_open_holes() {
        let mut triangles = Vec::new();
        let mut owners = Vec::new();
        for x in 0..16 {
            for y in 0..16 {
                if (5..8).contains(&x) && (5..8).contains(&y) {
                    continue;
                }
                let a = DVec3::new(x as f64, y as f64, 0.);
                triangles.extend([
                    [a, a + DVec3::X, a + DVec3::Y],
                    [a + DVec3::X, a + DVec3::X + DVec3::Y, a + DVec3::Y],
                ]);
                owners.extend([Some((x * 16 + y) as u64); 2]);
            }
        }
        let mut index = Index::default();
        index.prepare(1, &triangles, &owners);
        for x in 0..16 {
            for y in 0..16 {
                let ray = Ray {
                    origin: DVec3::new(x as f64 + 0.25, y as f64 + 0.35, 2.),
                    direction: -DVec3::Z,
                };
                let expected = triangles
                    .iter()
                    .zip(&owners)
                    .filter_map(|(points, owner)| {
                        Triangle {
                            points: *points,
                            owner: *owner,
                        }
                        .hit(ray, f64::INFINITY)
                    })
                    .min_by(|a, b| a.distance.total_cmp(&b.distance));
                let actual = index.nearest(ray, f64::INFINITY);
                assert_eq!(actual.map(|hit| hit.owner), expected.map(|hit| hit.owner));
                assert_eq!(
                    actual.map(|hit| hit.distance),
                    expected.map(|hit| hit.distance)
                );
            }
        }
    }
}
