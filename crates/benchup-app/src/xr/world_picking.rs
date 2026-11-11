//! Retained world-space edge and construction-guide controller-ray queries.
use benchup_core::{ConstructionGuide, DVec3, Model, Ray};
use std::sync::Arc;

/// The finite displayed span of construction lines, shared by drawing and picking.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct GuideFrame {
    pub center: DVec3,
    pub extent: f64,
}

impl GuideFrame {
    pub fn segment(self, guide: &ConstructionGuide) -> (DVec3, DVec3) {
        guide
            .direction
            .map_or((guide.origin, guide.origin), |direction| {
                let center = guide.origin + direction * (self.center - guide.origin).dot(direction);
                (
                    center - direction * self.extent,
                    center + direction * self.extent,
                )
            })
    }
}

#[derive(Clone, Copy)]
struct Edge {
    id: u64,
    a: DVec3,
    b: DVec3,
}

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    lo: DVec3,
    hi: DVec3,
}

impl Bounds {
    pub(super) fn empty() -> Self {
        Self {
            lo: DVec3::INFINITY,
            hi: DVec3::NEG_INFINITY,
        }
    }
    pub(super) fn include(&mut self, p: DVec3) {
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
        self.padded_entry(ray, radius, limit)
    }
    pub(super) fn padded_entry(self, ray: Ray, radius: f64, limit: f64) -> Option<f64> {
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

#[derive(Clone, Copy)]
pub(super) struct Hit {
    pub id: u64,
    pub point: DVec3,
    depth: f64,
    score: f64,
}

#[derive(Default)]
pub(super) struct Index {
    source: Option<Arc<Model>>,
    guide_frame: Option<GuideFrame>,
    edges: Vec<Edge>,
    nodes: Vec<Node>,
}

impl Index {
    /// Query point anchors after an affine projection (for example onto an
    /// inference plane). Transform node bounds, not every scene anchor per frame.
    pub fn query_mapped_candidates(
        &self, ray: Ray, slope: f64, transform: glam::DMat4, limit: usize,
        visible: impl Fn(u64, DVec3, DVec3) -> bool,
        equivalent: impl Fn(u64, u64) -> bool,
    ) -> Vec<Hit> {
        let mut best: Vec<Hit> = Vec::with_capacity(limit);
        if self.nodes.is_empty() || limit == 0 { return best; }
        let Some(direction) = ray.direction.try_normalize() else { return best; };
        let ray = Ray { direction, ..ray };
        if !ray.origin.is_finite() || !slope.is_finite() || slope <= 0. || !transform.is_finite() { return best; }
        let rank = |a: &Hit, b: &Hit| a.score.total_cmp(&b.score).then(a.depth.total_cmp(&b.depth)).then(a.id.cmp(&b.id));
        let mut pending = [0_usize; usize::BITS as usize + 1];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let node = &self.nodes[pending[len]];
            let mut bounds = Bounds::empty();
            for x in [node.bounds.lo.x, node.bounds.hi.x] {
                for y in [node.bounds.lo.y, node.bounds.hi.y] {
                    for z in [node.bounds.lo.z, node.bounds.hi.z] {
                        bounds.include(transform.transform_point3(DVec3::new(x, y, z)));
                    }
                }
            }
            let threshold = if best.len() == limit { best.last().unwrap().score.min(slope) } else { slope };
            if bounds.entry(ray, threshold, f64::INFINITY).is_none() { continue; }
            if let Some((a, b)) = node.children {
                pending[len] = a; pending[len + 1] = b; len += 2;
                continue;
            }
            for anchor in &self.edges[node.start..node.end] {
                let point = transform.transform_point3(anchor.a);
                let delta = point - ray.origin;
                let depth = delta.dot(ray.direction);
                if depth <= 0. || !depth.is_finite() { continue; }
                let score = (delta - ray.direction * depth).length() / depth;
                let candidate = Hit { id: anchor.id, point, depth, score };
                if score > slope || best.len() == limit && rank(&candidate, best.last().unwrap()).is_gt() { continue; }
                let duplicate = best.iter().position(|old| equivalent(anchor.id, old.id));
                if duplicate.is_some_and(|index| !rank(&candidate, &best[index]).is_lt()) { continue; }
                if visible(anchor.id, anchor.a, point) {
                    if let Some(index) = duplicate { best.remove(index); }
                    let index = best.binary_search_by(|old| rank(old, &candidate)).unwrap_or_else(|index| index);
                    best.insert(index, candidate);
                    best.truncate(limit);
                }
            }
        }
        best
    }

    pub fn replace_points(&mut self, points: impl IntoIterator<Item = (u64, DVec3)>) {
        self.replace_segments(points.into_iter().map(|(id, point)| (id, point, point)));
    }

    pub fn replace_segments(&mut self, segments: impl IntoIterator<Item = (u64, DVec3, DVec3)>) {
        self.edges.clear();
        self.edges
            .extend(segments.into_iter().filter_map(|(id, a, b)| {
                (a.is_finite() && b.is_finite()).then_some(Edge { id, a, b })
            }));
        self.nodes.clear();
        if !self.edges.is_empty() {
            self.build(0, self.edges.len());
        }
        self.source = None;
        self.guide_frame = None;
    }

    /// Query retained point anchors with a conservative radius for their geometry.
    /// The callback tests the exact displayed shape only in intersected leaves.
    pub fn query_padded(
        &self,
        ray: Ray,
        radius: f64,
        point: impl Fn(u64) -> Option<DVec3>,
    ) -> Option<Hit> {
        self.nodes.first()?;
        if !radius.is_finite() || radius < 0. || !ray.origin.is_finite() {
            return None;
        }
        let ray = Ray {
            direction: ray.direction.try_normalize()?,
            ..ray
        };
        let mut best: Option<Hit> = None;
        let mut pending = [0_usize; usize::BITS as usize + 1];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let node = &self.nodes[pending[len]];
            let limit = best.as_ref().map_or(f64::INFINITY, |hit| hit.depth);
            if node.bounds.padded_entry(ray, radius, limit).is_none() {
                continue;
            }
            if let Some((a, b)) = node.children {
                pending[len] = a;
                pending[len + 1] = b;
                len += 2;
            } else {
                for edge in &self.edges[node.start..node.end] {
                    let Some(point) = point(edge.id).filter(|p| p.is_finite()) else {
                        continue;
                    };
                    let depth = (point - ray.origin).dot(ray.direction);
                    if depth > 0.
                        && best.as_ref().is_none_or(|hit| {
                            depth < hit.depth || depth == hit.depth && edge.id < hit.id
                        })
                    {
                        best = Some(Hit {
                            id: edge.id,
                            point,
                            depth,
                            score: 0.,
                        });
                    }
                }
            }
        }
        best
    }

