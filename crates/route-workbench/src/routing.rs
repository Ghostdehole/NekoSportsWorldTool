use crate::{distance_m, Point, MAX_POINTS};
use petgraph::{graph::NodeIndex, visit::EdgeRef};
use route_planner::RoadGraph;
use std::{cmp::Ordering, collections::BinaryHeap};

const MAX_NODES: usize = 250_000;
const MAX_EDGES: usize = 1_000_000;
const MAX_OPERATIONS: usize = 10_000_000;

#[derive(Clone, Copy)]
struct QueueItem {
    cost: f64,
    node: NodeIndex,
}
impl PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.node == other.node
    }
}
impl Eq for QueueItem {}
impl PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.node.index().cmp(&self.node.index()))
    }
}

pub(crate) fn eligible_highway(highway: &str) -> bool {
    matches!(
        highway,
        "footway"
            | "path"
            | "pedestrian"
            | "steps"
            | "living_street"
            | "residential"
            | "service"
            | "unclassified"
            | "track"
            | "tertiary"
            | "tertiary_link"
            | "road"
            | "corridor"
            | "cycleway"
            | "bridleway"
    )
}

/// Plans a polyline on actual directed graph edges. Inputs snap to existing nodes
/// within 100 m; off-road connectors and disconnected-component bridges are never added.
/// Use only a raw WGS84 local graph, not one transformed into a service coordinate system.
pub fn plan_on_network(net: &RoadGraph, waypoints: &[Point]) -> Result<Vec<Point>, String> {
    if !(2..=128).contains(&waypoints.len()) {
        return Err("沿路规划需要 2～128 个起点、终点或途经点".into());
    }
    if net.graph.node_count() == 0
        || net.graph.node_count() > MAX_NODES
        || net.graph.edge_count() > MAX_EDGES
    {
        return Err("路网为空或过大，请导入较小范围的校园路网".into());
    }
    for point in waypoints {
        point.validate()?;
    }
    let mut eligible = vec![false; net.graph.node_count()];
    for node in net.graph.node_weights() {
        Point {
            lat: node.coord.lat,
            lon: node.coord.lon,
        }
        .validate()?;
    }
    for edge in net.graph.edge_references() {
        if eligible_highway(&edge.weight().highway) {
            if !edge.weight().length_m.is_finite() || edge.weight().length_m < 0.0 {
                return Err("路网包含无效边长".into());
            }
            eligible[edge.source().index()] = true;
            eligible[edge.target().index()] = true;
        }
    }
    let mut operations = 0usize;
    let mut snapped = Vec::with_capacity(waypoints.len());
    for (index, point) in waypoints.iter().enumerate() {
        let mut nearest = None;
        let mut best = f64::INFINITY;
        for node in net
            .graph
            .node_indices()
            .filter(|node| eligible[node.index()])
        {
            budget(&mut operations)?;
            let c = net.node_coord(node);
            let distance = distance_m(
                *point,
                Point {
                    lat: c.lat,
                    lon: c.lon,
                },
            );
            if distance < best {
                best = distance;
                nearest = Some(node);
            }
        }
        if best > 100.0 {
            return Err(format!(
                "第 {} 个点距可用道路节点超过 100 米，请靠近已有路网打点",
                index + 1
            ));
        }
        snapped.push(nearest.ok_or("路网没有可用步行道路")?);
    }
    let mut result = Vec::new();
    for (leg, pair) in snapped.windows(2).enumerate() {
        let path = shortest_path(net, pair[0], pair[1], &mut operations)?.ok_or_else(|| {
            format!(
                "第 {} 段道路不连通，或当前方向不可达；请调整途经点",
                leg + 1
            )
        })?;
        for node in path {
            let c = net.node_coord(node);
            let point = Point {
                lat: c.lat,
                lon: c.lon,
            };
            if result.last() != Some(&point) {
                result.push(point);
            }
            if result.len() > MAX_POINTS {
                return Err("规划结果超过 10000 个点，请缩小路线范围".into());
            }
        }
    }
    if result.len() < 2 {
        return Err("所有点吸附到同一道路节点，请拉开起终点距离".into());
    }
    Ok(result)
}

fn budget(operations: &mut usize) -> Result<(), String> {
    *operations += 1;
    if *operations > MAX_OPERATIONS {
        Err("规划计算量过大，请减少途经点或缩小路网".into())
    } else {
        Ok(())
    }
}

fn shortest_path(
    net: &RoadGraph,
    start: NodeIndex,
    end: NodeIndex,
    operations: &mut usize,
) -> Result<Option<Vec<NodeIndex>>, String> {
    let mut distances = vec![f64::INFINITY; net.graph.node_count()];
    let mut previous = vec![None; net.graph.node_count()];
    let mut queue = BinaryHeap::new();
    distances[start.index()] = 0.0;
    queue.push(QueueItem {
        cost: 0.0,
        node: start,
    });
    while let Some(QueueItem { cost, node }) = queue.pop() {
        budget(operations)?;
        if cost > distances[node.index()] {
            continue;
        }
        if node == end {
            let mut path = vec![node];
            let mut current = node;
            while current != start {
                current = previous[current.index()].ok_or("路网路径回溯失败")?;
                path.push(current);
                if path.len() > MAX_POINTS {
                    return Err("单段规划超过 10000 个点".into());
                }
            }
            path.reverse();
            return Ok(Some(path));
        }
        for edge in net.graph.edges(node) {
            budget(operations)?;
            if !eligible_highway(&edge.weight().highway) {
                continue;
            }
            let candidate = cost + edge.weight().length_m;
            let target = edge.target();
            if candidate < distances[target.index()] {
                distances[target.index()] = candidate;
                previous[target.index()] = Some(node);
                queue.push(QueueItem {
                    cost: candidate,
                    node: target,
                });
            }
        }
    }
    Ok(None)
}
