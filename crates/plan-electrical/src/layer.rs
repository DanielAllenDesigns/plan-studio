//! The electrical layer: devices plus switch-to-load connections.

use crate::device::Device;
use crate::symbol::Stroke;
use plan_core::geometry::project_on_segment;
use plan_core::{Id, Point, Wall};
use serde::{Deserialize, Serialize};

/// A switch (or outlet) to load link, drawn as a curved dashed line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    /// The controlling switch (or outlet).
    pub from: Id,
    /// The controlled light or outlet.
    pub to: Id,
    /// Signed sagitta of the arc in inches: the distance the arc midpoint sits
    /// off the straight chord. Positive bulges to the left of `from` to `to`.
    pub arc_bulge: f64,
}

/// All electrical devices and connections of one floor.
///
/// Read record by record: a device or connection this build cannot parse (a
/// newer build's device kind) is kept as raw JSON in `unreadable_devices` /
/// `unreadable_connections` and written back after the readable ones, so one
/// strange record never takes its neighbours with it (QA-28). Keys of the
/// layer this build has no field for are kept in `extra`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElectricalLayer {
    pub devices: Vec<Device>,
    pub connections: Vec<Connection>,
    /// Device records that did not parse, as they were read.
    pub unreadable_devices: Vec<serde_json::Value>,
    /// Connection records that did not parse, as they were read.
    pub unreadable_connections: Vec<serde_json::Value>,
    /// Keys of the layer object that no field here holds.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ElectricalLayerDe {
    #[serde(default)]
    devices: Vec<serde_json::Value>,
    #[serde(default)]
    connections: Vec<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl<'de> Deserialize<'de> for ElectricalLayer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let de = ElectricalLayerDe::deserialize(d)?;
        let (devices, bad_devices) = plan_core::foreign::read_each::<Device>(&de.devices);
        let (connections, bad_connections) =
            plan_core::foreign::read_each::<Connection>(&de.connections);
        Ok(ElectricalLayer {
            devices,
            connections,
            unreadable_devices: bad_devices.into_iter().map(|(_, v)| v).collect(),
            unreadable_connections: bad_connections.into_iter().map(|(_, v)| v).collect(),
            extra: de.extra,
        })
    }
}

/// A list of typed records followed by raw ones, written as one JSON array.
struct Records<'a, T> {
    typed: &'a [T],
    raw: &'a [serde_json::Value],
}

impl<T: Serialize> Serialize for Records<'_, T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.typed.len() + self.raw.len()))?;
        for t in self.typed {
            seq.serialize_element(t)?;
        }
        for r in self.raw {
            seq.serialize_element(r)?;
        }
        seq.end()
    }
}

impl Serialize for ElectricalLayer {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        m.serialize_entry(
            "devices",
            &Records {
                typed: &self.devices,
                raw: &self.unreadable_devices,
            },
        )?;
        m.serialize_entry(
            "connections",
            &Records {
                typed: &self.connections,
                raw: &self.unreadable_connections,
            },
        )?;
        for (k, v) in &self.extra {
            if k != "devices" && k != "connections" {
                m.serialize_entry(k, v)?;
            }
        }
        m.end()
    }
}

/// Arc sagitta as a fraction of the chord, clamped to a readable range (inches).
const BULGE_FRACTION: f64 = 0.2;
const BULGE_MIN: f64 = 6.0;
const BULGE_MAX: f64 = 48.0;

