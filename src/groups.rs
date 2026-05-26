use serde::{Deserialize, Serialize};

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

pub fn normalize_reference(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize_reference, ComponentGroup, ComponentGroupPlan};

    #[test]
    fn normalizes_reference_designators() {
        assert_eq!(normalize_reference(" u12 "), "U12");
        assert_eq!(normalize_reference("r101"), "R101");
    }

    #[test]
    fn plan_json_round_trips() {
        let plan = ComponentGroupPlan {
            version: 1,
            generated_by: "agent-authored".to_string(),
            groups: vec![ComponentGroup {
                name: "Power input".to_string(),
                kind: "functional-block".to_string(),
                reason: "Agent grouped the connector, fuse, and regulator from board context"
                    .to_string(),
                references: vec!["J1".to_string(), "F1".to_string(), "U3".to_string()],
                item_ids: Vec::new(),
                nets: vec!["VBUS".to_string(), "3V3".to_string()],
            }],
        };
        let json = serde_json::to_string_pretty(&plan).expect("plan should serialize");
        let decoded: ComponentGroupPlan =
            serde_json::from_str(&json).expect("plan should deserialize");
        assert_eq!(decoded, plan);
    }
}
