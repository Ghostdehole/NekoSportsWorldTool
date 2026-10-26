use crate::{check_size, Point, RouteDocument, MAX_POINTS};
use quick_xml::{events::Event, name::ResolveResult, reader::NsReader, XmlVersion};
use std::fmt::Write;

pub fn export(doc: &RouteDocument) -> Result<String, String> {
    doc.validate()?;
    if doc.points.len() < 2 {
        return Err("至少添加两个点后才能导出 GPX 路线".into());
    }
    if doc.closed && doc.points.len() == MAX_POINTS && doc.points.first() != doc.points.last() {
        return Err("闭合路线导出 GPX 最多允许 9999 个编辑点（另需一个闭合点）".into());
    }
    let mut text = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Neko Route Workbench\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n  <rte>\n",
    );
    writeln!(
        text,
        "    <name>{}</name>",
        quick_xml::escape::escape(&doc.name)
    )
    .unwrap();
    for point in &doc.points {
        writeln!(
            text,
            "    <rtept lat=\"{}\" lon=\"{}\"/>",
            point.lat, point.lon
        )
        .unwrap();
    }
    if doc.closed && doc.points.first() != doc.points.last() {
        let first = doc.points[0];
        writeln!(
            text,
            "    <rtept lat=\"{}\" lon=\"{}\"/>",
            first.lat, first.lon
        )
        .unwrap();
    }
    text.push_str("  </rte>\n</gpx>\n");
    Ok(text)
}

fn is_name_path(stack: &[String]) -> bool {
    stack.len() == 3
        && stack[0] == "gpx"
        && matches!(stack[1].as_str(), "rte" | "trk")
        && stack[2] == "name"
}

