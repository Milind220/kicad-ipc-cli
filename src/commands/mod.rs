mod api;
mod groups;
mod inspect;
mod mutate;

use anyhow::bail;

use crate::cli::{
    ApiBoardCommand, ApiCommand, ApiCommonCommand, ApiDocumentCommand, ApiItemsCommand,
    ApiRawCommand, ApiSelectionCommand, Cli, Command, ComponentGroupsCommand, SelectCommand,
    ViewCommand, ZonesCommand,
};

pub fn dispatch(cli: Cli) -> anyhow::Result<()> {
    if let Command::Api(args) = &cli.command {
        if matches!(args.command, ApiCommand::List) {
            return api::list(cli.format);
        }
    }

    let client = crate::client::connect(&cli)?;

    match &cli.command {
        Command::Doctor => inspect::doctor(&client, cli.format),
        Command::BoardSummary => inspect::board_summary(&client, cli.format),
        Command::Inventory => inspect::inventory(&client, cli.format),
        Command::Selection(args) => inspect::selection(&client, cli.format, args),
        Command::NetReport(args) => inspect::net_report(&client, cli.format, args),
        Command::ComponentGroups(args) => match &args.command {
            ComponentGroupsCommand::Suggest(args) => groups::suggest(&client, cli.format, args),
            ComponentGroupsCommand::Apply(args) => {
                require_yes(cli.yes, "component-groups apply")?;
                groups::apply(&client, cli.format, args)
            }
        },
        Command::Select(args) => match &args.command {
            SelectCommand::Add(args) => mutate::select_add(&client, cli.format, &args.item_ids),
            SelectCommand::Remove(args) => {
                mutate::select_remove(&client, cli.format, &args.item_ids)
            }
            SelectCommand::Clear => mutate::select_clear(&client, cli.format),
            SelectCommand::ByRef(args) => mutate::select_by_ref(&client, cli.format, args),
            SelectCommand::ByNet(args) => mutate::select_by_net(&client, cli.format, args),
        },
        Command::View(args) => match &args.command {
            ViewCommand::ActiveLayer { layer_id } => {
                mutate::view_active_layer(&client, cli.format, *layer_id)
            }
            ViewCommand::VisibleLayers { layer_ids } => {
                mutate::view_visible_layers(&client, cli.format, layer_ids)
            }
            ViewCommand::Preset(args) => mutate::view_preset(&client, cli.format, args),
        },
        Command::DrcMarker(args) => {
            require_yes(cli.yes, "drc-marker")?;
            mutate::drc_marker(&client, cli.format, args)
        }
        Command::Zones(args) => match &args.command {
            ZonesCommand::Refill(args) => {
                require_yes(cli.yes, "zones refill")?;
                mutate::zones_refill(&client, cli.format, args)
            }
        },
        Command::Snapshot(args) => inspect::snapshot(&client, cli.format, args),
        Command::TextShapes(args) => inspect::text_shapes(&client, cli.format, args),
        Command::Api(args) => match &args.command {
            ApiCommand::List => unreachable!("api list returns before connecting"),
            ApiCommand::Common(args) => match &args.command {
                ApiCommonCommand::Ping => api::ping(&client, cli.format),
                ApiCommonCommand::Version => api::version(&client, cli.format),
                ApiCommonCommand::ProjectPath => api::project_path(&client, cli.format),
                ApiCommonCommand::HasOpenBoard => api::has_open_board(&client, cli.format),
                ApiCommonCommand::OpenDocuments(args) => {
                    api::open_documents(&client, cli.format, args)
                }
                ApiCommonCommand::KicadBinaryPath(args) => {
                    api::kicad_binary_path(&client, cli.format, args)
                }
                ApiCommonCommand::PluginSettingsPath(args) => {
                    api::plugin_settings_path(&client, cli.format, args)
                }
                ApiCommonCommand::Refresh(args) => api::refresh(&client, cli.format, args),
                ApiCommonCommand::RunAction(args) => api::run_action(&client, cli.format, args),
                ApiCommonCommand::NetClasses => api::net_classes(&client, cli.format),
                ApiCommonCommand::SetNetClasses(args) => {
                    require_yes(cli.yes, "api common set-net-classes")?;
                    api::set_net_classes(&client, cli.format, args)
                }
                ApiCommonCommand::TextVariablesGet => api::text_variables_get(&client, cli.format),
                ApiCommonCommand::TextVariablesSet(args) => {
                    require_yes(cli.yes, "api common text-variables-set")?;
                    api::text_variables_set(&client, cli.format, args)
                }
                ApiCommonCommand::ExpandTextVariables(args) => {
                    api::expand_text_variables(&client, cli.format, args)
                }
                ApiCommonCommand::TextExtents(args) => api::text_extents(&client, cli.format, args),
                ApiCommonCommand::TextAsShapes(args) => {
                    api::text_as_shapes(&client, cli.format, args)
                }
            },
            ApiCommand::Board(args) => match &args.command {
                ApiBoardCommand::Nets => api::board_nets(&client, cli.format),
                ApiBoardCommand::EnabledLayers => api::enabled_layers(&client, cli.format),
                ApiBoardCommand::SetEnabledLayers(args) => {
                    require_yes(cli.yes, "api board set-enabled-layers")?;
                    api::set_enabled_layers(&client, cli.format, args)
                }
                ApiBoardCommand::ActiveLayer => api::active_layer(&client, cli.format),
                ApiBoardCommand::SetActiveLayer(args) => {
                    api::set_active_layer(&client, cli.format, args)
                }
                ApiBoardCommand::VisibleLayers => api::visible_layers(&client, cli.format),
                ApiBoardCommand::SetVisibleLayers(args) => {
                    api::set_visible_layers(&client, cli.format, args)
                }
                ApiBoardCommand::LayerName(args) => api::layer_name(&client, cli.format, args),
                ApiBoardCommand::Origin(args) => api::origin(&client, cli.format, args),
                ApiBoardCommand::SetOrigin(args) => {
                    require_yes(cli.yes, "api board set-origin")?;
                    api::set_origin(&client, cli.format, args)
                }
                ApiBoardCommand::Stackup => api::stackup(&client, cli.format),
                ApiBoardCommand::UpdateStackup(args) => {
                    require_yes(cli.yes, "api board update-stackup")?;
                    api::update_stackup(&client, cli.format, args)
                }
                ApiBoardCommand::GraphicsDefaults => api::graphics_defaults(&client, cli.format),
                ApiBoardCommand::Appearance => api::appearance(&client, cli.format),
                ApiBoardCommand::SetAppearance(args) => {
                    api::set_appearance(&client, cli.format, args)
                }
                ApiBoardCommand::InteractiveMoveItems(args) => {
                    api::interactive_move_items(&client, cli.format, args)
                }
                ApiBoardCommand::Items(args) => api::board_items(&client, cli.format, args),
                ApiBoardCommand::ItemsByNet(args) => api::items_by_net(&client, cli.format, args),
                ApiBoardCommand::ItemsByNetClass(args) => {
                    api::items_by_net_class(&client, cli.format, args)
                }
                ApiBoardCommand::ConnectedItems(args) => {
                    api::connected_items(&client, cli.format, args)
                }
                ApiBoardCommand::NetclassForNets(args) => {
                    api::netclass_for_nets(&client, cli.format, args)
                }
                ApiBoardCommand::PadShapeAsPolygon(args) => {
                    api::pad_shape_as_polygon(&client, cli.format, args)
                }
                ApiBoardCommand::PadstackPresence(args) => {
                    api::padstack_presence(&client, cli.format, args)
                }
                ApiBoardCommand::InjectDrcError(args) => {
                    require_yes(cli.yes, "api board inject-drc-error")?;
                    mutate::drc_marker(&client, cli.format, args)
                }
                ApiBoardCommand::BoundingBoxes(args) => {
                    api::bounding_boxes(&client, cli.format, args)
                }
                ApiBoardCommand::HitTest(args) => api::hit_test(&client, cli.format, args),
                ApiBoardCommand::RefillZones(args) => {
                    require_yes(cli.yes, "api board refill-zones")?;
                    api::refill_zones(&client, cli.format, args)
                }
            },
            ApiCommand::Selection(args) => match &args.command {
                ApiSelectionCommand::Summary(args) => {
                    api::selection_summary(&client, cli.format, args)
                }
                ApiSelectionCommand::Get(args) => api::selection_get(&client, cli.format, args),
                ApiSelectionCommand::Details(args) => {
                    api::selection_details(&client, cli.format, args)
                }
                ApiSelectionCommand::Add(args) => api::selection_add(&client, cli.format, args),
                ApiSelectionCommand::Remove(args) => {
                    api::selection_remove(&client, cli.format, args)
                }
                ApiSelectionCommand::Clear => api::selection_clear(&client, cli.format),
            },
            ApiCommand::Items(args) => match &args.command {
                ApiItemsCommand::BeginCommit => api::begin_commit(&client, cli.format),
                ApiItemsCommand::EndCommit(args) => api::end_commit(&client, cli.format, args),
                ApiItemsCommand::CreateBoardText(args) => {
                    require_yes(cli.yes, "api items create-board-text")?;
                    api::create_board_text(&client, cli.format, args)
                }
                ApiItemsCommand::CreateBoardTexts(args) => {
                    require_yes(cli.yes, "api items create-board-texts")?;
                    api::create_board_texts(&client, cli.format, args)
                }
                ApiItemsCommand::CreateRaw(args) => {
                    require_yes(cli.yes, "api items create-raw")?;
                    api::create_raw(&client, cli.format, args)
                }
                ApiItemsCommand::UpdateRaw(args) => {
                    require_yes(cli.yes, "api items update-raw")?;
                    api::update_raw(&client, cli.format, args)
                }
                ApiItemsCommand::ParseCreate(args) => {
                    require_yes(cli.yes, "api items parse-create")?;
                    api::parse_create(&client, cli.format, args)
                }
                ApiItemsCommand::Delete(args) => {
                    require_yes(cli.yes, "api items delete")?;
                    api::delete_items(&client, cli.format, args)
                }
                ApiItemsCommand::GetById(args) => api::get_items_by_id(&client, cli.format, args),
                ApiItemsCommand::GetEditableById(args) => {
                    api::get_editable_items_by_id(&client, cli.format, args)
                }
            },
            ApiCommand::Raw(args) => match &args.command {
                ApiRawCommand::Send(args) => api::raw_send(&client, cli.format, args),
            },
            ApiCommand::Document(args) => match &args.command {
                ApiDocumentCommand::TitleBlock => api::title_block(&client, cli.format),
                ApiDocumentCommand::SetTitleBlock(args) => {
                    require_yes(cli.yes, "api document set-title-block")?;
                    api::set_title_block(&client, cli.format, args)
                }
                ApiDocumentCommand::Save => {
                    require_yes(cli.yes, "api document save")?;
                    api::save_document(&client, cli.format)
                }
                ApiDocumentCommand::SaveCopy(args) => api::save_copy(&client, cli.format, args),
                ApiDocumentCommand::Revert => {
                    require_yes(cli.yes, "api document revert")?;
                    api::revert_document(&client, cli.format)
                }
                ApiDocumentCommand::BoardString => api::board_string(&client, cli.format),
                ApiDocumentCommand::SelectionString => api::selection_string(&client, cli.format),
            },
        },
    }
}

fn require_yes(yes: bool, command: &str) -> anyhow::Result<()> {
    if yes {
        Ok(())
    } else {
        bail!("`{command}` modifies the board; rerun with --yes to confirm")
    }
}

fn all_type_codes() -> Vec<i32> {
    kicad_ipc_rs::KiCadClient::pcb_object_type_codes()
        .iter()
        .map(|entry| entry.code)
        .collect()
}
