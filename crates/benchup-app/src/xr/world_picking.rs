//! Retained world-space edge hierarchy for controller-ray queries.
use benchup_core::{DVec3, Model, Ray};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Edge {
    id: u64,
    a: DVec3,
    b: DVec3,
}

#[derive(Clone, Copy)]
struct Bounds {
    lo: DVec3,
    hi: DVec3,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            lo: DVec3::INFINITY,
            hi: DVec3::NEG_INFINITY,
        }
    }
    fn include(&mut self, p: DVec3) {
        self.lo = self.lo.min(p);
        self.hi = self.hi.max(p);
    }
    fn entry(self, ray: Ray, slope: f64, limit: f64) -> Option<f64> {
        // Expanding by the maximum cone radius in this node is conservative:
        // it cannot prune a segment point inside the angular hit tolerance.
        let far = (self.lo - ray.origin)
            .abs()
            .max((self.hi - ray.origin).abs())
            .length();
        let radius = (far.min(limit) * slope).max(1e-12);
        let lo = self.lo - DVec3::splat(radius);
        let hi = self.hi + DVec3::splat(radius);
        let (mut enter, mut exit) = (0_f64, limit);
        for axis in 0..3 {
            if ray.direction[axis].abs() < 1e-15 {
                if ray.origin[axis] < lo[axis] || ray.origin[axis] > hi[axis] {
                    return None;
                }
            } else {
                let a = (lo[axis] - ray.origin[axis]) / ray.direction[axis];
                let b = (hi[axis] - ray.origin[axis]) / ray.direction[axis];
                enter = enter.max(a.min(b));
                exit = exit.min(a.max(b));
                if enter > exit {
                    return None;
                }
            }
        }
        Some(enter)
    }
}

struct Node {
    bounds: Bounds,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}

pub(super) struct Hit {
    pub id: u64,
    pub point: DVec3,
    depth: f64,
    score: f64,
}

#[derive(Default)]
pub(super) struct Index {
    source: Option<Arc<Model>>,
    edges: Vec<Edge>,
    nodes: Vec<Node>,
}

impl Index {
    pub fn prepare(&mut self, model: &Arc<Model>) {
        if self
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, model))
        {
            return;
        }
        self.edges.clear();
        self.edges.extend(model.edges.values().filter_map(|edge| {
            let (a, b) = (
                model.vertices.get(&edge.a)?.position,
                model.vertices.get(&edge.b)?.position,
            );
            (a.is_finite() && b.is_finite() && a.distance_squared(b) > 1e-24).then_some(Edge {
                id: edge.id,
                a,
                b,
            })
        }));
        self.nodes.clear();
        if !self.edges.is_empty() {
            self.build(0, self.edges.len());
        }
        self.source = Some(Arc::clone(model));
    }

    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = Bounds::empty();
        let mut centers = Bounds::empty();
        for edge in &self.edges[start..end] {
            bounds.include(edge.a);
            bounds.include(edge.b);
            centers.include((edge.a + edge.b) * 0.5);
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 8 {
            // Split centroids, not endpoint extents: long parallel edges still
            // partition by their differing positions.
            let extent = centers.hi - centers.lo;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let middle = (start + end) / 2;
            self.edges[start..end].select_nth_unstable_by(middle - start, |a, b| {
                (a.a[axis] + a.b[axis])
                    .total_cmp(&(b.a[axis] + b.b[axis]))
                    .then(a.id.cmp(&b.id))
            });
            let a = self.build(start, middle);
            let b = self.build(middle, end);
            self.nodes[index].children = Some((a, b));
        }
        index
    }

    pub fn query(
        &self,
        ray: Ray,
        slope: f64,
        clip: impl Fn(u64, DVec3, DVec3) -> Option<(DVec3, DVec3)>,
        visible: impl Fn(u64, DVec3) -> bool,
    ) -> Option<Hit> {
        self.nodes.first()?;
        if !ray.origin.is_finite() || !slope.is_finite() || slope <= 0. {
            return None;
        }
        let ray = Ray {
            origin: ray.origin,
            direction: ray.direction.try_normalize()?,
        };
        let mut best: Option<Hit> = None;
        let mut pending = [0_usize; usize::BITS as usize + 1];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let node = &self.nodes[pending[len]];
            // A zero angular error cannot be beaten except by a nearer hit.
            let limit = best
                .as_ref()
                .filter(|hit| hit.score == 0.)
                .map_or(f64::INFINITY, |hit| hit.depth);
            if node.bounds.entry(ray, slope, limit).is_none() {
                continue;
            }
            if let Some((a, b)) = node.children {
                let a_entry = self.nodes[a].bounds.entry(ray, slope, limit);
                let b_entry = self.nodes[b].bounds.entry(ray, slope, limit);
                let (near, far) =
                    if a_entry.unwrap_or(f64::INFINITY) <= b_entry.unwrap_or(f64::INFINITY) {
                        (a, b)
                    } else {
                        (b, a)
                    };
                pending[len] = far;
                pending[len + 1] = near;
                len += 2;
                continue;
            }
            for edge in &self.edges[node.start..node.end] {
                let Some((a, b)) = clip(edge.id, edge.a, edge.b) else {
                    continue;
                };
                let Some((point, depth, score)) = closest(ray, a, b) else {
                    continue;
                };
                if score > slope || depth > limit {
                    continue;
                }
                if best.as_ref().is_some_and(|old| {
                    score > old.score || score == old.score && depth >= old.depth
                }) {
                    continue;
                }
                if visible(edge.id, point) {
                    best = Some(Hit {
                        id: edge.id,
                        point,
                        depth,
                        score,
                    });
                }
            }
        }
        best
    }
}

