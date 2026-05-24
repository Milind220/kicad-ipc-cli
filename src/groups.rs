use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintFact {
    pub id: Option<String>,
    pub reference: String,
    pub value: Option<String>,
    pub nets: Vec<String>,
    pub pad_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComponentGroupPlan {
    pub version: u32,
    pub generated_by: String,
    pub groups: Vec<ComponentGroup>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComponentGroup {
    pub name: String,
    pub kind: String,
    pub reason: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub item_ids: Vec<String>,
    #[serde(default)]
    pub nets: Vec<String>,
}

pub fn suggest_component_groups(facts: &[FootprintFact]) -> ComponentGroupPlan {
    let mut groups = BTreeMap::<String, GroupBuilder>::new();

    for fact in facts {
        let tags = classify(fact);
        for tag in tags {
            let builder = groups
                .entry(tag.name.to_string())
                .or_insert_with(|| GroupBuilder::new(tag.name, tag.kind, tag.reason));
            builder.push(fact);
        }
    }

    let groups = groups
        .into_values()
        .filter_map(GroupBuilder::finish)
        .collect();

    ComponentGroupPlan {
        version: 1,
        generated_by: "kicad-ipc-cli component-groups suggest".to_string(),
        groups,
    }
}

pub fn normalize_reference(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

pub fn reference_prefix(value: &str) -> String {
    normalize_reference(value)
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect()
}

fn classify(fact: &FootprintFact) -> Vec<GroupTag> {
    let reference = normalize_reference(&fact.reference);
    let prefix = reference_prefix(&reference);
    let value = fact.value.as_deref().unwrap_or("").to_ascii_lowercase();
    let net_text = fact.nets.join(" ").to_ascii_lowercase();
    let mut tags = Vec::new();

    if prefix == "J" || prefix == "P" || value.contains("connector") || value.contains("usb") {
        tags.push(GroupTag::new(
            "ipc: connectors",
            "connectors",
            "connector reference prefix or connector-like value",
        ));
    }

    if prefix == "U"
        && (value.contains("mcu")
            || value.contains("microcontroller")
            || value.contains("stm32")
            || value.contains("rp2040")
            || value.contains("esp32")
            || value.contains("nrf52"))
    {
        tags.push(GroupTag::new("ipc: mcu", "mcu", "MCU-like footprint value"));
    }

    if value.contains("can") || net_text.contains("canh") || net_text.contains("canl") {
        tags.push(GroupTag::new(
            "ipc: can-interface",
            "can-transceiver",
            "CAN-related value or CANH/CANL nets",
        ));
    }

    if value.contains("regulator")
        || value.contains("ldo")
        || value.contains("buck")
        || value.contains("boost")
        || value.contains("dc-dc")
        || value.contains("dcdc")
    {
        tags.push(GroupTag::new(
            "ipc: power-regulation",
            "power-rail-regulator",
            "power-regulator-like footprint value",
        ));
    }

    if prefix == "C" && has_ground_net(&fact.nets) && fact.nets.iter().any(|net| is_power_net(net))
    {
        tags.push(GroupTag::new(
            "ipc: decoupling-capacitors",
            "decoupling",
            "capacitor tied between ground and a power-like rail",
        ));
    }

    if tags.is_empty() && matches!(prefix.as_str(), "R" | "C" | "L" | "D") {
        tags.push(GroupTag::new(
            "ipc: passives",
            "passives",
            "passive reference prefix",
        ));
    }

    tags
}

fn has_ground_net(nets: &[String]) -> bool {
    nets.iter().any(|net| {
        matches!(
            net.trim().to_ascii_uppercase().as_str(),
            "GND" | "GNDA" | "DGND" | "AGND"
        )
    })
}

pub fn is_power_net(net: &str) -> bool {
    let net = net.trim().to_ascii_uppercase();
    if net.is_empty() {
        return false;
    }
    net == "VCC"
        || net == "VDD"
        || net == "VSS"
        || net == "VBUS"
        || net == "VIN"
        || net == "VOUT"
        || net.starts_with("+")
        || net.starts_with("VCC")
        || net.starts_with("VDD")
        || net.ends_with("V")
        || net.contains("3V3")
        || net.contains("5V")
}

#[derive(Clone, Copy)]
struct GroupTag {
    name: &'static str,
    kind: &'static str,
    reason: &'static str,
}

impl GroupTag {
    fn new(name: &'static str, kind: &'static str, reason: &'static str) -> Self {
        Self { name, kind, reason }
    }
}

struct GroupBuilder {
    name: String,
    kind: String,
    reason: String,
    references: BTreeSet<String>,
    item_ids: BTreeSet<String>,
    nets: BTreeSet<String>,
}

impl GroupBuilder {
    fn new(name: &str, kind: &str, reason: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: kind.to_string(),
            reason: reason.to_string(),
            references: BTreeSet::new(),
            item_ids: BTreeSet::new(),
            nets: BTreeSet::new(),
        }
    }

    fn push(&mut self, fact: &FootprintFact) {
        self.references.insert(normalize_reference(&fact.reference));
        if let Some(id) = &fact.id {
            self.item_ids.insert(id.clone());
        }
        for net in &fact.nets {
            if !net.trim().is_empty() {
                self.nets.insert(net.clone());
            }
        }
    }

    fn finish(self) -> Option<ComponentGroup> {
        if self.references.is_empty() && self.item_ids.is_empty() {
            return None;
        }
        Some(ComponentGroup {
            name: self.name,
            kind: self.kind,
            reason: self.reason,
            references: self.references.into_iter().collect(),
            item_ids: self.item_ids.into_iter().collect(),
            nets: self.nets.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_power_net, normalize_reference, reference_prefix, suggest_component_groups,
        ComponentGroupPlan, FootprintFact,
    };

    #[test]
    fn normalizes_reference_designators() {
        assert_eq!(normalize_reference(" u12 "), "U12");
        assert_eq!(reference_prefix("R101"), "R");
        assert_eq!(reference_prefix("TP3"), "TP");
    }

    #[test]
    fn detects_power_nets() {
        assert!(is_power_net("+3V3"));
        assert!(is_power_net("VDD"));
        assert!(is_power_net("USB_5V"));
        assert!(!is_power_net("CANH"));
    }

    #[test]
    fn suggests_expected_agent_groups() {
        let facts = vec![
            fact("fp-u1", "U1", "STM32G0 MCU", ["VDD", "GND", "SWDIO"]),
            fact(
                "fp-u2",
                "U2",
                "TCAN1042 CAN transceiver",
                ["CANH", "CANL", "VCC", "GND"],
            ),
            fact("fp-j1", "J1", "USB-C connector", ["VBUS", "D+", "D-"]),
            fact("fp-c1", "C1", "100nF", ["VDD", "GND"]),
            fact("fp-u3", "U3", "3.3V LDO regulator", ["VIN", "VOUT", "GND"]),
            fact("fp-r1", "R1", "10k", ["RESET", "VDD"]),
        ];

        let plan = suggest_component_groups(&facts);
        let names: Vec<_> = plan
            .groups
            .iter()
            .map(|group| group.name.as_str())
            .collect();
        assert!(names.contains(&"ipc: mcu"));
        assert!(names.contains(&"ipc: can-interface"));
        assert!(names.contains(&"ipc: connectors"));
        assert!(names.contains(&"ipc: decoupling-capacitors"));
        assert!(names.contains(&"ipc: power-regulation"));
        assert!(names.contains(&"ipc: passives"));
    }

    #[test]
    fn plan_json_round_trips() {
        let plan = suggest_component_groups(&[fact("fp-u1", "U1", "STM32 MCU", ["VDD", "GND"])]);
        let json = serde_json::to_string_pretty(&plan).expect("plan should serialize");
        let decoded: ComponentGroupPlan =
            serde_json::from_str(&json).expect("plan should deserialize");
        assert_eq!(decoded, plan);
    }

    fn fact<const N: usize>(
        id: &str,
        reference: &str,
        value: &str,
        nets: [&str; N],
    ) -> FootprintFact {
        FootprintFact {
            id: Some(id.to_string()),
            reference: reference.to_string(),
            value: Some(value.to_string()),
            nets: nets.into_iter().map(str::to_string).collect(),
            pad_count: nets.len(),
        }
    }
}
