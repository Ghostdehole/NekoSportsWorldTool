//! Bounded, local OSM loading into the original project's graph type.
//! Track-way support is inspired by DemaciaForge's fork; disconnected components
//! deliberately stay disconnected. No proximity-based road stitching is used.

use crate::{routing::eligible_highway, Point};
use quick_xml::{events::Event, Reader, XmlVersion};
use route_planner::{
    load::{build_graph, OsmBuilding, OsmRoad, ParsedOsm},
    Coord, RoadGraph,
};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
};

const MAX_OSM_BYTES: u64 = 20 * 1024 * 1024;
type Tags = HashMap<String, String>;
#[derive(Default)]
struct Way {
    id: u64,
    refs: Vec<u64>,
    tags: Tags,
}
#[derive(Default)]
struct Relation {
    members: Vec<u64>,
    tags: Tags,
}

/// Loads a user-selected OSM XML file without network or account access.
pub fn load_local_network(path: &Path) -> Result<RoadGraph, String> {
    let file = File::open(path).map_err(|e| format!("无法打开本地 OSM 文件：{e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_OSM_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取本地 OSM 文件：{e}"))?;
    if bytes.len() as u64 > MAX_OSM_BYTES {
        return Err("OSM 文件超过 20 MiB，请导出较小校园范围".into());
    }
    parse_local_network(&bytes)
}

fn id(value: Option<&String>, kind: &str) -> Result<u64, String> {
    value
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| format!("OSM {kind} ID 缺失或无效"))
}

fn denied(tags: &Tags) -> bool {
    match tags.get("foot").map(String::as_str) {
        Some("no" | "private") => true,
        Some("yes" | "designated" | "permissive") => false,
        _ => matches!(
            tags.get("access").map(String::as_str),
            Some("no" | "private")
        ),
    }
}

