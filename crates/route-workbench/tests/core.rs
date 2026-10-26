use route_planner::{graph::EdgeData, Coord, RoadGraph};
use route_workbench::{plan_on_network, Point, RouteDocument};

fn point(lat: f64, lon: f64) -> Point {
    Point { lat, lon }
}

fn document(points: Vec<Point>) -> RouteDocument {
    RouteDocument {
        points,
        ..RouteDocument::default()
    }
}

fn edge(length_m: f64) -> EdgeData {
    EdgeData {
        way_id: 1,
        oneway: true,
        highway: "footway".into(),
        maxspeed_kmh: None,
        length_m,
    }
}

fn chain() -> (RoadGraph, Vec<Point>) {
    let points = vec![
        point(40.0, 116.0),
        point(40.0, 116.001),
        point(40.001, 116.001),
    ];
    let mut net = RoadGraph::new();
    let nodes: Vec<_> = points
        .iter()
        .map(|p| net.add_node(Coord::new(p.lon, p.lat)))
        .collect();
    net.add_edge(nodes[0], nodes[1], edge(85.0));
    net.add_edge(nodes[1], nodes[2], edge(111.0));
    net.compute_anchor();
    net.rebuild_index();
    (net, points)
}

#[test]
fn default_and_single_point_are_valid_drafts() {
    let draft = RouteDocument::default();
    assert_eq!(draft.version, 1);
    assert_eq!(draft.coordinate_system, "WGS84");
    assert_eq!(draft.name, "自定义路线");
    assert!(draft.validate().is_ok());
    assert_eq!(draft.length_m(), 0.0);
    assert!(draft.to_gpx().is_err());
    let one = document(vec![point(40.0, 116.0)]);
    assert!(one.validate().is_ok());
    assert_eq!(one.length_m(), 0.0);
    assert!(one.to_gpx().is_err());
}

#[test]
fn coordinate_validation_rejects_nonfinite_and_out_of_range() {
    for p in [
        point(f64::NAN, 0.0),
        point(0.0, f64::INFINITY),
        point(-90.001, 0.0),
        point(90.001, 0.0),
        point(0.0, -180.001),
        point(0.0, 180.001),
    ] {
        assert!(document(vec![p]).validate().is_err(), "accepted {p:?}");
    }
    assert!(document(vec![point(-90.0, -180.0), point(90.0, 180.0)])
        .validate()
        .is_ok());
}

#[test]
fn drafts_are_bounded_and_names_use_character_count() {
    let mut draft = document(vec![point(40.0, 116.0); 10_001]);
    assert!(draft.validate().is_err());
    draft.points.truncate(10_000);
    draft.name = "路".repeat(120);
    assert!(draft.validate().is_ok());
    draft.name.push('线');
    assert!(draft.validate().is_err());
}

