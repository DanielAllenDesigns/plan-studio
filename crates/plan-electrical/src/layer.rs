//! The electrical layer: devices, Electrical Connection splines and rope lights.
//!
//! A connection is a spline (manual p. 696): two segments through a middle
//! vertex when first drawn, which looks like an arc, then any number of
//! vertices. Its ends attach to devices (`from`, `to`) or float free
//! (`from_at`, `to_at` with an id of 0). Dragging an end off its device
//! detaches it ([`ElectricalLayer::detach_end`]), dropping it on a device
//! attaches it ([`ElectricalLayer::attach_end`]) and Reset Curvature
//! ([`ElectricalLayer::reset_curvature`]) takes the vertices away again.

use crate::device::Device;
use crate::options::DeviceOptions;
use crate::rope::RopeLightPath;
use crate::symbol::Stroke;
use plan_core::geometry::{dist_to_segment, project_on_segment};
use plan_core::{Id, LineStyle, Point, Wall};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which end of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnEnd {
    Start,
    End,
}

/// An arrow head on an Electrical Connection spline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Arrow {
    #[default]
    None,
    /// At the `to` end.
    End,
    /// At the `from` end.
    Start,
    Both,
}

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::None, Arrow::End, Arrow::Start, Arrow::Both];

    pub fn name(self) -> &'static str {
        match self {
            Arrow::None => "None",
            Arrow::End => "End",
            Arrow::Start => "Start",
            Arrow::Both => "Both",
        }
    }
}

/// A switch (or outlet) to load link, drawn as a spline (dashed by default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    /// The controlling switch (or outlet); 0 when the start is not attached.
    pub from: Id,
    /// The controlled light or outlet; 0 when the end is not attached.
    pub to: Id,
    /// Signed sagitta of the first arc in inches: the distance the arc
    /// midpoint sits off the straight chord. Positive bulges to the left of
    /// `from` to `to`. Reset Curvature returns to it.
    pub arc_bulge: f64,
    /// The vertices between the ends. Empty is the original arc (two
    /// segments through its midpoint).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertices: Vec<Point>,
    /// Where an unattached start is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_at: Option<Point>,
    /// Where an unattached end is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_at: Option<Point>,
    /// Line style; `None` is dashed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_style: Option<LineStyle>,
    #[serde(default, skip_serializing_if = "is_no_arrow")]
    pub arrow: Arrow,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

fn is_no_arrow(a: &Arrow) -> bool {
    *a == Arrow::None
}

impl Connection {
    /// A connection between two devices.
    pub fn new(from: Id, to: Id, arc_bulge: f64) -> Self {
        Self {
            from,
            to,
            arc_bulge,
            vertices: Vec::new(),
            from_at: None,
            to_at: None,
            line_style: None,
            arrow: Arrow::None,
            label: String::new(),
        }
    }

    /// The line style it is drawn in.
    pub fn style(&self) -> LineStyle {
        self.line_style.unwrap_or(LineStyle::Dashed)
    }