    /// Find precise guide targets near the ray without testing every guide pair.
    /// Only the nearest 16 local candidates participate (at most 120 pairs).
    pub fn guide_query(
        &self,
        ray: Ray,
        slope: f64,
        visible: impl Fn(u64) -> bool,
    ) -> (Option<Hit>, [Option<DVec3>; 8]) {
        let empty = (None, [None; 8]);
        if self.nodes.is_empty() || !ray.origin.is_finite() || !slope.is_finite() || slope <= 0. {
            return empty;
        }
        let Some(direction) = ray.direction.try_normalize() else {
            return empty;
        };
        let ray = Ray { direction, ..ray };
        let mut nearby: Vec<(Edge, Hit)> = Vec::with_capacity(16);
        let mut pending = [0_usize; usize::BITS as usize + 1];
        let mut len = 1;
        while len > 0 {
            len -= 1;
            let node = &self.nodes[pending[len]];
            if node.bounds.entry(ray, slope, f64::INFINITY).is_none() {
                continue;
            }
            if let Some((a, b)) = node.children {
                pending[len] = a;
                pending[len + 1] = b;
                len += 2;
                continue;
            }
            for &edge in &self.edges[node.start..node.end] {
                if !visible(edge.id) {
                    continue;
                }
                let Some((point, depth, score)) =
                    closest(ray, edge.a, edge.b).filter(|(_, _, score)| *score <= slope)
                else {
                    continue;
                };
                let hit = Hit {
                    id: edge.id,
                    point,
                    depth,
                    score,
                };
                let at = nearby.partition_point(|(_, old)| {
                    old.score
                        .total_cmp(&score)
                        .then(old.depth.total_cmp(&depth))
                        .then(old.id.cmp(&edge.id))
                        .is_lt()
                });
                if at < 16 {
                    nearby.insert(at, (edge, hit));
                    nearby.truncate(16);
                }
            }
        }
        let mut points: Vec<(DVec3, f64, f64)> = Vec::new();
        let mut add = |point| {
            if let Some(candidate) =
                closest(ray, point, point).filter(|(_, _, score)| *score <= slope)
                && !points
                    .iter()
                    .any(|(old, _, _)| old.distance_squared(point) < 1e-12)
            {
                points.push(candidate);
            }
        };
        for (i, (a, _)) in nearby.iter().enumerate() {
            if a.a == a.b {
                add(a.a);
            }
            for (b, _) in &nearby[i + 1..] {
                if let Some(point) = intersection(*a, *b) {
                    add(point);
                }
            }
        }
        points.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1)));
        let mut targets = [None; 8];
        for (target, (point, _, _)) in targets.iter_mut().zip(points) {
            *target = Some(point);
        }
        (nearby.first().map(|(_, hit)| *hit), targets)
    }

    pub fn prepare_guides(&mut self, model: &Arc<Model>, frame: GuideFrame) {
        if self
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, model))
            && self.guide_frame == Some(frame)
        {
            return;
        }
        self.edges.clear();
        self.edges
            .extend(model.guides.iter().filter_map(|(&id, guide)| {
                let (a, b) = frame.segment(guide);
                guide.valid().then_some(Edge { id, a, b })
            }));
        self.nodes.clear();
        if !self.edges.is_empty() {
            self.build(0, self.edges.len());
        }
        self.source = Some(Arc::clone(model));
        self.guide_frame = Some(frame);
    }

    pub fn prepare(&mut self, model: &Arc<Model>) {
        if self
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, model))
            && self.guide_frame.is_none()
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
        self.guide_frame = None;
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

