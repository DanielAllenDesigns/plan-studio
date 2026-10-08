//! Circuit grouping, the device schedule and the symbol legend.

use crate::device::{Device, DeviceKind};
use crate::layer::ElectricalLayer;
use plan_core::Id;

/// A branch circuit and the devices on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Circuit {
    pub number: u32,
    pub amps: u32,
    pub devices: Vec<Id>,
    pub description: String,
}

/// Grouping rules for [`circuits`].
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitOptions {
    /// General receptacles per circuit (20 A).
    pub outlets_per_circuit: usize,
    pub outlet_amps: u32,
    /// Lights and detectors per circuit (15 A).
    pub lights_per_circuit: usize,
    pub light_amps: u32,
    /// Amperage of each dedicated 220 V circuit.
    pub dedicated_amps: u32,
    /// Circuits the kitchen counter GFCIs are split across (at least).
    pub kitchen_counter_circuits: usize,
    /// Receptacles at or above this height count as counter outlets, inches.
    pub counter_min_height: f64,
    pub first_number: u32,
}

impl Default for CircuitOptions {
    fn default() -> Self {
        Self {
            outlets_per_circuit: 10,
            outlet_amps: 20,
            lights_per_circuit: 12,
            light_amps: 15,
            dedicated_amps: 30,
            kitchen_counter_circuits: 2,
            counter_min_height: 40.0,
            first_number: 1,
        }
    }
}

/// Greedy nearest-neighbour clusters of at most `cap` devices each.
///
/// Starts from the lowest (x, y) device and repeatedly adds the remaining
/// device closest to the last one added.
fn cluster(devices: &[&Device], cap: usize) -> Vec<Vec<Id>> {
    let cap = cap.max(1);
    let mut left: Vec<&Device> = devices.to_vec();
    left.sort_by(|a, b| {
        a.position
            .x
            .total_cmp(&b.position.x)
            .then(a.position.y.total_cmp(&b.position.y))
    });
    let mut groups = Vec::new();
    while !left.is_empty() {
        let mut group = vec![left.remove(0)];
        while group.len() < cap && !left.is_empty() {
            let last = group[group.len() - 1].position;
            let next = left
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.position.dist(last).total_cmp(&b.1.position.dist(last)))
                .map_or(0, |(i, _)| i);
            group.push(left.remove(next));
        }
        groups.push(group.iter().map(|d| d.id).collect());
    }
    groups
}

/// Group the layer's loads into branch circuits (the layer is not modified;
/// see [`assign_circuits`]).
///
/// Order and rules: each 220 V outlet gets its own dedicated circuit; counter
/// height receptacles are split across `kitchen_counter_circuits` circuits
/// (more if over the per-circuit limit); remaining receptacles are grouped by
/// proximity, `outlets_per_circuit` per 20 A circuit; lights, fans and
/// detectors `lights_per_circuit` per 15 A circuit. Switches, thermostats,
/// doorbells and panels are not loads.
pub fn circuits(layer: &ElectricalLayer, opts: &CircuitOptions) -> Vec<Circuit> {
    let mut out: Vec<Circuit> = Vec::new();
    let mut number = opts.first_number;
    let mut push = |amps: u32, devices: Vec<Id>, description: String| {
        out.push(Circuit {
            number,
            amps,
            devices,
            description,
        });
        number += 1;
    };

    for d in layer
        .devices
        .iter()
        .filter(|d| d.kind == DeviceKind::Outlet220)
    {
        let name = if d.label.is_empty() {
            format!("#{}", d.id)
        } else {
            d.label.clone()
        };
        push(
            opts.dedicated_amps,
            vec![d.id],
            format!("Dedicated 220V {name}"),
        );
    }

    let is_counter = |d: &&Device| {
        matches!(d.kind, DeviceKind::Gfci | DeviceKind::Outlet110)
            && d.height >= opts.counter_min_height
    };
    let counters: Vec<&Device> = layer.devices.iter().filter(is_counter).collect();
    if !counters.is_empty() {
        let chain = cluster(&counters, counters.len()).remove(0);
        let by_limit = chain.len().div_ceil(opts.outlets_per_circuit.max(1));
        let n = opts.kitchen_counter_circuits.max(by_limit).min(chain.len());
        for (i, chunk) in chain.chunks(chain.len().div_ceil(n)).enumerate() {
            push(
                opts.outlet_amps,
                chunk.to_vec(),
                format!("Kitchen counter GFCI {}", i + 1),
            );
        }
    }

    let general: Vec<&Device> = layer
        .devices
        .iter()
        .filter(|d| d.kind.is_outlet() && d.kind != DeviceKind::Outlet220 && !is_counter(d))
        .collect();
    for (i, ids) in cluster(&general, opts.outlets_per_circuit)
        .into_iter()
        .enumerate()
    {
        push(
            opts.outlet_amps,
            ids,
            format!("General receptacles {}", i + 1),
        );
    }

    let loads: Vec<&Device> = layer
        .devices
        .iter()
        .filter(|d| {
            d.kind.is_light()
                || matches!(
                    d.kind,
                    DeviceKind::CeilingFan | DeviceKind::SmokeDetector | DeviceKind::CoDetector
                )
        })
        .collect();
    for (i, ids) in cluster(&loads, opts.lights_per_circuit)
        .into_iter()
        .enumerate()
    {
        push(opts.light_amps, ids, format!("Lighting {}", i + 1));
    }
    out
}

/// Compute [`circuits`] and write each circuit number into its devices.
pub fn assign_circuits(layer: &mut ElectricalLayer, opts: &CircuitOptions) -> Vec<Circuit> {
    let list = circuits(layer, opts);
    for c in &list {
        for id in &c.devices {
            if let Some(d) = layer.device_mut(*id) {
                d.circuit = Some(c.number);
            }
        }
    }
    list
}

/// Count of each device kind present, as `(kind name, count)` in legend order.
pub fn schedule(layer: &ElectricalLayer) -> Vec<(String, usize)> {
    DeviceKind::all()
        .iter()
        .filter_map(|k| {
            let n = layer
                .devices
                .iter()
                .filter(|d| std::mem::discriminant(&d.kind) == std::mem::discriminant(k))
                .count();
            (n > 0).then(|| (k.name().to_string(), n))
        })
        .collect()
}

/// Every symbol with its legend text (`RopeLight` is listed with length 0).
pub fn legend() -> Vec<(DeviceKind, &'static str)> {
    DeviceKind::all()
        .into_iter()
        .map(|k| (k, k.description()))
        .collect()
}