impl ElectricalLayer {
    pub fn device(&self, id: Id) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub fn device_mut(&mut self, id: Id) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    /// Add a device and return its id. A zero or already-used id is replaced
    /// with the next free one; any other id is kept.
    pub fn add(&mut self, mut device: Device) -> Id {
        let unreadable = |id: Id| {
            self.unreadable_devices
                .iter()
                .any(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
        };
        if device.id == 0 || self.device(device.id).is_some() || unreadable(device.id) {
            let top = self.devices.iter().map(|d| d.id).max().unwrap_or(0);
            let top_raw = self
                .unreadable_devices
                .iter()
                .filter_map(|v| v.get("id").and_then(serde_json::Value::as_u64))
                .max()
                .unwrap_or(0);
            device.id = top.max(top_raw) + 1;
        }
        let id = device.id;
        self.devices.push(device);
        id
    }

    /// Add many devices; returns their ids in order.
    pub fn add_all(&mut self, devices: impl IntoIterator<Item = Device>) -> Vec<Id> {
        devices.into_iter().map(|d| self.add(d)).collect()
    }

    /// Remove a device and every connection that touches it.
    pub fn remove(&mut self, id: Id) {
        self.devices.retain(|d| d.id != id);
        self.connections.retain(|c| c.from != id && c.to != id);
        for d in &mut self.devices {
            d.switched_by.retain(|s| *s != id);
        }
        normalize_switch_kinds(self, true);
    }

    /// The curved dashed line for a connection: an [`Stroke::Arc`] through both
    /// device positions, or a [`Stroke::Line`] when the bulge is zero.
    pub fn connection_arc(&self, c: &Connection) -> Option<Stroke> {
        let a = self.device(c.from)?.position;
        let b = self.device(c.to)?.position;
        Some(arc_through(a, b, c.arc_bulge))
    }

    /// The point halfway along a connection's arc: its bend handle.
    pub fn arc_midpoint(&self, c: &Connection) -> Option<Point> {
        let a = self.device(c.from)?.position;
        let b = self.device(c.to)?.position;
        let n = b.sub(a).perp().normalized();
        Some(Point::lerp(a, b, 0.5) + n * c.arc_bulge)
    }

    /// The connection whose bend handle is nearest `p`, within `tol` inches.
    pub fn connection_handle_at(&self, p: Point, tol: f64) -> Option<usize> {
        self.connections
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Some((i, self.arc_midpoint(c)?.dist(p))))
            .filter(|(_, d)| *d <= tol)
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|(i, _)| i)
    }

    /// Bend connection `index` so its arc passes through `to`: the bulge becomes
    /// the signed distance of `to` from the chord (at most half the chord).
    /// Returns `false` for an unknown connection or devices at one spot.
    pub fn bend_connection(&mut self, index: usize, to: Point) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        let (Some(a), Some(b)) = (self.device(c.from), self.device(c.to)) else {
            return false;
        };
        let (a, b) = (a.position, b.position);
        let chord = a.dist(b);
        if chord < 1e-6 {
            return false;
        }
        let n = b.sub(a).perp().normalized();
        let bulge = to.sub(Point::lerp(a, b, 0.5)).dot(n);
        self.connections[index].arc_bulge = bulge.clamp(-chord * 0.5, chord * 0.5);
        true
    }

    /// `switch` and every switch wired to it by a traveler connection (a link
    /// between two 3-way or 4-way switches): the switches that all control the
    /// same loads. The switch itself comes first.
    pub fn switch_group(&self, switch: Id) -> Vec<Id> {
        let mut group = vec![switch];
        let mut i = 0;
        while i < group.len() {
            let at = group[i];
            for c in &self.connections {
                let other = if c.from == at {
                    c.to
                } else if c.to == at {
                    c.from
                } else {
                    continue;
                };
                let both = self.device(at).is_some_and(|d| d.kind.is_switch())
                    && self.device(other).is_some_and(|d| d.kind.is_switch());
                if both && !group.contains(&other) {
                    group.push(other);
                }
            }
            i += 1;
        }
        group
    }

    /// Makes every load wired to a switch also switched by the rest of that
    /// switch's group (so both ends of a 3-way pair control the light).
    fn spread_switches(&mut self) {
        let wired: Vec<(Id, Id)> = self
            .connections
            .iter()
            .filter(|c| self.device(c.to).is_some_and(|d| !d.kind.is_switch()))
            .map(|c| (c.from, c.to))
            .collect();
        for (from, to) in wired {
            if !self.device(from).is_some_and(|d| d.kind.is_switch()) {
                continue;
            }
            for s in self.switch_group(from) {
                if let Some(d) = self.device_mut(to) {
                    if !d.switched_by.contains(&s) {
                        d.switched_by.push(s);
                    }
                }
            }
        }
    }

    /// The loads a switch controls: its own connections plus those of its
    /// group, in device order.
    pub fn loads_of(&self, switch: Id) -> Vec<Id> {
        self.devices
            .iter()
            .filter(|d| !d.kind.is_switch() && d.switched_by.contains(&switch))
            .map(|d| d.id)
            .collect()
    }
}