pub fn import(text: &str) -> Result<RouteDocument, String> {
    check_size(text)?;
    let mut reader = NsReader::from_str(text);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<String> = Vec::new();
    let mut doc = RouteDocument::default();
    let (mut roots, mut containers, mut segments) = (0, 0, 0);
    let mut name = String::new();
    let mut name_seen = false;
    let mut root_namespace: Option<Vec<u8>> = None;
    loop {
        match reader
            .read_event()
            .map_err(|e| format!("GPX XML 无效：{e}"))?
        {
            Event::Start(e) => {
                if stack.len() >= 64 {
                    return Err("GPX 嵌套层级过深".into());
                }
                let mut local = String::from_utf8(e.local_name().as_ref().to_vec())
                    .map_err(|_| "GPX 元素名称无效")?;
                let namespace = match reader.resolver().resolve_element(e.name()).0 {
                    ResolveResult::Unbound => None,
                    ResolveResult::Bound(ns) => Some(ns.as_ref().to_vec()),
                    ResolveResult::Unknown(_) => return Err("GPX 存在未声明的命名空间前缀".into()),
                };
                if stack.is_empty() {
                    roots += 1;
                    if roots != 1 || local != "gpx" {
                        return Err("文件必须只有一个 GPX 根元素".into());
                    }
                    if namespace.as_deref().is_some_and(|ns| {
                        ns != b"http://www.topografix.com/GPX/1/1"
                            && ns != b"http://www.topografix.com/GPX/1/0"
                    }) {
                        return Err("不支持此 GPX 命名空间".into());
                    }
                    root_namespace = namespace.clone();
                } else if local == "gpx" {
                    return Err("GPX 根元素不能嵌套".into());
                }
                let structural = matches!(
                    local.as_str(),
                    "gpx" | "rte" | "trk" | "trkseg" | "rtept" | "trkpt"
                );
                if structural && namespace != root_namespace {
                    return Err("GPX 路线元素的命名空间不一致".into());
                }
                if local == "name" && namespace != root_namespace {
                    local = "extension-name".into();
                }
                let mut lat = None;
                let mut lon = None;
                for attribute in e.attributes() {
                    let a = attribute.map_err(|e| format!("GPX 属性无效：{e}"))?;
                    let key = a.key.as_ref();
                    let value = a
                        .decoded_and_normalized_value(XmlVersion::Explicit1_0, reader.decoder())
                        .map_err(|e| format!("GPX 属性无效：{e}"))?;
                    if key == b"lat" {
                        lat = value.parse::<f64>().ok();
                    }
                    if key == b"lon" {
                        lon = value.parse::<f64>().ok();
                    }
                }
                if matches!(local.as_str(), "rte" | "trk") {
                    if stack.len() != 1 || stack[0] != "gpx" {
                        return Err("GPX 路线/轨迹的位置无效".into());
                    }
                    containers += 1;
                    if containers > 1 {
                        return Err(
                            "文件包含多条路线/轨迹，请先选择单条再导入；不会自动连接分段".into(),
                        );
                    }
                }
                if local == "trkseg" {
                    if stack.as_slice() != ["gpx", "trk"] {
                        return Err("GPX 轨迹分段的位置无效".into());
                    }
                    segments += 1;
                    if segments > 1 {
                        return Err("文件包含多个轨迹分段，请先拆分；不会跨分段连线".into());
                    }
                }
                if matches!(local.as_str(), "rtept" | "trkpt") {
                    let valid = (local == "rtept" && stack.as_slice() == ["gpx", "rte"])
                        || (local == "trkpt" && stack.as_slice() == ["gpx", "trk", "trkseg"]);
                    if !valid {
                        return Err("GPX 路线点的位置无效".into());
                    }
                    let point = Point {
                        lat: lat.ok_or("GPX 点缺少有效纬度")?,
                        lon: lon.ok_or("GPX 点缺少有效经度")?,
                    };
                    point.validate()?;
                    doc.points.push(point);
                    if doc.points.len() > MAX_POINTS {
                        return Err("GPX 点数超过 10000".into());
                    }
                }
                if is_name_path(&stack) {
                    return Err("GPX 路线名称不能包含嵌套元素".into());
                }
                stack.push(local);
                if is_name_path(&stack) {
                    if name_seen {
                        return Err("GPX 路线包含重复名称".into());
                    }
                    name_seen = true;
                }
            }
            Event::End(_) => {
                if stack.pop().is_none() {
                    return Err("GPX XML 结束标签无效".into());
                }
            }
            Event::Text(e) => {
                let value = e.decode().map_err(|e| format!("GPX 文本无效：{e}"))?;
                if is_name_path(&stack) {
                    name.push_str(
                        &quick_xml::escape::unescape(&value)
                            .map_err(|e| format!("GPX 名称无效：{e}"))?,
                    );
                } else if stack.is_empty() && !value.trim().is_empty() {
                    return Err("GPX 根元素外不能包含文本".into());
                }
            }
            Event::GeneralRef(e) => {
                let reference = e.decode().map_err(|e| format!("GPX 文本引用无效：{e}"))?;
                let encoded = format!("&{reference};");
                let decoded = quick_xml::escape::unescape(&encoded)
                    .map_err(|e| format!("GPX 文本引用无效：{e}"))?;
                if is_name_path(&stack) {
                    name.push_str(&decoded);
                }
                if stack.is_empty() {
                    return Err("GPX 根元素外不能包含文本引用".into());
                }
            }
            Event::CData(e) => {
                if is_name_path(&stack) {
                    name.push_str(&e.decode().map_err(|e| format!("GPX 名称无效：{e}"))?);
                } else if stack.is_empty() {
                    return Err("GPX 根元素外不能包含 CDATA".into());
                }
            }
            Event::DocType(_) => return Err("不支持带 DTD 的 GPX 文件".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if roots != 1 || !stack.is_empty() || containers != 1 || doc.points.len() < 2 {
        return Err("GPX 不完整或少于两个路线点".into());
    }
    if name_seen {
        doc.name = name;
    }
    if doc.points.len() > 2 && doc.points.first() == doc.points.last() {
        doc.closed = true;
        doc.points.pop();
    }
    doc.validate()?;
    Ok(doc)
}