#[test]
fn json_roundtrip_preserves_route_without_implicit_conversion() {
    let mut original = document(vec![point(40.0, 116.0), point(40.001, 116.002)]);
    original.name = "西门→操场".into();
    original.closed = true;
    let decoded = RouteDocument::from_json(&original.to_json().unwrap()).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn json_rejects_unknown_version_or_coordinate_system() {
    let mut draft = RouteDocument::default();
    draft.version = 2;
    assert!(draft.validate().is_err());
    assert!(RouteDocument::from_json(&serde_json::to_string(&draft).unwrap()).is_err());
    draft.version = 1;
    draft.coordinate_system = "GCJ02".into();
    assert!(draft.validate().is_err());
    assert!(RouteDocument::from_json(&serde_json::to_string(&draft).unwrap()).is_err());
}

#[test]
fn distance_is_geodesic_and_includes_optional_closure() {
    let mut route = document(vec![point(0.0, 0.0), point(0.0, 1.0)]);
    let one_degree_m = route.length_m();
    assert!((one_degree_m - 111_195.0).abs() < 100.0);
    route.closed = true;
    assert!((route.length_m() - 2.0 * one_degree_m).abs() < 0.001);
    let dateline = document(vec![point(0.0, 179.9), point(0.0, -179.9)]);
    assert!((dateline.length_m() - 22_239.0).abs() < 30.0);
}

#[test]
fn gpx_export_is_a_plan_without_timestamps_or_synthetic_metrics() {
    let mut route = document(vec![point(40.0, 116.0), point(40.001, 116.002)]);
    route.name = "校园 <西门> & \"操场\"".into();
    let text = route.to_gpx().unwrap();
    assert!(text.contains("<rte>"));
    assert!(text.contains("rtept"));
    assert!(!text.contains("<time"));
    assert!(!text.contains("<speed"));
    assert!(!text.contains("<cadence"));
    let parsed = RouteDocument::from_gpx(&text).unwrap();
    assert_eq!(parsed.points, route.points);
    assert_eq!(parsed.name, route.name);
}

#[test]
fn gpx_accepts_prefixed_namespaces_and_real_track_points() {
    let gpx = r#"<?xml version="1.0"?>
<g:gpx xmlns:g="http://www.topografix.com/GPX/1/1" version="1.1">
  <g:trk><g:name>晨跑路线</g:name><g:trkseg>
    <g:trkpt lat="40" lon="116"><g:time>2026-10-26T00:00:00Z</g:time></g:trkpt>
    <g:trkpt lat="40.001" lon="116.002"/>
  </g:trkseg></g:trk>
</g:gpx>"#;
    let route = RouteDocument::from_gpx(gpx).unwrap();
    assert_eq!(
        route.points,
        vec![point(40.0, 116.0), point(40.001, 116.002)]
    );
    assert_eq!(route.name, "晨跑路线");
}

#[test]
fn gpx_resolves_namespace_aliases_and_rejects_undefined_prefixes() {
    let alias = r#"<g:gpx xmlns:g="http://www.topografix.com/GPX/1/1" xmlns="http://www.topografix.com/GPX/1/1"><rte><g:rtept lat="40" lon="116"/><rtept lat="40.001" lon="116.001"/></rte></g:gpx>"#;
    assert!(RouteDocument::from_gpx(alias).is_ok());
    let unbound = r#"<g:gpx><g:rte><g:rtept lat="40" lon="116"/><g:rtept lat="40.001" lon="116.001"/></g:rte></g:gpx>"#;
    assert!(RouteDocument::from_gpx(unbound).is_err());
    let rebound = r#"<gpx xmlns="http://www.topografix.com/GPX/1/1"><rte xmlns="urn:other"><rtept lat="40" lon="116"/><rtept lat="40.001" lon="116.001"/></rte></gpx>"#;
    assert!(RouteDocument::from_gpx(rebound).is_err());
}

#[test]
fn gpx_closed_route_roundtrip_and_strict_boundary() {
    let mut route = document(vec![
        point(40.0, 116.0),
        point(40.001, 116.0),
        point(40.0, 116.001),
    ]);
    route.closed = true;
    assert_eq!(
        RouteDocument::from_gpx(&route.to_gpx().unwrap()).unwrap(),
        route
    );
    route.points = (0..9_999)
        .map(|i| point(40.0, 116.0 + i as f64 / 100_000.0))
        .collect();
    assert_eq!(
        RouteDocument::from_gpx(&route.to_gpx().unwrap()).unwrap(),
        route
    );
    route.points.push(point(41.0, 117.0));
    assert!(route.to_json().is_ok());
    assert!(route.to_gpx().is_err());
}

#[test]
fn gpx_rejects_separate_segments_instead_of_joining_them() {
    let text = r#"<gpx><trk>
      <trkseg><trkpt lat="40" lon="116"/><trkpt lat="40.001" lon="116"/></trkseg>
      <trkseg><trkpt lat="41" lon="116"/><trkpt lat="41.001" lon="116"/></trkseg>
    </trk></gpx>"#;
    assert!(RouteDocument::from_gpx(text).is_err());
}

#[test]
fn gpx_rejects_multiple_routes_instead_of_joining_them() {
    let text = r#"<gpx>
      <rte><rtept lat="40" lon="116"/><rtept lat="40.001" lon="116"/></rte>
      <rte><rtept lat="41" lon="116"/><rtept lat="41.001" lon="116"/></rte>
    </gpx>"#;
    assert!(RouteDocument::from_gpx(text).is_err());
}

#[test]
fn gpx_rejects_malformed_xml_non_gpx_and_dtd() {
    let bad = [
        "<gpx><rte><rtept lat=\"40\" lon=\"116\"/>",
        "<gpx><rte></trk></gpx>",
        "<other><rte><rtept lat=\"40\" lon=\"116\"/><rtept lat=\"41\" lon=\"116\"/></rte></other>",
        "<!DOCTYPE gpx [<!ENTITY x 'unsafe'>]><gpx><rte><rtept lat=\"40\" lon=\"116\"/><rtept lat=\"41\" lon=\"116\"/></rte></gpx>",
        "<gpx><rte><rtept lat=\"NaN\" lon=\"116\"/><rtept lat=\"41\" lon=\"116\"/></rte></gpx>",
        "<gpx><rte><rtept lat=\"40\"/><rtept lat=\"41\" lon=\"116\"/></rte></gpx>",
    ];
    for input in bad {
        assert!(RouteDocument::from_gpx(input).is_err(), "accepted {input}");
    }
}

#[test]
fn gpx_rejects_point_and_byte_limits() {
    let many = format!(
        "<gpx><rte>{}</rte></gpx>",
        "<rtept lat=\"40\" lon=\"116\"/>".repeat(10_001)
    );
    assert!(RouteDocument::from_gpx(&many).is_err());
    let oversized = " ".repeat(5 * 1024 * 1024 + 1);
    assert!(RouteDocument::from_gpx(&oversized).is_err());
    assert!(RouteDocument::from_json(&oversized).is_err());
}

#[test]
fn routing_follows_real_edges_and_keeps_corners() {
    let (net, points) = chain();
    let route = plan_on_network(&net, &[points[0], points[2]]).unwrap();
    assert_eq!(route, points);
}

#[test]
fn routing_chooses_shortest_connected_path_not_straight_line() {
    let (mut net, points) = chain();
    let nodes: Vec<_> = net.graph.node_indices().collect();
    net.add_edge(nodes[0], nodes[2], edge(500.0));
    net.rebuild_index();
    assert_eq!(
        plan_on_network(&net, &[points[0], points[2]]).unwrap(),
        points
    );
}

#[test]
fn routing_honors_waypoint_order() {
    let (mut net, points) = chain();
    let nodes: Vec<_> = net.graph.node_indices().collect();
    net.add_edge(nodes[2], nodes[1], edge(111.0));
    net.rebuild_index();
    let route = plan_on_network(&net, &[points[0], points[2], points[1]]).unwrap();
    assert_eq!(route, vec![points[0], points[1], points[2], points[1]]);
}

#[test]
fn routing_does_not_reverse_oneway_or_bridge_disconnected_components() {
    let (mut net, points) = chain();
    assert!(plan_on_network(&net, &[points[2], points[0]]).is_err());
    let d = net.add_node(Coord::new(116.003, 40.0));
    let e = net.add_node(Coord::new(116.004, 40.0));
    net.add_edge(d, e, edge(85.0));
    net.compute_anchor();
    net.rebuild_index();
    assert!(plan_on_network(&net, &[points[0], point(40.0, 116.004)]).is_err());
}

#[test]
fn routing_snaps_nearby_points_without_fake_connector_segments() {
    let (net, points) = chain();
    let route = plan_on_network(
        &net,
        &[point(points[0].lat + 0.00001, points[0].lon), points[2]],
    )
    .unwrap();
    assert_eq!(route, points);
}

#[test]
fn routing_rejects_far_invalid_or_excessive_waypoints() {
    let (net, points) = chain();
    assert!(plan_on_network(&net, &[point(41.0, 117.0), points[2]]).is_err());
    assert!(plan_on_network(&net, &[point(f64::NAN, 116.0), points[2]]).is_err());
    assert!(plan_on_network(&net, &[]).is_err());
    assert!(plan_on_network(&net, &[points[0]]).is_err());
    assert!(plan_on_network(&net, &vec![points[0]; 129]).is_err());
}
