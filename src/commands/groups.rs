use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{bail, Context};
use kicad_ipc_rs::{CommitAction, EditablePcbItem, GroupItem, KiCadClientBlocking, PcbItem};
use serde::Serialize;

use crate::cli::{ComponentGroupApplyArgs, ComponentGroupSuggestArgs, OutputFormat};
use crate::groups::{suggest_component_groups, ComponentGroupPlan, FootprintFact};
use crate::model::item_id;
use crate::output;

use super::inspect::{ensure_board_open, flatten_items};

pub fn suggest(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ComponentGroupSuggestArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let facts = collect_footprint_facts(client)?;
    let plan = suggest_component_groups(&facts);

    if let Some(path) = &args.output {
        let json = serde_json::to_string_pretty(&plan).context("failed to encode group plan")?;
        fs::write(path, json)
            .with_context(|| format!("failed to write group plan to `{}`", path.display()))?;
    }

    output::print(format, &plan, || {
        serde_json::to_string_pretty(&plan).expect("group plan should serialize")
    })
}

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

    let all_items = flatten_items(
        &client
            .get_all_pcb_items()
            .context("failed to read PCB items before applying groups")?,
    );
    let footprints = footprint_id_map(&all_items);
    let existing_groups = existing_group_map(&all_items);
    let prepared = prepare_groups(&plan, &footprints)?;

    let commit = client
        .begin_commit()
        .context("failed to begin KiCad commit")?;
    let result = (|| -> anyhow::Result<ApplyReport> {
        let mut deleted_group_ids = Vec::new();
        if args.replace_existing {
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

pub fn collect_footprint_facts(client: &KiCadClientBlocking) -> anyhow::Result<Vec<FootprintFact>> {
    let all_items = flatten_items(
        &client
            .get_all_pcb_items()
            .context("failed to read PCB items for group suggestions")?,
    );
    let pad_rows = client
        .get_pad_netlist()
        .context("failed to read pad netlist for group suggestions")?;
    let mut nets_by_footprint_id = BTreeMap::<String, BTreeSet<String>>::new();
    let mut nets_by_reference = BTreeMap::<String, BTreeSet<String>>::new();

    for row in &pad_rows {
        if let Some(net_name) = &row.net_name {
            if let Some(id) = &row.footprint_id {
                nets_by_footprint_id
                    .entry(id.clone())
                    .or_default()
                    .insert(net_name.clone());
            }
            if let Some(reference) = &row.footprint_reference {
                nets_by_reference
                    .entry(reference.clone())
                    .or_default()
                    .insert(net_name.clone());
            }
        }
    }

    let facts = all_items
        .iter()
        .filter_map(|item| match item {
            PcbItem::Footprint(footprint) => {
                let reference = footprint.reference.clone()?;
                let mut nets = BTreeSet::<String>::new();
                if let Some(id) = &footprint.id {
                    if let Some(rows) = nets_by_footprint_id.get(id) {
                        nets.extend(rows.iter().cloned());
                    }
                }
                if let Some(rows) = nets_by_reference.get(&reference) {
                    nets.extend(rows.iter().cloned());
                }
                Some(FootprintFact {
                    id: footprint.id.clone(),
                    reference,
                    value: footprint.value.clone(),
                    nets: nets.into_iter().collect(),
                    pad_count: footprint.pad_count,
                })
            }
            _ => None,
        })
        .collect();

    Ok(facts)
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
) -> anyhow::Result<Vec<PreparedGroup>> {
    let mut prepared = Vec::new();
    for group in &plan.groups {
        if group.name.trim().is_empty() {
            bail!("group name cannot be empty");
        }

        let mut ids = BTreeSet::<String>::new();
        ids.extend(
            group
                .item_ids
                .iter()
                .filter(|id| !id.trim().is_empty())
                .cloned(),
        );
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
