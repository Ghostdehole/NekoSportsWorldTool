//! Local-only route documents. No session, account, clock or service transport is used.
//!
//! Reuses the upstream project's OSM graph representation for ordinary geometry.
//! Coordinates here are always WGS84 and must never be passed to an upload flow.

mod gpx;
mod osm;
mod routing;

pub use osm::load_local_network;
pub use routing::plan_on_network;
use serde::{Deserialize, Serialize};

pub const MAX_POINTS: usize = 10_000;
pub const MAX_DOCUMENT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

impl Point {
    pub fn validate(&self) -> Result<(), String> {
        if !self.lat.is_finite()
            || !self.lon.is_finite()
            || self.lat.abs() > 90.0
            || self.lon.abs() > 180.0
        {
            Err("坐标无效：纬度须在 -90～90，经度须在 -180～180，且为有限数字".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDocument {
    pub version: u32,
    pub name: String,
    pub coordinate_system: String,
    pub points: Vec<Point>,
    pub closed: bool,
}

impl Default for RouteDocument {
    fn default() -> Self {
        Self {
            version: 1,
            name: "自定义路线".into(),
            coordinate_system: "WGS84".into(),
            points: Vec::new(),
            closed: false,
        }
    }
}

impl RouteDocument {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("不支持的路线文件版本（当前支持 version=1）".into());
        }
        if self.coordinate_system != "WGS84" {
            return Err("路线文件必须明确使用 WGS84 坐标；请先转换 GCJ-02 / BD-09".into());
        }
        if self.name.chars().count() > 120 || self.name.chars().any(|c| c.is_control()) {
            return Err("路线名称最多 120 个字符，且不能包含控制字符".into());
        }
        if self.points.len() > MAX_POINTS {
            return Err(format!("路线最多包含 {MAX_POINTS} 个点"));
        }
        for (index, point) in self.points.iter().enumerate() {
            point
                .validate()
                .map_err(|e| format!("第 {} 个点：{e}", index + 1))?;
        }
        Ok(())
    }

    /// Great-circle distance in meters; a closed draft adds only its closing edge.
    pub fn length_m(&self) -> f64 {
        if self.validate().is_err() {
            return 0.0;
        }
        let mut length: f64 = self.points.windows(2).map(|p| distance_m(p[0], p[1])).sum();
        if self.closed && self.points.len() > 2 {
            length += distance_m(*self.points.last().unwrap(), self.points[0]);
        } else if self.closed && self.points.len() == 2 {
            length *= 2.0;
        }
        length
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| format!("无法编码路线：{e}"))
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        check_size(text)?;
        let doc: Self = serde_json::from_str(text).map_err(|e| format!("路线 JSON 无效：{e}"))?;
        doc.validate()?;
        Ok(doc)
    }

    pub fn to_gpx(&self) -> Result<String, String> {
        gpx::export(self)
    }

    pub fn from_gpx(text: &str) -> Result<Self, String> {
        gpx::import(text)
    }
}

pub(crate) fn check_size(text: &str) -> Result<(), String> {
    if text.len() > MAX_DOCUMENT_BYTES {
        Err("路线文件超过 5 MiB，请先拆分文件".into())
    } else {
        Ok(())
    }
}

pub(crate) fn distance_m(a: Point, b: Point) -> f64 {
    let dlat = (b.lat - a.lat).to_radians();
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2)
        + a.lat.to_radians().cos() * b.lat.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    6_371_008.8 * 2.0 * h.clamp(0.0, 1.0).sqrt().asin()
}