/// Hit the displayed rectangle, including its finite boundary.
pub(super) fn quad_point(ray: Ray, quad: [DVec3; 4]) -> Option<(DVec3, f64)> {
    let u = quad[1] - quad[0];
    let v = quad[3] - quad[0];
    let normal = u.cross(v).try_normalize()?;
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 1e-8 {
        return None;
    }
    let depth = (quad[0] - ray.origin).dot(normal) / denominator;
    if !depth.is_finite() || depth <= 0. {
        return None;
    }
    let point = ray.origin + ray.direction * depth;
    let delta = point - quad[0];
    let (a, b) = (
        delta.dot(u) / u.length_squared(),
        delta.dot(v) / v.length_squared(),
    );
    ((0. ..=1.).contains(&a) && (0. ..=1.).contains(&b)).then_some((point, depth))
}

fn intersection(a: Edge, b: Edge) -> Option<DVec3> {
    let ad = (a.b - a.a).try_normalize()?;
    let bd = (b.b - b.a).try_normalize()?;
    let point = Ray {
        origin: a.a,
        direction: ad,
    }
    .axis(b.a, bd)?;
    if (point - a.a).cross(ad).length() > 1e-6 {
        return None;
    }
    for edge in [a, b] {
        let segment = edge.b - edge.a;
        let t = (point - edge.a).dot(segment) / segment.length_squared();
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
    }
    Some(point)
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
    #[test]
    fn displayed_flip_quads_reject_parallel_behind_and_outside_rays() {
        let quad = [
            DVec3::new(-1., -1., 2.),
            DVec3::new(1., -1., 2.),
            DVec3::new(1., 1., 2.),
            DVec3::new(-1., 1., 2.),
        ];
        assert_eq!(quad_point(ray(), quad), Some((DVec3::new(0., 0., 2.), 2.)));
        assert!(
            quad_point(
                Ray {
                    direction: DVec3::X,
                    ..ray()
                },
                quad
            )
            .is_none()
        );
        assert!(
            quad_point(
                Ray {
                    direction: -DVec3::Z,
                    ..ray()
                },
                quad
            )
            .is_none()
        );
        assert!(
            quad_point(
                Ray {
                    origin: DVec3::new(2., 0., 0.),
                    ..ray()
                },
                quad
            )
            .is_none()
        );
        assert!(quad_point(ray(), [DVec3::ZERO; 4]).is_none());
    }

    #[test]
    fn guide_crossings_and_points_are_precise_local_targets() {
        let crossing = DVec3::new(0.006, 0., 2.);
        let point = DVec3::new(-0.004, 0., 2.);
        let guides = index(&[
            (1, DVec3::new(-1., 0., 2.), DVec3::new(1., 0., 2.)),
            (2, crossing - DVec3::Y, crossing + DVec3::Y),
            (3, point, point),
        ]);
        let (hit, targets) = guides.guide_query(ray(), 0.01, |_| true);
        assert_eq!(hit.unwrap().id, 1);
        let targets: Vec<_> = targets.into_iter().flatten().collect();
        assert_eq!(targets, vec![point, crossing]);
        let (_, targets) = guides.guide_query(ray(), 0.01, |id| id != 2);
        assert_eq!(
            targets.into_iter().flatten().collect::<Vec<_>>(),
            vec![point]
        );
    }

    #[test]
    fn guide_intersections_reject_skew_parallel_and_out_of_span_lines() {
        let a = Edge {
            id: 1,
            a: DVec3::new(-1., 0., 2.),
            b: DVec3::new(1., 0., 2.),
        };
        let b = Edge {
            id: 2,
            a: DVec3::new(0., -1., 2.),
            b: DVec3::new(0., 1., 2.),
        };
        assert_eq!(intersection(a, b), Some(DVec3::new(0., 0., 2.)));
        assert!(
            intersection(
                a,
                Edge {
                    a: b.a + DVec3::Z * 0.01,
                    b: b.b + DVec3::Z * 0.01,
                    ..b
                }
            )
            .is_none()
        );
        assert!(
            intersection(
                a,
                Edge {
                    a: b.a + DVec3::X * 2.,
                    b: b.b + DVec3::X * 2.,
                    ..b
                }
            )
            .is_none()
        );
        assert!(
            intersection(
                a,
                Edge {
                    a: a.a + DVec3::Y,
                    b: a.b + DVec3::Y,
                    ..b
                }
            )
            .is_none()
        );
    }

    #[test]
    fn guide_points_and_displayed_line_spans_share_the_hierarchy() {
        let mut model = Model::default();
        model.guides.insert(
            1,
            ConstructionGuide {
                origin: DVec3::new(0., 0., 2.),
                direction: None,
            },
        );
        model.guides.insert(
            2,
            ConstructionGuide {
                origin: DVec3::new(0., 1., 4.),
                direction: Some(DVec3::X),
            },
        );
        let model = Arc::new(model);
        let frame = GuideFrame {
            center: DVec3::ZERO,
            extent: 2.,
        };
        let mut index = Index::default();
        index.prepare_guides(&model, frame);
        assert_eq!(
            index
                .query(ray(), 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .unwrap()
                .id,
            1
        );
        let line_ray = Ray {
            origin: DVec3::new(1., 1., 0.),
            direction: DVec3::Z,
        };
        assert_eq!(
            index
                .query(line_ray, 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .unwrap()
                .id,
            2
        );
        let outside = Ray {
            origin: DVec3::new(3., 1., 0.),
            direction: DVec3::Z,
        };
        assert!(
            index
                .query(outside, 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .is_none()
        );
        index.prepare_guides(
            &model,
            GuideFrame {
                center: DVec3::new(2., 0., 0.),
                ..frame
            },
        );
        assert_eq!(
            index
                .query(outside, 0.01, |_, a, b| Some((a, b)), |_, _| true)
                .unwrap()
                .id,
            2
        );
        assert!(
            index
                .query(outside, 0.01, |_, _, _| None, |_, _| true)
                .is_none()
        );
    }

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
