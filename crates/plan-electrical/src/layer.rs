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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElectricalLayer {
    pub devices: Vec<Device>,
    pub connections: Vec<Connection>,
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
        if device.id == 0 || self.device(device.id).is_some() {
            device.id = self.devices.iter().map(|d| d.id).max().unwrap_or(0) + 1;
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
    }

    /// The curved dashed line for a connection: an [`Stroke::Arc`] through both
    /// device positions, or a [`Stroke::Line`] when the bulge is zero.
    pub fn connection_arc(&self, c: &Connection) -> Option<Stroke> {
        let a = self.device(c.from)?.position;
        let b = self.device(c.to)?.position;
        Some(arc_through(a, b, c.arc_bulge))
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
            .any(|c| c.from == switch && c.to == device)
    {
        return false;
    }
    let (Some(from), Some(to)) = (layer.device(switch), layer.device(device)) else {
        return false;
    };
    let arc_bulge = bulge(from, to);
    layer.connections.push(Connection {
        from: switch,
        to: device,
        arc_bulge,
    });
    if let Some(d) = layer.device_mut(device) {
        if !d.switched_by.contains(&switch) {
            d.switched_by.push(switch);
        }
    }
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
