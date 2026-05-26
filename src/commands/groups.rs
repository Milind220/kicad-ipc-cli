use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{bail, Context};
use kicad_ipc_rs::{CommitAction, EditablePcbItem, GroupItem, KiCadClientBlocking, PcbItem};
use serde::Serialize;

use crate::cli::{ComponentGroupApplyArgs, OutputFormat};
use crate::groups::ComponentGroupPlan;
use crate::model::item_id;
use crate::output;

use super::inspect::{ensure_board_open, read_all_decoded_pcb_items};

pub fn apply(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ComponentGroupApplyArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let plan_text = fs::read_to_string(&args.plan)
        .with_context(|| format!("failed to read group plan `{}`", args.plan.display()))?;
    let plan: ComponentGroupPlan =
        serde_json::from_str(&plan_text).context("failed to parse group plan JSON")?;
    if plan.version != 1 {
        bail!(
            "unsupported component group plan version {}; expected 1",
            plan.version
        );
    }
    if plan.groups.is_empty() {
        bail!("component group plan contains no groups");
    }

    let all_items = read_all_decoded_pcb_items(client)
        .context("failed to read PCB items before applying groups")?;
    let footprints = footprint_id_map(&all_items);
    let existing_groups = existing_group_map(&all_items);
    let known_item_ids = all_items
        .iter()
        .filter_map(item_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let prepared = prepare_groups(&plan, &footprints, &known_item_ids)?;

    let commit = client
        .begin_commit()
        .context("failed to begin KiCad commit")?;
    let result = (|| -> anyhow::Result<ApplyReport> {
        let mut deleted_group_ids = Vec::new();
        if args.replace_existing && !args.keep_existing {
            let names = prepared
                .iter()
                .map(|group| group.name.as_str())
                .collect::<BTreeSet<_>>();
            deleted_group_ids = existing_groups
                .iter()
                .filter(|(name, _)| names.contains(name.as_str()))
                .flat_map(|(_, ids)| ids.iter().cloned())
                .collect::<Vec<_>>();
            if !deleted_group_ids.is_empty() {
                client
                    .delete_items(deleted_group_ids.clone())
                    .context("failed to delete existing groups")?;
            }
        }

        let editable_groups = prepared
            .iter()
            .map(|group| {
                EditablePcbItem::Group(GroupItem::new(group.name.clone(), group.item_ids.clone()))
            })
            .collect::<Vec<_>>();
        let created = client
            .create_editable_items(editable_groups, None)
            .context("failed to create KiCad groups")?;
        let created_groups = created
            .iter()
            .enumerate()
            .map(|(index, item)| CreatedGroupSummary {
                name: prepared[index].name.clone(),
                id: item.id().map(str::to_string),
                item_count: prepared[index].item_ids.len(),
            })
            .collect::<Vec<_>>();

        Ok(ApplyReport {
            commit_id: commit.id.clone(),
            action: "commit".to_string(),
            plan: args.plan.display().to_string(),
            deleted_group_ids,
            created_groups,
            warnings: prepared
                .iter()
                .flat_map(|group| group.warnings.iter().cloned())
                .collect(),
        })
    })();

    match result {
        Ok(report) => {
            client
                .end_commit(
                    commit,
                    CommitAction::Commit,
                    "kicad-ipc-cli component-groups apply".to_string(),
                )
                .context("failed to commit group changes")?;
            output::print(format, &report, || {
                format!(
                    "component groups applied\ncommit: {}\ncreated groups: {}\ndeleted existing groups: {}",
                    report.commit_id,
                    report.created_groups.len(),
                    report.deleted_group_ids.len()
                )
            })
        }
        Err(err) => {
            let _ = client.end_commit(
                commit,
                CommitAction::Drop,
                "kicad-ipc-cli component-groups apply failed".to_string(),
            );
            Err(err)
        }
    }
}

fn footprint_id_map(items: &[PcbItem]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for item in items {
        if let PcbItem::Footprint(footprint) = item {
            if let (Some(reference), Some(id)) = (&footprint.reference, &footprint.id) {
                map.insert(crate::groups::normalize_reference(reference), id.clone());
            }
        }
    }
    map
}

fn existing_group_map(items: &[PcbItem]) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::<String, Vec<String>>::new();
    for item in items {
        if let PcbItem::Group(group) = item {
            if let Some(id) = item_id(item) {
                map.entry(group.name.clone())
                    .or_default()
                    .push(id.to_string());
            }
        }
    }
    map
}