    /// Is the connection a plain arc (no vertices were added)?
    pub fn is_arc(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// All electrical devices, connections and rope lights of one floor.
///
/// Read record by record: a device, connection or rope light this build
/// cannot parse (a newer build's device kind) is kept as raw JSON in
/// `unreadable_*` and written back after the readable ones, so one strange
/// record never takes its neighbours with it (QA-28). Keys of the layer this
/// build has no field for are kept in `extra`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElectricalLayer {
    pub devices: Vec<Device>,
    pub connections: Vec<Connection>,
    /// Rope light paths.
    pub ropes: Vec<RopeLightPath>,
    /// Options of the devices that differ from the defaults, by device id.
    pub options: BTreeMap<Id, DeviceOptions>,
    /// Device records that did not parse, as they were read.
    pub unreadable_devices: Vec<serde_json::Value>,
    /// Connection records that did not parse, as they were read.
    pub unreadable_connections: Vec<serde_json::Value>,
    /// Rope light records that did not parse, as they were read.
    pub unreadable_ropes: Vec<serde_json::Value>,
    /// Keys of the layer object that no field here holds.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ElectricalLayerDe {
    #[serde(default)]
    devices: Vec<serde_json::Value>,
    #[serde(default)]
    connections: Vec<serde_json::Value>,
    #[serde(default)]
    ropes: Vec<serde_json::Value>,
    #[serde(default)]
    options: BTreeMap<String, serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl<'de> Deserialize<'de> for ElectricalLayer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let de = ElectricalLayerDe::deserialize(d)?;
        let (devices, bad_devices) = plan_core::foreign::read_each::<Device>(&de.devices);
        let (connections, bad_connections) =
            plan_core::foreign::read_each::<Connection>(&de.connections);
        let (ropes, bad_ropes) = plan_core::foreign::read_each::<RopeLightPath>(&de.ropes);
        let options = de
            .options
            .iter()
            .filter_map(|(k, v)| {
                Some((
                    k.parse::<Id>().ok()?,
                    serde_json::from_value::<DeviceOptions>(v.clone()).ok()?,
                ))
            })
            .collect();
        Ok(ElectricalLayer {
            devices,
            connections,
            ropes,
            options,
            unreadable_devices: bad_devices.into_iter().map(|(_, v)| v).collect(),
            unreadable_connections: bad_connections.into_iter().map(|(_, v)| v).collect(),
            unreadable_ropes: bad_ropes.into_iter().map(|(_, v)| v).collect(),
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
        if !self.ropes.is_empty() || !self.unreadable_ropes.is_empty() {
            m.serialize_entry(
                "ropes",
                &Records {
                    typed: &self.ropes,
                    raw: &self.unreadable_ropes,
                },
            )?;
        }
        if !self.options.is_empty() {
            m.serialize_entry("options", &self.options)?;
        }
        for (k, v) in &self.extra {
            if !matches!(k.as_str(), "devices" | "connections" | "ropes" | "options") {
                m.serialize_entry(k, v)?;
            }
        }
        m.end()
    }
}

/// Segments an arc is sampled into.
const ARC_SAMPLES: usize = 24;
/// Samples per spline segment through vertices.
const SPLINE_SAMPLES: usize = 10;

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
        self.options.remove(&id);
        for d in &mut self.devices {
            d.switched_by.retain(|s| *s != id);
        }
        normalize_switch_kinds(self, true);
    }

    // ----- device options -----

    /// The options of device `id` (the defaults for a device with none).
    pub fn options_of(&self, id: Id) -> DeviceOptions {
        self.options.get(&id).cloned().unwrap_or_default()
    }

    /// Stores the options of device `id`; defaults store nothing.
    pub fn set_options(&mut self, id: Id, options: DeviceOptions) {
        if options.is_default() {
            self.options.remove(&id);
        } else {
            self.options.insert(id, options);
        }
    }

    // ----- rope lights -----