fn parse_local_network(bytes: &[u8]) -> Result<RoadGraph, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "OSM 文件必须是 UTF-8 XML")?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().expand_empty_elements = true;
    let mut stack = Vec::<String>::new();
    let mut roots = 0;
    let mut nodes = HashMap::new();
    let mut blocked = HashSet::new();
    let mut node: Option<(u64, Tags)> = None;
    let mut way: Option<Way> = None;
    let mut relation: Option<Relation> = None;
    let mut ways = Vec::new();
    let mut way_ids = HashSet::new();
    let mut relations = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| format!("OSM XML 无效：{e}"))?
        {
            Event::Start(e) => {
                if stack.len() >= 64 {
                    return Err("OSM XML 嵌套层级过深".into());
                }
                let name = String::from_utf8(e.name().as_ref().to_vec())
                    .map_err(|_| "OSM 元素名称无效")?;
                if stack.is_empty() {
                    roots += 1;
                    if roots != 1 || name != "osm" {
                        return Err("文件必须只有一个 osm 根元素".into());
                    }
                }
                let mut attrs = Tags::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| format!("OSM 属性无效：{e}"))?;
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .map_err(|_| "OSM 属性名称无效")?
                        .to_owned();
                    let value = attr
                        .decoded_and_normalized_value(XmlVersion::Explicit1_0, reader.decoder())
                        .map_err(|e| e.to_string())?
                        .into_owned();
                    attrs.insert(key, value);
                }
                match name.as_str() {
                    "node" => {
                        if stack.as_slice() != ["osm"] {
                            return Err("OSM node 结构无效".into());
                        }
                        let node_id = id(attrs.get("id"), "node")?;
                        let point = Point {
                            lat: attrs
                                .get("lat")
                                .and_then(|v| v.parse().ok())
                                .ok_or("OSM node 缺少有效纬度")?,
                            lon: attrs
                                .get("lon")
                                .and_then(|v| v.parse().ok())
                                .ok_or("OSM node 缺少有效经度")?,
                        };
                        point.validate()?;
                        if nodes
                            .insert(node_id, Coord::new(point.lon, point.lat))
                            .is_some()
                        {
                            return Err("OSM 包含重复 node ID".into());
                        }
                        if nodes.len() > 250_000 {
                            return Err("OSM 节点超过 250000，请缩小导出范围".into());
                        }
                        node = Some((node_id, Tags::new()));
                    }
                    "way" => {
                        if stack.as_slice() != ["osm"] {
                            return Err("OSM way 结构无效".into());
                        }
                        let way_id = id(attrs.get("id"), "way")?;
                        if !way_ids.insert(way_id) {
                            return Err("OSM 包含重复 way ID".into());
                        }
                        way = Some(Way {
                            id: way_id,
                            ..Default::default()
                        });
                    }
                    "relation" => {
                        if stack.as_slice() != ["osm"] {
                            return Err("OSM relation 结构无效".into());
                        }
                        relation = Some(Relation::default());
                    }
                    "nd" if stack.as_slice() == ["osm", "way"] => {
                        let w = way.as_mut().ok_or("OSM nd 位于无效道路内")?;
                        w.refs.push(id(attrs.get("ref"), "nd")?);
                        if w.refs.len() > 100_000 {
                            return Err("单条 OSM way 过大".into());
                        }
                    }
                    "tag" => {
                        let (key, value) = (
                            attrs.get("k").ok_or("OSM tag 缺少 k")?,
                            attrs.get("v").ok_or("OSM tag 缺少 v")?,
                        );
                        let tags = match stack.as_slice() {
                            [a, b] if a == "osm" && b == "node" => node.as_mut().map(|n| &mut n.1),
                            [a, b] if a == "osm" && b == "way" => way.as_mut().map(|w| &mut w.tags),
                            [a, b] if a == "osm" && b == "relation" => {
                                relation.as_mut().map(|r| &mut r.tags)
                            }
                            _ => None,
                        };
                        if let Some(tags) = tags {
                            if tags.insert(key.clone(), value.clone()).is_some() {
                                return Err("OSM 对象包含重复 tag".into());
                            }
                        }
                    }
                    "member" if stack.as_slice() == ["osm", "relation"] => {
                        if attrs.get("type").map(String::as_str) == Some("way")
                            && attrs.get("role").map(String::as_str) != Some("inner")
                        {
                            relation
                                .as_mut()
                                .ok_or("OSM member 结构无效")?
                                .members
                                .push(id(attrs.get("ref"), "member")?);
                        }
                    }
                    _ => {}
                }
                stack.push(name);
            }
            Event::End(_) => match stack.pop().as_deref() {
                Some("node") => {
                    if let Some((id, tags)) = node.take() {
                        if denied(&tags) {
                            blocked.insert(id);
                        }
                    }
                }
                Some("way") => {
                    ways.push(way.take().ok_or("OSM way 结构无效")?);
                    if ways.len() > 100_000 {
                        return Err("OSM 道路数量过大".into());
                    }
                }
                Some("relation") => {
                    relations.push(relation.take().ok_or("OSM relation 结构无效")?);
                }
                Some(_) => {}
                None => return Err("OSM XML 结束标签无效".into()),
            },
            Event::DocType(_) => return Err("不支持带 DTD 的 OSM 文件".into()),
            Event::Text(e) if stack.is_empty() => {
                if !e.decode().map_err(|e| e.to_string())?.trim().is_empty() {
                    return Err("OSM 根元素外不能包含文本".into());
                }
            }
            Event::CData(_) | Event::GeneralRef(_) if stack.is_empty() => {
                return Err("OSM 根元素外包含无效内容".into())
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if roots != 1 || !stack.is_empty() {
        return Err("OSM 文件不完整".into());
    }
    // Resolve after parsing all ways, so relation ordering cannot drop a track.
    let mut track_members = HashSet::new();
    for r in relations {
        if !denied(&r.tags) && r.tags.get("leisure").map(String::as_str) == Some("track") {
            track_members.extend(r.members);
        }
    }
    let mut roads = Vec::new();
    let mut buildings = Vec::new();
    for way in ways {
        if way.tags.get("building").is_some_and(|v| v != "no") {
            if way.refs.len() >= 3 && way.refs.iter().all(|r| nodes.contains_key(r)) {
                buildings.push(OsmBuilding {
                    outer: vec![way.refs],
                });
            }
            continue;
        }
        if denied(&way.tags) {
            continue;
        }
        let is_track = way.tags.get("leisure").map(String::as_str) == Some("track")
            || track_members.contains(&way.id);
        let highway = match way.tags.get("highway") {
            Some(h) if eligible_highway(h) => h.clone(),
            Some(_) => continue,
            None if is_track => "footway".to_owned(),
            None => continue,
        };
        let one_way = way
            .tags
            .get("oneway:foot")
            .or_else(|| way.tags.get("oneway"))
            .map(String::as_str);
        let oneway = matches!(one_way, Some("yes" | "true" | "1"));
        let oneway_reversed = one_way == Some("-1");
        let mut segment = Vec::new();
        for node_id in way.refs.into_iter().chain(std::iter::once(u64::MAX)) {
            // Missing references and explicit forbidden crossing nodes break the road.
            if nodes.contains_key(&node_id) && !blocked.contains(&node_id) && node_id != u64::MAX {
                segment.push(node_id);
            } else {
                if segment.len() >= 2 {
                    roads.push(OsmRoad {
                        id: way.id,
                        node_refs: std::mem::take(&mut segment),
                        oneway,
                        oneway_reversed,
                        highway: highway.clone(),
                        maxspeed_kmh: None,
                    });
                }
                segment.clear();
            }
        }
    }
    let graph = build_graph(&ParsedOsm {
        nodes,
        roads,
        buildings,
    });
    if graph.graph.edge_count() == 0 {
        return Err("OSM 文件没有可用步行道路；可手动打点或换用其他路网".into());
    }
    if graph.graph.edge_count() > 1_000_000 {
        return Err("OSM 路网边数过大，请缩小导出范围".into());
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(body: &str) -> Result<RoadGraph, String> {
        parse_local_network(format!("<osm>{body}</osm>").as_bytes())
    }
    const NODES: &str = "<node id='1' lat='40' lon='116'/><node id='2' lat='40' lon='116.001'/><node id='3' lat='40' lon='116.002'/><node id='4' lat='40' lon='116.003'/>";
    #[test]
    fn missing_nodes_split_roads() {
        let g = parse(&format!("{NODES}<way id='1'><nd ref='1'/><nd ref='2'/><nd ref='999'/><nd ref='3'/><nd ref='4'/><tag k='highway' v='footway'/></way>")).unwrap();
        assert_eq!(g.graph.edge_count(), 4);
        assert!(crate::plan_on_network(
            &g,
            &[
                Point {
                    lat: 40.,
                    lon: 116.
                },
                Point {
                    lat: 40.,
                    lon: 116.003
                }
            ]
        )
        .is_err());
    }
    #[test]
    fn track_relation_before_way_is_resolved() {
        let g = parse(&format!("<relation id='9'><member type='way' ref='1' role='outer'/><tag k='leisure' v='track'/></relation>{NODES}<way id='1'><nd ref='1'/><nd ref='2'/></way>")).unwrap();
        assert_eq!(g.graph.edge_count(), 2);
    }
    #[test]
    fn track_does_not_override_foot_restrictions() {
        assert!(parse(&format!("{NODES}<way id='1'><nd ref='1'/><nd ref='2'/><tag k='leisure' v='track'/><tag k='foot' v='no'/></way>")).is_err());
        assert!(parse(&format!("{NODES}<way id='1'><nd ref='1'/><nd ref='2'/><tag k='leisure' v='track'/><tag k='highway' v='motorway'/></way>")).is_err());
    }
    #[test]
    fn malformed_and_nonfinite_osm_is_rejected() {
        assert!(parse_local_network(b"<osm><node id='1' lat='NaN' lon='116'/></osm>").is_err());
        assert!(parse_local_network(b"<osm><way id='1'>").is_err());
        assert!(parse_local_network(b"<!DOCTYPE osm><osm/>").is_err());
    }
}