fn prepare_groups(
    plan: &ComponentGroupPlan,
    footprints: &BTreeMap<String, String>,
    known_item_ids: &BTreeSet<String>,
) -> anyhow::Result<Vec<PreparedGroup>> {
    let mut prepared = Vec::new();
    let mut names = BTreeSet::<String>::new();
    for group in &plan.groups {
        let name = group.name.trim();
        if name.is_empty() {
            bail!("group name cannot be empty");
        }
        if !names.insert(name.to_string()) {
            bail!("duplicate group name `{name}` in component group plan");
        }

        let mut ids = BTreeSet::<String>::new();
        for id in group.item_ids.iter().filter(|id| !id.trim().is_empty()) {
            if !known_item_ids.contains(id) {
                bail!(
                    "group `{}` item_id `{}` was not found; explicit item_ids are validated before mutation",
                    group.name,
                    id
                );
            }
            ids.insert(id.clone());
        }
        let mut warnings = Vec::new();
        for reference in &group.references {
            let normalized = crate::groups::normalize_reference(reference);
            if let Some(id) = footprints.get(&normalized) {
                ids.insert(id.clone());
            } else {
                warnings.push(format!(
                    "group `{}` references `{}` but no matching footprint was found",
                    group.name, reference
                ));
            }
        }
        if ids.is_empty() {
            bail!(
                "group `{}` has no item_ids and none of its references resolved to footprint IDs",
                group.name
            );
        }
        prepared.push(PreparedGroup {
            name: group.name.clone(),
            item_ids: ids.into_iter().collect(),
            warnings,
        });
    }
    Ok(prepared)
}

#[derive(Debug)]
struct PreparedGroup {
    name: String,
    item_ids: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ApplyReport {
    commit_id: String,
    action: String,
    plan: String,
    deleted_group_ids: Vec<String>,
    created_groups: Vec<CreatedGroupSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreatedGroupSummary {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    item_count: usize,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::groups::{ComponentGroup, ComponentGroupPlan};

    use super::prepare_groups;

    #[test]
    fn prepare_groups_rejects_duplicate_names() {
        let plan = ComponentGroupPlan {
            version: 1,
            generated_by: "agent-authored".to_string(),
            groups: vec![group("Power", &["U1"], &[]), group("Power", &["U2"], &[])],
        };
        let footprints = BTreeMap::from([
            ("U1".to_string(), "id-u1".to_string()),
            ("U2".to_string(), "id-u2".to_string()),
        ]);

        let known_item_ids = BTreeSet::from(["id-u1".to_string(), "id-u2".to_string()]);

        let err = prepare_groups(&plan, &footprints, &known_item_ids)
            .expect_err("duplicate names should fail");

        assert!(err.to_string().contains("duplicate group name"));
    }

    #[test]
    fn prepare_groups_rejects_unresolved_explicit_item_ids() {
        let plan = ComponentGroupPlan {
            version: 1,
            generated_by: "agent-authored".to_string(),
            groups: vec![group("Power", &[], &["missing-id"])],
        };

        let err = prepare_groups(&plan, &BTreeMap::new(), &BTreeSet::new())
            .expect_err("unresolved explicit item IDs should fail");

        assert!(err
            .to_string()
            .contains("item_id `missing-id` was not found"));
    }

    fn group(name: &str, references: &[&str], item_ids: &[&str]) -> ComponentGroup {
        ComponentGroup {
            name: name.to_string(),
            kind: "metadata-only".to_string(),
            reason: "test".to_string(),
            references: references.iter().map(|value| value.to_string()).collect(),
            item_ids: item_ids.iter().map(|value| value.to_string()).collect(),
            nets: vec!["GND".to_string()],
        }
    }
}