    /// Adds a rope light and returns its id (a used or zero id is replaced).
    pub fn add_rope(&mut self, mut rope: RopeLightPath) -> Id {
        let used = |id: Id, this: &Self| {
            this.ropes.iter().any(|r| r.id == id)
                || this
                    .unreadable_ropes
                    .iter()
                    .any(|v| v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
        };
        if rope.id == 0 || used(rope.id, self) {
            let top = self.ropes.iter().map(|r| r.id).max().unwrap_or(0);
            let top_raw = self
                .unreadable_ropes
                .iter()
                .filter_map(|v| v.get("id").and_then(serde_json::Value::as_u64))
                .max()
                .unwrap_or(0);
            rope.id = top.max(top_raw) + 1;
        }
        let id = rope.id;
        self.ropes.push(rope);
        id
    }

    pub fn rope(&self, id: Id) -> Option<&RopeLightPath> {
        self.ropes.iter().find(|r| r.id == id)
    }

    pub fn rope_mut(&mut self, id: Id) -> Option<&mut RopeLightPath> {
        self.ropes.iter_mut().find(|r| r.id == id)
    }

    pub fn remove_rope(&mut self, id: Id) -> bool {
        let before = self.ropes.len();
        self.ropes.retain(|r| r.id != id);
        self.ropes.len() != before
    }

    /// The rope light whose path passes within `tol` of `p`.
    pub fn rope_at(&self, p: Point, tol: f64) -> Option<Id> {
        self.ropes
            .iter()
            .map(|r| (r.id, r.distance_to(p)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    // ----- connection geometry -----

    /// Where an end is: the device's position, or the free point.
    fn end_point(&self, id: Id, at: Option<Point>) -> Option<Point> {
        if id != 0 {
            Some(self.device(id)?.position)
        } else {
            at
        }
    }

    /// The two end points of a connection.
    pub fn connection_ends(&self, c: &Connection) -> Option<(Point, Point)> {
        Some((
            self.end_point(c.from, c.from_at)?,
            self.end_point(c.to, c.to_at)?,
        ))
    }

    /// The circular arc of a connection from its first bulge: an
    /// [`Stroke::Arc`] through both ends, or a [`Stroke::Line`] when the
    /// bulge is zero. (A connection with vertices follows
    /// [`connection_path`](Self::connection_path) instead.)
    pub fn connection_arc(&self, c: &Connection) -> Option<Stroke> {
        let (a, b) = self.connection_ends(c)?;
        Some(arc_through(a, b, c.arc_bulge))
    }

    /// The point halfway along a connection's arc: its bend handle.
    pub fn arc_midpoint(&self, c: &Connection) -> Option<Point> {
        let (a, b) = self.connection_ends(c)?;
        let n = b.sub(a).perp().normalized();
        Some(Point::lerp(a, b, 0.5) + n * c.arc_bulge)
    }

    /// The vertices of a connection's spline, ends included: the edit handles
    /// (start, one per vertex, end). A plain arc has its midpoint as the one
    /// vertex between the ends.
    pub fn connection_handles(&self, c: &Connection) -> Option<Vec<Point>> {
        let (a, b) = self.connection_ends(c)?;
        let mut v = vec![a];
        if c.vertices.is_empty() {
            v.push(self.arc_midpoint(c)?);
        } else {
            v.extend(c.vertices.iter().copied());
        }
        v.push(b);
        Some(v)
    }

    /// The path of a connection as a polyline: the arc sampled, or the smooth
    /// spline through its vertices.
    pub fn connection_path(&self, c: &Connection) -> Option<Vec<Point>> {
        let (a, b) = self.connection_ends(c)?;
        if c.vertices.is_empty() {
            return Some(match arc_through(a, b, c.arc_bulge) {
                Stroke::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                } => (0..=ARC_SAMPLES)
                    .map(|i| {
                        let t = start + sweep * i as f64 / ARC_SAMPLES as f64;
                        Point::new(center.x + radius * t.cos(), center.y + radius * t.sin())
                    })
                    .collect(),
                _ => vec![a, b],
            });
        }
        let mut pts = vec![a];
        pts.extend(c.vertices.iter().copied());
        pts.push(b);
        Some(smooth_through(&pts, SPLINE_SAMPLES))
    }

    /// Curvature ratio of a plain arc: the sagitta over the chord.
    pub fn curvature_ratio(&self, c: &Connection) -> Option<f64> {
        let (a, b) = self.connection_ends(c)?;
        let chord = a.dist(b);
        (chord > 1e-9).then(|| c.arc_bulge.abs() / chord)
    }

    /// The connection whose bend handle (an interior vertex) is nearest `p`,
    /// within `tol` inches.
    pub fn connection_handle_at(&self, p: Point, tol: f64) -> Option<usize> {
        self.connection_handle_hit(p, tol, false).map(|(i, _)| i)
    }

    /// The handle nearest `p` within `tol`: `(connection index, handle
    /// index)` where handle 0 is the start and the last the end. With
    /// `ends_too` false only the interior vertices count.
    pub fn connection_handle_hit(
        &self,
        p: Point,
        tol: f64,
        ends_too: bool,
    ) -> Option<(usize, usize)> {
        let mut best: Option<((usize, usize), f64)> = None;
        for (i, c) in self.connections.iter().enumerate() {
            let Some(h) = self.connection_handles(c) else {
                continue;
            };
            for (j, q) in h.iter().enumerate() {
                if !ends_too && (j == 0 || j + 1 == h.len()) {
                    continue;
                }
                let d = q.dist(p);
                if d <= tol && best.as_ref().is_none_or(|(_, bd)| d < *bd) {
                    best = Some(((i, j), d));
                }
            }
        }
        best.map(|(h, _)| h)
    }

    /// The connection whose spline passes within `tol` of `p`.
    pub fn connection_at(&self, p: Point, tol: f64) -> Option<usize> {
        self.connections
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let path = self.connection_path(c)?;
                let d = path
                    .windows(2)
                    .map(|w| dist_to_segment(p, w[0], w[1]))
                    .fold(f64::INFINITY, f64::min);
                (d <= tol).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Bend connection `index` so its arc passes through `to`: the bulge becomes
    /// the signed distance of `to` from the chord (at most half the chord).
    /// Returns `false` for an unknown connection or ends at one spot.
    pub fn bend_connection(&mut self, index: usize, to: Point) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        let Some((a, b)) = self.connection_ends(c) else {
            return false;
        };
        let chord = a.dist(b);
        if chord < 1e-6 {
            return false;
        }
        let n = b.sub(a).perp().normalized();
        let bulge = to.sub(Point::lerp(a, b, 0.5)).dot(n);
        self.connections[index].arc_bulge = bulge.clamp(-chord * 0.5, chord * 0.5);
        true
    }

    /// Moves interior handle `handle` of connection `index` to `to`: a plain
    /// arc bends, a spline with vertices moves that vertex. The ends are
    /// moved by [`detach_end`](Self::detach_end) / [`attach_end`](Self::attach_end).
    pub fn move_handle(&mut self, index: usize, handle: usize, to: Point) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        if c.vertices.is_empty() {
            return handle == 1 && self.bend_connection(index, to);
        }
        if handle == 0 || handle > c.vertices.len() {
            return false;
        }
        self.connections[index].vertices[handle - 1] = to;
        true
    }

    /// Adds a vertex to connection `index` at the point of its control
    /// polygon nearest `at`; a plain arc first becomes a spline through its
    /// midpoint. Returns the new handle's index.
    pub fn insert_vertex(&mut self, index: usize, at: Point) -> Option<usize> {
        let c = self.connections.get(index)?;
        let mut handles = self.connection_handles(c)?;
        let seg = (0..handles.len() - 1)
            .map(|i| (i, dist_to_segment(at, handles[i], handles[i + 1])))
            .min_by(|a, b| a.1.total_cmp(&b.1))?
            .0;
        handles.insert(seg + 1, at);
        let c = &mut self.connections[index];
        c.vertices = handles[1..handles.len() - 1].to_vec();
        Some(seg + 1)
    }

    /// Removes interior handle `handle`; the last vertex turns the spline
    /// back into an arc through where it was.
    pub fn remove_vertex(&mut self, index: usize, handle: usize) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        if c.vertices.is_empty() || handle == 0 || handle > c.vertices.len() {
            return false;
        }
        let last = c.vertices.len() == 1;
        let gone = self.connections[index].vertices.remove(handle - 1);
        if last {
            self.connections[index].vertices.clear();
            self.bend_connection(index, gone);
        }
        true
    }

    /// Reset Curvature (manual p. 697): takes the vertices away, restores the
    /// original curvature direction and applies `ratio` (the Electrical
    /// Connection Defaults' curvature ratio).
    pub fn reset_curvature(&mut self, index: usize, ratio: f64) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        let Some((a, b)) = self.connection_ends(c) else {
            return false;
        };
        let chord = a.dist(b);
        let sign = if c.arc_bulge < 0.0 { -1.0 } else { 1.0 };
        let c = &mut self.connections[index];
        c.vertices.clear();
        c.arc_bulge = sign * (chord * ratio.clamp(0.0, 0.5));
        true
    }