fn closest(ray: Ray, a: DVec3, b: DVec3) -> Option<(DVec3, f64, f64)> {
    let start = a - ray.origin;
    let segment = b - a;
    let t = start.dot(ray.direction);
    let dt = segment.dot(ray.direction);
    let p = start - ray.direction * t;
    let dp = segment - ray.direction * dt;
    let (aa, bb, cc) = (p.length_squared(), p.dot(dp), dp.length_squared());
    let denominator = cc * t - bb * dt;
    let critical = if denominator.abs() > 1e-30 {
        ((aa * dt - bb * t) / denominator).clamp(0., 1.)
    } else {
        0.
    };
    // Minimize angular error, not world distance. A farther endpoint can be
    // much closer to the aim direction than the Euclidean closest point.
    [a, b, a + segment * critical]
        .into_iter()
        .filter_map(|point| {
            let delta = point - ray.origin;
            let depth = delta.dot(ray.direction);
            if depth <= 0. {
                return None;
            }
            let score = (delta - ray.direction * depth).length() / depth;
            score.is_finite().then_some((point, depth, score))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn index(edges: &[(u64, DVec3, DVec3)]) -> Index {
        let mut index = Index::default();
        index
            .edges
            .extend(edges.iter().map(|&(id, a, b)| Edge { id, a, b }));
        if !index.edges.is_empty() {
            index.build(0, index.edges.len());
        }
        index
    }
    fn ray() -> Ray {
        Ray {
            origin: DVec3::ZERO,
            direction: DVec3::Z,
        }
    }
    #[test]
    fn cone_reaches_offset_bounds_and_rejects_behind_or_wide_edges() {
        let near = index(&[(1, DVec3::new(0.006, -1., 1.), DVec3::new(0.006, 1., 1.))]);
        assert_eq!(
            near.query(ray(), 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .unwrap()
                .id,
            1
        );
        let wide = index(&[
            (2, DVec3::new(0.02, -1., 1.), DVec3::new(0.02, 1., 1.)),
            (3, DVec3::new(0., -1., -1.), DVec3::new(0., 1., -1.)),
        ]);
        assert!(
            wide.query(ray(), 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .is_none()
        );
    }
    #[test]
    fn angular_minimum_handles_sloping_and_crossing_segments() {
        let diagonal = index(&[(1, DVec3::new(0.09, 0., 1.), DVec3::new(0.1, 0., 10.))]);
        let hit = diagonal
            .query(ray(), 0.02, |_, a, b| Some((a, b)), |_, _| true)
            .unwrap();
        assert!((hit.depth - 10.).abs() < 1e-9);
        let crossing = index(&[(2, DVec3::new(1., -1., 1.), DVec3::new(-1., 1., 2.))]);
        assert!(
            crossing
                .query(ray(), 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .unwrap()
                .point
                .distance(DVec3::new(0., 0., 1.5))
                < 1e-9
        );
    }
    #[test]
    fn clipping_and_occlusion_can_remove_the_best_angular_target() {
        let edges = index(&[
            (1, DVec3::new(0.003, -1., 1.), DVec3::new(0.003, 1., 1.)),
            (2, DVec3::new(0.004, -1., 2.), DVec3::new(0.004, 1., 2.)),
        ]);
        assert_eq!(
            edges
                .query(ray(), 0.01, |_, a, b| Some((a, b)), |id, _| id != 2)
                .unwrap()
                .id,
            1
        );
        assert_eq!(
            edges
                .query(
                    ray(),
                    0.01,
                    |id, a, b| (id != 1).then_some((a, b)),
                    |_, _| true
                )
                .unwrap()
                .id,
            2
        );
        assert!(
            edges
                .query(
                    ray(),
                    0.01,
                    |_, a, b| Some((a.lerp(b, 0.8), b)),
                    |_, _| true
                )
                .is_none()
        );
    }

    #[test]
    fn hierarchy_keeps_the_nearest_of_many_parallel_crossing_edges() {
        let edges: Vec<_> = (1..=256)
            .map(|id| {
                (
                    id,
                    DVec3::new(-10., 0., id as f64),
                    DVec3::new(10., 0., id as f64),
                )
            })
            .collect();
        let index = index(&edges);
        let hit = index
            .query(ray(), 0.01, |_, a, b| Some((a, b)), |_, _| true)
            .unwrap();
        assert_eq!(hit.id, 1);
        assert!((hit.depth - 1.).abs() < 1e-9);
    }
}
