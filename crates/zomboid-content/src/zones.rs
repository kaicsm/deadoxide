//! Map zone file (`media/maps/<map>/objects.lua`): a Lua table literal `objects = { {name=..,type=..,x=..,..}, ... }`.
//! Parsed with a small constructor-only Lua reader (strings, numbers, booleans, nil, nested tables, `k = v`, `[k] = v`).
//! It does NOT evaluate expressions; it is checked against the real Lua count in tests/ (see docs/COMPATIBILITY.md).
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    Table(Vec<Val>, BTreeMap<String, Val>),
}

impl Val {
    pub fn get(&self, k: &str) -> Option<&Val> {
        if let Val::Table(_, m) = self {
            m.get(k)
        } else {
            None
        }
    }
    pub fn num(&self, k: &str) -> Option<f64> {
        if let Some(Val::Num(n)) = self.get(k) {
            Some(*n)
        } else {
            None
        }
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        if let Some(Val::Str(s)) = self.get(k) {
            Some(s)
        } else {
            None
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        loop {
            while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.b[self.i..].starts_with(b"--") {
                while self.i < self.b.len() && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }
    fn ident(&mut self) -> String {
        let s = self.i;
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.b[s..self.i]).into_owned()
    }
    fn value(&mut self) -> Result<Val, String> {
        self.ws();
        let c = *self.b.get(self.i).ok_or("eof")?;
        match c {
            b'{' => self.table(),
            b'"' | b'\'' => {
                self.i += 1;
                let mut s = Vec::new();
                while self.i < self.b.len() && self.b[self.i] != c {
                    if self.b[self.i] == b'\\' {
                        self.i += 1;
                    }
                    s.push(self.b[self.i]);
                    self.i += 1;
                }
                self.i += 1;
                Ok(Val::Str(String::from_utf8_lossy(&s).into_owned()))
            }
            b'-' | b'.' | b'0'..=b'9' => {
                let s = self.i;
                self.i += 1;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_digit()
                        || matches!(self.b[self.i], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[s..self.i])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .map(Val::Num)
                    .ok_or_else(|| format!("bad number at {s}"))
            }
            _ => match self.ident().as_str() {
                "true" => Ok(Val::Bool(true)),
                "false" => Ok(Val::Bool(false)),
                "nil" => Ok(Val::Nil),
                w => Err(format!("unsupported expression `{w}` at byte {}", self.i)),
            },
        }
    }
    fn table(&mut self) -> Result<Val, String> {
        self.i += 1; // {
        let (mut arr, mut map) = (Vec::new(), BTreeMap::new());
        loop {
            self.ws();
            match self.b.get(self.i) {
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                Some(b',') | Some(b';') => {
                    self.i += 1;
                    continue;
                }
                None => return Err("eof in table".into()),
                _ => {}
            }
            let save = self.i;
            if self.b[self.i] == b'[' {
                self.i += 1;
                let k = self.value()?;
                self.ws();
                self.i += 1; // ]
                self.ws();
                self.i += 1; // =
                let v = self.value()?;
                map.insert(
                    match k {
                        Val::Str(s) => s,
                        Val::Num(n) => n.to_string(),
                        _ => return Err("bad key".into()),
                    },
                    v,
                );
                continue;
            }
            if self.b[self.i].is_ascii_alphabetic() || self.b[self.i] == b'_' {
                let id = self.ident();
                self.ws();
                if self.b.get(self.i) == Some(&b'=') && self.b.get(self.i + 1) != Some(&b'=') {
                    self.i += 1;
                    let v = self.value()?;
                    map.insert(id, v);
                    continue;
                }
                self.i = save;
            }
            arr.push(self.value()?);
        }
        Ok(Val::Table(arr, map))
    }
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub name: String,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    /// polygon points (x,y) when `geometry = "polygon"`, else empty
    pub points: Vec<(i32, i32)>,
    pub has_properties: bool,
}

/// Parse an `objects.lua`; returns every zone in file order.
pub fn parse_objects(src: &[u8]) -> Result<Vec<Zone>, String> {
    let mut p = P { b: src, i: 0 };
    p.ws();
    if p.ident() != "objects" {
        return Err("file does not start with `objects =`".into());
    }
    p.ws();
    p.i += 1; // =
    let Val::Table(arr, _) = p.value()? else {
        return Err("objects is not a table".into());
    };
    let mut out = Vec::with_capacity(arr.len());
    for v in &arr {
        let pts = match v.get("points") {
            Some(Val::Table(a, _)) => a
                .chunks(2)
                .filter_map(|c| match c {
                    [Val::Num(x), Val::Num(y)] => Some((*x as i32, *y as i32)),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        out.push(Zone {
            name: v.str("name").unwrap_or("").to_string(),
            kind: v.str("type").unwrap_or("").to_string(),
            x: v.num("x").unwrap_or(0.0) as i32,
            y: v.num("y").unwrap_or(0.0) as i32,
            z: v.num("z").unwrap_or(0.0) as i32,
            w: v.num("width").unwrap_or(0.0) as i32,
            h: v.num("height").unwrap_or(0.0) as i32,
            points: pts,
            has_properties: v.get("properties").is_some(),
        });
    }
    Ok(out)
}

/// WorldGen-only properties from objects.lua. Unlike `parse_objects`, this keeps
/// Rocks and geometry, which IsoMetaGrid.registerWorldGenZone forwards to the biome generator.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldGenZoneSpec {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    pub geometry: String,
    pub points: Vec<i32>,
    pub polyline_width: i32,
    pub rocks: Option<bool>,
}

/// Parse special zones without losing the `properties.Rocks` override.
/// Unsupported/malformed WorldGen geometry is rejected, not silently downgraded.
pub fn parse_worldgen_zones(src: &[u8]) -> Result<Vec<WorldGenZoneSpec>, String> {
    let mut p = P { b: src, i: 0 };
    p.ws();
    if p.ident() != "objects" { return Err("file does not start with `objects =`".into()); }
    p.ws();
    if p.b.get(p.i) != Some(&b'=') { return Err("missing objects assignment".into()); }
    p.i += 1;
    let Val::Table(arr, _) = p.value()? else { return Err("objects is not a table".into()); };
    let mut out = Vec::new();
    for v in arr {
        if v.str("type") != Some("WorldGen") { continue; }
        let number = |k: &str| -> Result<i32, String> {
            let n = v.num(k).ok_or_else(|| format!("WorldGen zone missing {k}"))?;
            if !n.is_finite() || n.fract() != 0.0 || n < i32::MIN as f64 || n > i32::MAX as f64 {
                return Err(format!("WorldGen zone invalid {k}: {n}"));
            }
            Ok(n as i32)
        };
        let geometry = v.str("geometry").unwrap_or("").to_string();
        if !["", "point", "polyline", "polygon"].contains(&geometry.as_str()) {
            return Err(format!("WorldGen unsupported geometry {geometry}"));
        }
        let points = match v.get("points") {
            Some(Val::Table(a, _)) => a.iter().map(|x| {
                let Val::Num(n) = x else { return Err("WorldGen point not numeric".to_string()); };
                if !n.is_finite() || n.fract() != 0.0 || *n < i32::MIN as f64 || *n > i32::MAX as f64 {
                    return Err("WorldGen point outside i32".to_string());
                }
                Ok(*n as i32)
            }).collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err("WorldGen points is not a list".into()),
            None => Vec::new(),
        };
        if points.len() % 2 != 0 { return Err("WorldGen point pair truncated".into()); }
        let rocks = match v.get("properties").and_then(|p| p.get("Rocks")) {
            Some(Val::Bool(r)) => Some(*r),
            Some(_) => return Err("WorldGen Rocks is not boolean".into()),
            None => None,
        };
        // IsoMetaGrid.registerGeometryZone forwards a geometry's point list and the
        // properties.LineWidth value, not the rectangle's `width` field.
        let line_width = match v.get("properties").and_then(|p| p.num("LineWidth")) {
            Some(n) if n.is_finite() && n.fract() == 0.0 && n >= 0.0 && n <= 255.0 => n as i32,
            Some(_) => return Err("WorldGen LineWidth must be an integer 0..255".into()),
            None => 0,
        };
        let z = number("z")?;
        let (x, y, w, h) = if geometry.is_empty() {
            (number("x")?, number("y")?, number("width")?, number("height")?)
        } else {
            if points.len() < 2 { return Err("WorldGen geometry has no points".into()); }
            let (min_x,max_x) = points.iter().step_by(2)
                .fold((i32::MAX,i32::MIN), |(lo,hi),x| (lo.min(*x),hi.max(*x)));
            let (min_y,max_y) = points.iter().skip(1).step_by(2)
                .fold((i32::MAX,i32::MIN), |(lo,hi),y| (lo.min(*y),hi.max(*y)));
            // Polygon/polylines are registered with their geometry bounds in the
            // meta grid; an expanded bounding rectangle is safe for shape contains.
            let pad = if geometry == "polyline" { (line_width + 1) / 2 } else { 0 };
            let x = min_x.saturating_sub(pad);
            let y = min_y.saturating_sub(pad);
            let w = max_x.saturating_add(pad).saturating_sub(x).saturating_add(1);
            let h = max_y.saturating_add(pad).saturating_sub(y).saturating_add(1);
            (x,y,w,h)
        };
        if w < 0 || h < 0 { return Err("WorldGen zone has negative dimensions".into()); }
        out.push(WorldGenZoneSpec {
            name: v.str("name").unwrap_or("").to_string(),
            x, y, z, w, h, geometry, points, polyline_width: line_width, rocks,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worldgen_rocks_and_malformed_cases() {
        let parsed = parse_worldgen_zones(br#"objects = {
            { name="NoRocks", type="WorldGen", x=-16, y=3, z=0, width=8, height=8, properties={ Rocks=false } },
            { name="Ordinary", type="TownZone", x=2 },
            { name="Default", type="WorldGen", x=8, y=8, z=0, width=8, height=8 },
        }"#).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].rocks, Some(false));
        assert_eq!(parsed[0].x, -16);
        assert_eq!(parsed[1].rocks, None);
        assert!(parse_worldgen_zones(br#"objects={{type="WorldGen",x=0,y=0,z=0,width=8,height=8,properties={Rocks=5}}}"#).is_err());
    }
    #[test]
    fn parses_rect_polygon_and_properties() {
        let z = parse_objects(br#"objects = {
          { name = "a", type = "TownZone", x = 1, y = 2, z = 0, width = 3, height = 4 }, -- c
          { name = "", type = "Vegitation", z = 0, geometry = "polygon", points = { 1, 2, 3, 4, 5, 6 } },
          { name = "m", type = "Mannequin", x = 9, y = 8, z = 1, width = 1, height = 1, properties = { Pose = "x", [1] = true } },
        }"#).unwrap();
        assert_eq!(z.len(), 3);
        assert_eq!(
            (z[0].kind.as_str(), z[0].x, z[0].w, z[0].h),
            ("TownZone", 1, 3, 4)
        );
        assert_eq!(z[1].points, vec![(1, 2), (3, 4), (5, 6)]);
        assert!(z[2].has_properties && !z[0].has_properties);
    }
    #[test]
    fn real_muldraugh_matches_the_lua_oracle() {
        let p = std::path::Path::new("../../../media/maps/Muldraugh, KY/objects.lua");
        let Ok(b) = std::fs::read(p) else { return };
        let z = parse_objects(&b).unwrap();
        // 37,389 zone rows counted with `grep -c '^  {'` and by the real Lua (#objects, see docs/COMPATIBILITY.md)
        assert_eq!(z.len(), 37389);
        let bad: Vec<_> = z
            .iter()
            .filter(|z| z.kind == "Mannequin" && !z.has_properties)
            .collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(
            (bad[0].x, bad[0].y, bad[0].z),
            (13583, 1299, 0),
            "same coords as the real client's error"
        );
    }
}