    /// A spline with both ends free at `a` and `b`, curved by `d`; returns
    /// its index.
    pub fn add_free_connection(
        &mut self,
        a: Point,
        b: Point,
        d: &crate::defaults::ConnectionDefaults,
    ) -> usize {
        let mut c = Connection::new(0, 0, a.dist(b) * d.curvature_ratio.clamp(0.0, 0.5));
        c.from_at = Some(a);
        c.to_at = Some(b);
        self.apply_style(&mut c, d);
        self.connections.push(c);
        self.connections.len() - 1
    }

    fn apply_style(&self, c: &mut Connection, d: &crate::defaults::ConnectionDefaults) {
        c.line_style = (d.line_style != LineStyle::Dashed).then_some(d.line_style);
        c.arrow = d.arrow;
        c.label = d.label.clone();
    }

    /// Detaches an end of connection `index` from its device and leaves it
    /// free at `at` ("remove by dragging an end off"). The control the
    /// connection gave (a light switched by a switch) goes with it.
    pub fn detach_end(&mut self, index: usize, end: ConnEnd, at: Point) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        let (from, to) = (c.from, c.to);
        let attached = match end {
            ConnEnd::Start => from,
            ConnEnd::End => to,
        };
        if attached == 0 {
            // Already free: just move it.
            let c = &mut self.connections[index];
            match end {
                ConnEnd::Start => c.from_at = Some(at),
                ConnEnd::End => c.to_at = Some(at),
            }
            return true;
        }
        let group = self.switch_group(from);
        let c = &mut self.connections[index];
        match end {
            ConnEnd::Start => {
                c.from = 0;
                c.from_at = Some(at);
            }
            ConnEnd::End => {
                c.to = 0;
                c.to_at = Some(at);
            }
        }
        if from != 0 && to != 0 {
            unwire(self, from, to, &group);
        }
        true
    }

    /// Attaches an end of connection `index` to `device` (dropping the end on
    /// it). Refused (`false`) for an unknown device, the same device at both
    /// ends or a pair that is already connected. A switch-to-load pair is
    /// wired like [`connect`].
    pub fn attach_end(&mut self, index: usize, end: ConnEnd, device: Id) -> bool {
        let Some(c) = self.connections.get(index) else {
            return false;
        };
        if self.device(device).is_none() {
            return false;
        }
        let (mut from, mut to) = (c.from, c.to);
        match end {
            ConnEnd::Start => from = device,
            ConnEnd::End => to = device,
        }
        if from == to && from != 0 {
            return false;
        }
        let duplicate = self.connections.iter().enumerate().any(|(i, o)| {
            i != index
                && from != 0
                && to != 0
                && ((o.from == from && o.to == to) || (o.from == to && o.to == from))
        });
        if duplicate {
            return false;
        }
        let was_wired = c.from != 0 && c.to != 0;
        let (old_from, old_to) = (c.from, c.to);
        let group = self.switch_group(old_from);
        let c = &mut self.connections[index];
        match end {
            ConnEnd::Start => {
                c.from = device;
                c.from_at = None;
            }
            ConnEnd::End => {
                c.to = device;
                c.to_at = None;
            }
        }
        if was_wired {
            unwire(self, old_from, old_to, &group);
        }
        if from != 0 && to != 0 && wiring_allowed(self, from, to) {
            wire(self, from, to);
        }
        true
    }

    /// Deletes connection `index` and the control it gave (a light switched
    /// by a switch). Returns whether there was one.
    pub fn remove_connection(&mut self, index: usize) -> bool {
        if index >= self.connections.len() {
            return false;
        }
        let group = self.switch_group(self.connections[index].from);
        let c = self.connections.remove(index);
        if c.from != 0 && c.to != 0 {
            unwire(self, c.from, c.to, &group);
        }
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

/// Catmull-Rom spline through `pts`, `per` samples per segment.
fn smooth_through(pts: &[Point], per: usize) -> Vec<Point> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let at = |i: isize| pts[i.clamp(0, n as isize - 1) as usize];
    let mut out = vec![pts[0]];
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (
            at(i as isize - 1),
            at(i as isize),
            at(i as isize + 1),
            at(i as isize + 2),
        );
        for k in 1..=per {
            let t = k as f64 / per as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(Point::new(
                f(p0.x, p1.x, p2.x, p3.x),
                f(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    out
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

/// Signed sagitta for the chord `a`-`b` so the arc bulges toward `away`: the
/// chord times the curvature `ratio`, at most half the chord.
fn bulge_toward(a: Point, b: Point, away: Point, ratio: f64) -> f64 {
    let magnitude = a.dist(b) * ratio.clamp(0.0, 0.5);
    let n = b.sub(a).perp().normalized();
    if n.dot(away) < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

/// May `switch` control `device` (a switch or outlet to a load, or two
/// 3-way / 4-way switches as a traveler pair)?
fn wiring_allowed(layer: &ElectricalLayer, switch: Id, device: Id) -> bool {
    let (Some(from), Some(to)) = (layer.device(switch), layer.device(device)) else {
        return false;
    };
    let traveler = |k: crate::DeviceKind| {
        matches!(
            k,
            crate::DeviceKind::Switch3Way | crate::DeviceKind::Switch4Way
        )
    };
    if !(from.kind.is_switch() || from.kind.is_outlet()) {
        return false;
    }
    !(to.kind.is_switch() && !(traveler(from.kind) && traveler(to.kind)))
}

/// Records that `switch` controls `device` (a load) and promotes the
/// switches of a multi-way circuit.
fn wire(layer: &mut ElectricalLayer, switch: Id, device: Id) {
    if let Some(d) = layer.device_mut(device) {
        if !d.kind.is_switch() && !d.switched_by.contains(&switch) {
            d.switched_by.push(switch);
        }
    }
    layer.spread_switches();
    normalize_switch_kinds(layer, false);
}

fn link(
    layer: &mut ElectricalLayer,
    switch: Id,
    device: Id,
    d: &crate::defaults::ConnectionDefaults,
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
    let mut c = Connection::new(switch, device, arc_bulge);
    layer.apply_style(&mut c, d);
    layer.connections.push(c);
    wire(layer, switch, device);
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
/// that are not wired to any load, every other device and a switch whose
/// Automatically Change Switch Type When Wiring option is off are left alone.
pub fn normalize_switch_kinds(layer: &mut ElectricalLayer, demote: bool) {
    use crate::DeviceKind as K;
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
        let auto = layer.options_of(id).auto_switch_type;
        let Some(d) = layer.device_mut(id) else {
            continue;
        };
        if auto && matches!(d.kind, K::Switch | K::Switch3Way | K::Switch4Way) && (demote || r > 0)
        {
            d.kind = match r {
                2 => K::Switch3Way,
                1 => K::Switch4Way,
                _ => K::Switch,
            };
        }
    }
}

/// After the link `from` to `to` is gone: `to` is no longer switched by
/// `from` or its 3-way partners (`group`, taken before), unless another
/// connection still wires them.
fn unwire(layer: &mut ElectricalLayer, from: Id, to: Id, group: &[Id]) {
    let _ = from;
    let keep: Vec<Id> = layer
        .connections
        .iter()
        .filter(|c| c.to == to && c.from != 0)
        .flat_map(|c| layer.switch_group(c.from))
        .collect();
    if let Some(d) = layer.device_mut(to) {
        d.switched_by
            .retain(|s| keep.contains(s) || !group.contains(s));
    }
    normalize_switch_kinds(layer, true);
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
    unwire(layer, from, to, &group);
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
    let d = crate::defaults::ConnectionDefaults::default();
    link(layer, switch, device, &d, |from, to| {
        let facing = Point::new(from.angle.cos(), from.angle.sin());
        bulge_toward(from.position, to.position, facing, d.curvature_ratio)
    })
}

/// Like [`connect`], but the arc bulges away from the wall nearest the chord midpoint.
pub fn connect_in(layer: &mut ElectricalLayer, switch: Id, device: Id, walls: &[Wall]) -> bool {
    connect_with(
        layer,
        switch,
        device,
        walls,
        &crate::defaults::ConnectionDefaults::default(),
    )
}

/// [`connect_in`] with the plan's Electrical Connection Defaults: the arc's
/// curvature ratio, line style, arrow and label.
pub fn connect_with(
    layer: &mut ElectricalLayer,
    switch: Id,
    device: Id,
    walls: &[Wall],
    defaults: &crate::defaults::ConnectionDefaults,
) -> bool {
    link(layer, switch, device, defaults, |from, to| {
        let mid = Point::lerp(from.position, to.position, 0.5);
        let nearest = walls
            .iter()
            .map(|w| project_on_segment(mid, w.start, w.end).1)
            .min_by(|p, q| p.dist(mid).total_cmp(&q.dist(mid)));
        let away = match nearest {
            Some(q) if q.dist(mid) > 1e-6 => mid.sub(q),
            _ => Point::new(from.angle.cos(), from.angle.sin()),
        };
        bulge_toward(from.position, to.position, away, defaults.curvature_ratio)
    })
}

/// Draws a plain wiring spline between any two devices (no control is
/// recorded): the Electrical Connection tool's drag from one object to the
/// next. Returns the new connection's index, or `None` for unknown devices,
/// the same device twice or a pair already connected.
pub fn connect_drawn(
    layer: &mut ElectricalLayer,
    a: Id,
    b: Id,
    walls: &[Wall],
    defaults: &crate::defaults::ConnectionDefaults,
) -> Option<usize> {
    if a == b || layer.device(a).is_none() || layer.device(b).is_none() {
        return None;
    }
    if layer
        .connections
        .iter()
        .any(|c| (c.from == a && c.to == b) || (c.from == b && c.to == a))
    {
        return None;
    }
    if wiring_allowed(layer, a, b) {
        return connect_with(layer, a, b, walls, defaults).then(|| layer.connections.len() - 1);
    }
    // Not a switch-to-load pair: a plain spline.
    let (pa, pb) = (layer.device(a)?.position, layer.device(b)?.position);
    let mid = Point::lerp(pa, pb, 0.5);
    let nearest = walls
        .iter()
        .map(|w| project_on_segment(mid, w.start, w.end).1)
        .min_by(|p, q| p.dist(mid).total_cmp(&q.dist(mid)));
    let away = match nearest {
        Some(q) if q.dist(mid) > 1e-6 => mid.sub(q),
        _ => Point::new(1.0, 0.0),
    };
    let mut c = Connection::new(a, b, bulge_toward(pa, pb, away, defaults.curvature_ratio));
    layer.apply_style(&mut c, defaults);
    layer.connections.push(c);
    Some(layer.connections.len() - 1)
}