/// Arc from `a` to `b` whose midpoint sits `bulge` inches off the chord.
fn arc_through(a: Point, b: Point, bulge: f64) -> Stroke {
    let chord = a.dist(b);
    if bulge.abs() < 1e-6 || chord < 1e-6 {
        return Stroke::Line { a, b };
    }
    let n = b.sub(a).perp().normalized();
    let mid = Point::lerp(a, b, 0.5);
    let radius = (chord * chord * 0.25 + bulge * bulge) / (2.0 * bulge.abs());
    let apex = mid + n * bulge;
    let center = apex - n * (bulge.signum() * radius);
    let start = a.sub(center).angle();
    let end = b.sub(center).angle();
    // Travel from a to b through the apex: CCW when apex is CCW of a about the center.
    let ccw = a.sub(center).cross(apex.sub(center)) > 0.0;
    let tau = std::f64::consts::TAU;
    let sweep = if ccw {
        (end - start).rem_euclid(tau)
    } else {
        -((start - end).rem_euclid(tau))
    };
    Stroke::Arc {
        center,
        radius,
        start,
        sweep,
    }
}

/// Signed sagitta for the chord `a`-`b` so the arc bulges toward `away`.
fn bulge_toward(a: Point, b: Point, away: Point) -> f64 {
    let magnitude = (a.dist(b) * BULGE_FRACTION).clamp(BULGE_MIN, BULGE_MAX);
    let n = b.sub(a).perp().normalized();
    if n.dot(away) < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

fn link(
    layer: &mut ElectricalLayer,
    switch: Id,
    device: Id,
    bulge: impl FnOnce(&Device, &Device) -> f64,
) -> bool {
    if switch == device
        || layer
            .connections
            .iter()
            .any(|c| (c.from == switch && c.to == device) || (c.from == device && c.to == switch))
    {
        return false;
    }
    let (Some(from), Some(to)) = (layer.device(switch), layer.device(device)) else {
        return false;
    };
    // A switch only wires to another switch as a 3-way / 4-way traveler pair.
    let traveler = |k: crate::DeviceKind| {
        matches!(
            k,
            crate::DeviceKind::Switch3Way | crate::DeviceKind::Switch4Way
        )
    };
    if to.kind.is_switch() && !(traveler(from.kind) && traveler(to.kind)) {
        return false;
    }
    let arc_bulge = bulge(from, to);
    layer.connections.push(Connection {
        from: switch,
        to: device,
        arc_bulge,
    });
    if let Some(d) = layer.device_mut(device) {
        if !d.kind.is_switch() && !d.switched_by.contains(&switch) {
            d.switched_by.push(switch);
        }
    }
    layer.spread_switches();
    normalize_switch_kinds(layer, false);
    true
}

/// Gives every wired switch the symbol its wiring needs (S, S3 or S4).
///
/// A load controlled by two or more switches (directly or through a
/// traveler pair) is a multi-way circuit: the first and last switch that
/// control it are 3-way switches and the ones between are 4-way switches.
/// With `demote`, a switch that is connected to loads but is the only control
/// of each goes back to a plain single-pole switch (used after a connection
/// or device is removed; a new connection only ever promotes, so a 3-way
/// switch can be wired to its light before its partner). Dimmers, switches
/// that are not wired to any load and every other device are left alone.
pub fn normalize_switch_kinds(layer: &mut ElectricalLayer, demote: bool) {
    use crate::DeviceKind as K;
    use std::collections::BTreeMap;
    // 0 = single, 1 = 4-way (middle), 2 = 3-way (end); the highest wins.
    let mut role: BTreeMap<Id, u8> = BTreeMap::new();
    for load in layer.devices.iter().filter(|d| !d.kind.is_switch()) {
        let sw: Vec<Id> = load
            .switched_by
            .iter()
            .copied()
            .filter(|s| layer.device(*s).is_some_and(|d| d.kind.is_switch()))
            .collect();
        let last = sw.len().saturating_sub(1);
        for (i, s) in sw.iter().enumerate() {
            let r = if sw.len() < 2 {
                0
            } else if i == 0 || i == last {
                2
            } else {
                1
            };
            let e = role.entry(*s).or_insert(0);
            *e = (*e).max(r);
        }
    }
    for (id, r) in role {
        let Some(d) = layer.device_mut(id) else {
            continue;
        };
        if matches!(d.kind, K::Switch | K::Switch3Way | K::Switch4Way) && (demote || r > 0) {
            d.kind = match r {
                2 => K::Switch3Way,
                1 => K::Switch4Way,
                _ => K::Switch,
            };
        }
    }
}

/// Removes the connection from `from` to `to` (either direction between two
/// switches) and the control it gave: `to` is no longer switched by `from`
/// or by its 3-way partners, unless another connection still wires them.
/// Returns `false` when there was no such connection.
pub fn disconnect(layer: &mut ElectricalLayer, from: Id, to: Id) -> bool {
    let group = layer.switch_group(from);
    let before = layer.connections.len();
    layer
        .connections
        .retain(|c| !(c.from == from && c.to == to) && !(c.from == to && c.to == from));
    if layer.connections.len() == before {
        return false;
    }
    let keep: Vec<Id> = layer
        .connections
        .iter()
        .filter(|c| c.to == to)
        .flat_map(|c| layer.switch_group(c.from))
        .collect();
    if let Some(d) = layer.device_mut(to) {
        d.switched_by
            .retain(|s| keep.contains(s) || !group.contains(s));
    }
    normalize_switch_kinds(layer, true);
    true
}

/// Connect `switch` to `device` with a curved line.
///
/// Without wall geometry the arc bulges away from the switch's host wall, that
/// is, in the direction the switch faces. Use [`connect_in`] to bulge away from
/// the wall nearest the connection instead. Returns `false` (and changes
/// nothing) when either id is unknown, both are the same device, or the pair is
/// already connected.
pub fn connect(layer: &mut ElectricalLayer, switch: Id, device: Id) -> bool {
    link(layer, switch, device, |from, to| {
        let facing = Point::new(from.angle.cos(), from.angle.sin());
        bulge_toward(from.position, to.position, facing)
    })
}

/// Like [`connect`], but the arc bulges away from the wall nearest the chord midpoint.
pub fn connect_in(layer: &mut ElectricalLayer, switch: Id, device: Id, walls: &[Wall]) -> bool {
    link(layer, switch, device, |from, to| {
        let mid = Point::lerp(from.position, to.position, 0.5);
        let nearest = walls
            .iter()
            .map(|w| project_on_segment(mid, w.start, w.end).1)
            .min_by(|p, q| p.dist(mid).total_cmp(&q.dist(mid)));
        let away = match nearest {
            Some(q) if q.dist(mid) > 1e-6 => mid.sub(q),
            _ => Point::new(from.angle.cos(), from.angle.sin()),
        };
        bulge_toward(from.position, to.position, away)
    })
}
