use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use kicad_ipc_rs::{
    BoardFlipMode, BoardLayerInfo, BoardOriginKind, CommitAction, DocumentType, DrcSeverity,
    EditorFrameType, InactiveLayerDisplayMode, MapMergeMode, NetColorDisplayMode,
    PcbObjectTypeCode, RatsnestDisplayMode,
};

use crate::units::{parse_distance_nm, parse_point_nm, PointNm};

#[derive(Debug, Parser)]
#[command(version, about = "Agent-oriented KiCad PCB Editor IPC CLI")]
pub struct Cli {
    /// Override KiCad IPC socket path/URI.
    #[arg(long, global = true)]
    pub socket: Option<String>,

    /// Authentication token for KiCad IPC.
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// IPC request timeout in milliseconds.
    #[arg(long, global = true, default_value_t = 3000)]
    pub timeout_ms: u64,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,

    /// Confirm commands that modify a board.
    #[arg(long, global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ping KiCad and report version, project, board, and open documents.
    Doctor,
    /// Summarize board layers, origins, stackup, nets, and item counts.
    BoardSummary,
    /// Inventory footprints, pads, nets, tracks, vias, zones, and groups.
    Inventory(InventoryArgs),
    /// Summarize current selection.
    Selection(SelectionArgs),
    /// Report net classes and net-specific pads/items.
    NetReport(NetReportArgs),
    /// Apply agent-authored named PCB component groups.
    ComponentGroups(ComponentGroupsArgs),
    /// Change the PCB editor selection.
    Select(SelectArgs),
    /// Show or change visual editor state.
    View(ViewArgs),
    /// Inject a DRC marker.
    DrcMarker(DrcMarkerArgs),
    /// Zone operations.
    Zones(ZonesArgs),
    /// Save the board or selection as KiCad s-expression text.
    Snapshot(SnapshotArgs),
    /// Preview text extents and shape conversion metadata.
    TextShapes(TextShapesArgs),
    /// Direct access to kicad-ipc-rs binding operations.
    Api(ApiArgs),
}

#[derive(Debug, Args)]
pub struct SelectionArgs {
    /// Include decoded selection details.
    #[arg(long)]
    pub details: bool,

    /// Limit verbose item arrays while preserving counts.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct InventoryArgs {
    /// Limit verbose item arrays while preserving counts.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct NetReportArgs {
    /// Restrict the report to these net names or numeric net codes.
    #[arg(long = "net")]
    pub nets: Vec<String>,

    /// Include copper-connected items for net items.
    #[arg(long)]
    pub connected: bool,

    /// Limit verbose item arrays while preserving counts.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct ComponentGroupsArgs {
    #[command(subcommand)]
    pub command: ComponentGroupsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ComponentGroupsCommand {
    /// Create or refresh KiCad PCB groups from an agent-authored JSON group plan.
    Apply(ComponentGroupApplyArgs),
}

#[derive(Debug, Args)]
pub struct ComponentGroupApplyArgs {
    /// Agent-authored JSON group plan.
    #[arg(long)]
    pub plan: PathBuf,

    /// Delete existing KiCad groups with the same names before creating new ones.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub replace_existing: bool,

    /// Keep existing KiCad groups with matching names instead of replacing them.
    #[arg(long, conflicts_with = "replace_existing")]
    pub keep_existing: bool,
}

#[derive(Debug, Args)]
pub struct SelectArgs {
    #[command(subcommand)]
    pub command: SelectCommand,
}

#[derive(Debug, Subcommand)]
pub enum SelectCommand {
    /// Select item IDs, defaulting to replacement.
    ById(SelectByIdArgs),
    /// Add item IDs to the current selection.
    Add(ItemIdArgs),
    /// Remove item IDs from the current selection.
    Remove(ItemIdArgs),
    /// Clear the current selection.
    Clear,
    /// Select footprints by reference designator.
    ByRef(SelectByRefArgs),
    /// Select items by net name or code.
    ByNet(SelectByNetArgs),
}

#[derive(Debug, Args)]
pub struct ItemIdArgs {
    /// KiCad item IDs.
    #[arg(required = true)]
    pub item_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SelectByIdArgs {
    /// KiCad item IDs.
    #[arg(required = true)]
    pub item_ids: Vec<String>,

    /// How to combine IDs with the current selection.
    #[arg(long, value_enum, default_value_t = SelectionMode::Replace)]
    pub mode: SelectionMode,
}

#[derive(Debug, Args)]
pub struct SelectByRefArgs {
    /// Reference designators such as U1, C12, or J3.
    #[arg(required = true)]
    pub refs: Vec<String>,

    /// How to combine matches with the current selection.
    #[arg(long, value_enum, default_value_t = SelectionMode::Replace)]
    pub mode: SelectionMode,
}

#[derive(Debug, Args)]
pub struct SelectByNetArgs {
    /// Net names or numeric net codes.
    #[arg(required = true)]
    pub nets: Vec<String>,

    /// How to combine matches with the current selection.
    #[arg(long, value_enum, default_value_t = SelectionMode::Replace)]
    pub mode: SelectionMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum SelectionMode {
    /// Clear then add after preflighting IDs where possible; KiCad does not provide atomic replace.
    Replace,
    Add,
    Remove,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    #[command(subcommand)]
    pub command: ViewCommand,
}

#[derive(Debug, Subcommand)]
pub enum ViewCommand {
    /// Show or set the active layer by layer name or id.
    ActiveLayer {
        /// Layer name or id to activate. Omit to print the current active layer.
        #[arg(value_parser = parse_layer_id)]
        layer_id: Option<i32>,
    },
    /// Show or set visible layers by layer names or ids.
    VisibleLayers {
        /// Layer names or ids to make visible. Omit to print visible layers.
        #[arg(value_parser = parse_layer_id)]
        layer_ids: Vec<i32>,
    },
    /// Apply a compact appearance preset.
    Preset(ViewPresetArgs),
}

#[derive(Debug, Args)]
pub struct ViewPresetArgs {
    #[arg(value_enum)]
    pub preset: ViewPreset,

    /// Required by focus-net; it also preflights and replaces the live selection non-atomically.
    #[arg(long)]
    pub net: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ViewPreset {
    AllLayers,
    FocusNet,
    Ratsnest,
}

#[derive(Debug, Args)]
pub struct DrcMarkerArgs {
    /// Marker severity.
    #[arg(long, value_enum, default_value_t = DrcSeverityArg::Warning)]
    pub severity: DrcSeverityArg,

    /// Marker message.
    #[arg(long, default_value = "IPC marker")]
    pub message: String,

    /// Marker position, e.g. `10mm,20mm` or `1000000nm,2500000nm`.
    #[arg(long, value_parser = parse_point_nm)]
    pub at: Option<PointNm>,

    /// Attach marker to these item IDs.
    #[arg(long = "item-id")]
    pub item_ids: Vec<String>,

    /// Attach marker to the current selection.
    #[arg(long)]
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DrcSeverityArg {
    Warning,
    Error,
    Exclusion,
    Ignore,
    Info,
    Action,
    Debug,
    Undefined,
}

impl From<DrcSeverityArg> for DrcSeverity {
    fn from(value: DrcSeverityArg) -> Self {
        match value {
            DrcSeverityArg::Warning => Self::Warning,
            DrcSeverityArg::Error => Self::Error,
            DrcSeverityArg::Exclusion => Self::Exclusion,
            DrcSeverityArg::Ignore => Self::Ignore,
            DrcSeverityArg::Info => Self::Info,
            DrcSeverityArg::Action => Self::Action,
            DrcSeverityArg::Debug => Self::Debug,
            DrcSeverityArg::Undefined => Self::Undefined,
        }
    }
}

#[derive(Debug, Args)]
pub struct ZonesArgs {
    #[command(subcommand)]
    pub command: ZonesCommand,
}

#[derive(Debug, Subcommand)]
pub enum ZonesCommand {
    /// Refill all zones, named zone IDs, or selected zones.
    Refill(ZonesRefillArgs),
}

#[derive(Debug, Args)]
pub struct ZonesRefillArgs {
    /// Refill these zone IDs. Omit with no other flags to refill all zones.
    #[arg(long = "zone-id")]
    pub zone_ids: Vec<String>,

    /// Refill zones in the current selection.
    #[arg(long)]
    pub selected: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    /// Snapshot scope.
    #[arg(long, value_enum, default_value_t = SnapshotScope::Board)]
    pub scope: SnapshotScope,

    /// Output `.kicad_pcb` or s-expression file.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum SnapshotScope {
    Board,
    Selection,
}

#[derive(Debug, Args)]
pub struct TextShapesArgs {
    /// Text to measure and convert to shape metadata.
    pub text: String,
}

#[derive(Debug, Args)]
pub struct ApiArgs {
    #[command(subcommand)]
    pub command: ApiCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    /// List binding operations exposed by this binary.
    List,
    /// Common KiCad/editor operations.
    Common(ApiCommonArgs),
    /// Board, layer, net, geometry, and PCB item operations.
    Board(ApiBoardArgs),
    /// Selection operations.
    Selection(ApiSelectionArgs),
    /// Item mutation and commit operations.
    Items(ApiItemsArgs),
    /// Raw protobuf command escape hatch.
    Raw(ApiRawArgs),
    /// Document save/revert/title-block operations.
    Document(ApiDocumentArgs),
}

#[derive(Debug, Args)]
pub struct ApiCommonArgs {
    #[command(subcommand)]
    pub command: ApiCommonCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiCommonCommand {
    Ping,
    Version,
    ProjectPath,
    HasOpenBoard,
    OpenDocuments(DocumentTypeArgs),
    KicadBinaryPath(BinaryPathArgs),
    PluginSettingsPath(PluginSettingsArgs),
    Refresh(RefreshArgs),
    RunAction(RunActionArgs),
    NetClasses,
    SetNetClasses(NetClassesSetArgs),
    TextVariablesGet,
    TextVariablesSet(TextVariablesSetArgs),
    ExpandTextVariables(ExpandTextVariablesArgs),
    TextExtents(TextValueArgs),
    TextAsShapes(TextValueArgs),
}

#[derive(Debug, Args)]
pub struct ApiBoardArgs {
    #[command(subcommand)]
    pub command: ApiBoardCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiBoardCommand {
    Nets,
    EnabledLayers,
    SetEnabledLayers(SetEnabledLayersArgs),
    ActiveLayer,
    SetActiveLayer(LayerIdArgs),
    VisibleLayers,
    SetVisibleLayers(LayerIdsArgs),
    LayerName(LayerIdArgs),
    Origin(BoardOriginArgs),
    SetOrigin(SetBoardOriginArgs),
    Stackup,
    UpdateStackup(StackupUpdateArgs),
    GraphicsDefaults,
    Appearance,
    SetAppearance(SetAppearanceArgs),
    InteractiveMoveItems(ItemIdsArgs),
    Items(ApiBoardItemsArgs),
    ItemsByNet(ItemsByNetArgs),
    ItemsByNetClass(ItemsByNetClassArgs),
    ConnectedItems(ConnectedItemsArgs),
    NetclassForNets(NetNamesArgs),
    PadShapeAsPolygon(PadShapeAsPolygonArgs),
    PadstackPresence(PadstackPresenceArgs),
    InjectDrcError(DrcMarkerArgs),
    BoundingBoxes(BoundingBoxesArgs),
    HitTest(HitTestArgs),
    RefillZones(ZoneIdsArgs),
}

#[derive(Debug, Args)]
pub struct ApiSelectionArgs {
    #[command(subcommand)]
    pub command: ApiSelectionCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiSelectionCommand {
    Summary(TypeCodesArgs),
    Get(TypeCodesArgs),
    Details(TypeCodesArgs),
    Add(ItemIdsArgs),
    Remove(ItemIdsArgs),
    Clear,
}

#[derive(Debug, Args)]
pub struct ApiItemsArgs {
    #[command(subcommand)]
    pub command: ApiItemsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiItemsCommand {
    BeginCommit,
    EndCommit(EndCommitArgs),
    CreateBoardText(CreateBoardTextArgs),
    CreateBoardTexts(CreateBoardTextsArgs),
    CreateRaw(RawItemsArgs),
    UpdateRaw(RawItemsArgs),
    ParseCreate(ParseCreateArgs),
    Delete(ItemIdsArgs),
    GetById(LookupItemIdsArgs),
    GetEditableById(LookupItemIdsArgs),
}

#[derive(Debug, Args)]
pub struct ApiRawArgs {
    #[command(subcommand)]
    pub command: ApiRawCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiRawCommand {
    /// Send one raw protobuf Any command and print the raw response Any.
    Send(RawCommandArgs),
}

#[derive(Debug, Args)]
pub struct ApiDocumentArgs {
    #[command(subcommand)]
    pub command: ApiDocumentCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApiDocumentCommand {
    TitleBlock,
    SetTitleBlock(SetTitleBlockArgs),
    Save,
    SaveCopy(SaveCopyArgs),
    Revert,
    BoardString,
    SelectionString,
}

#[derive(Debug, Args)]
pub struct DocumentTypeArgs {
    #[arg(value_enum)]
    pub document_type: AgentDocumentType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentDocumentType {
    Schematic,
    Symbol,
    Pcb,
    Footprint,
    DrawingSheet,
    Project,
}

impl From<AgentDocumentType> for DocumentType {
    fn from(value: AgentDocumentType) -> Self {
        match value {
            AgentDocumentType::Schematic => Self::Schematic,
            AgentDocumentType::Symbol => Self::Symbol,
            AgentDocumentType::Pcb => Self::Pcb,
            AgentDocumentType::Footprint => Self::Footprint,
            AgentDocumentType::DrawingSheet => Self::DrawingSheet,
            AgentDocumentType::Project => Self::Project,
        }
    }
}

#[derive(Debug, Args)]
pub struct BinaryPathArgs {
    pub binary_name: String,
}

#[derive(Debug, Args)]
pub struct PluginSettingsArgs {
    pub identifier: String,
}

#[derive(Debug, Args)]
pub struct RefreshArgs {
    #[arg(long, value_enum, default_value_t = AgentFrameType::Pcb)]
    pub frame: AgentFrameType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentFrameType {
    ProjectManager,
    Schematic,
    Pcb,
    Spice,
    Symbol,
    Footprint,
    DrawingSheet,
}

impl From<AgentFrameType> for EditorFrameType {
    fn from(value: AgentFrameType) -> Self {
        match value {
            AgentFrameType::ProjectManager => Self::ProjectManager,
            AgentFrameType::Schematic => Self::SchematicEditor,
            AgentFrameType::Pcb => Self::PcbEditor,
            AgentFrameType::Spice => Self::SpiceSimulator,
            AgentFrameType::Symbol => Self::SymbolEditor,
            AgentFrameType::Footprint => Self::FootprintEditor,
            AgentFrameType::DrawingSheet => Self::DrawingSheetEditor,
        }
    }
}

#[derive(Debug, Args)]
pub struct RunActionArgs {
    pub action: String,
}

#[derive(Debug, Args)]
pub struct TextVariablesSetArgs {
    #[arg(long = "var", value_parser = parse_key_value)]
    pub vars: Vec<(String, String)>,

    #[arg(long, value_enum, default_value_t = AgentMergeMode::Merge)]
    pub mode: AgentMergeMode,
}

#[derive(Debug, Args)]
pub struct NetClassesSetArgs {
    #[arg(long, conflicts_with = "file")]
    pub json: Option<String>,

    #[arg(long, conflicts_with = "json")]
    pub file: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = AgentMergeMode::Merge)]
    pub mode: AgentMergeMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentMergeMode {
    Merge,
    Replace,
}

impl From<AgentMergeMode> for MapMergeMode {
    fn from(value: AgentMergeMode) -> Self {
        match value {
            AgentMergeMode::Merge => Self::Merge,
            AgentMergeMode::Replace => Self::Replace,
        }
    }
}

#[derive(Debug, Args)]
pub struct ExpandTextVariablesArgs {
    #[arg(required = true)]
    pub text: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TextValueArgs {
    pub text: String,
}

#[derive(Debug, Args)]
pub struct StackupUpdateArgs {
    #[arg(long, conflicts_with = "file")]
    pub json: Option<String>,

    #[arg(long, conflicts_with = "json")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SetEnabledLayersArgs {
    #[arg(long)]
    pub copper_layer_count: u32,

    #[arg(long = "layer", alias = "layer-id", required = true, value_parser = parse_layer_id)]
    pub layer_ids: Vec<i32>,
}

#[derive(Debug, Args)]
pub struct LayerIdArgs {
    #[arg(value_parser = parse_layer_id)]
    pub layer_id: i32,
}

#[derive(Debug, Args)]
pub struct LayerIdsArgs {
    #[arg(required = true, value_parser = parse_layer_id)]
    pub layer_ids: Vec<i32>,
}

#[derive(Debug, Args)]
pub struct BoardOriginArgs {
    #[arg(value_enum)]
    pub kind: AgentBoardOriginKind,
}

#[derive(Debug, Args)]
pub struct SetBoardOriginArgs {
    #[arg(value_enum)]
    pub kind: AgentBoardOriginKind,

    #[arg(value_parser = parse_point_nm)]
    pub at: PointNm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentBoardOriginKind {
    Grid,
    Drill,
}

impl From<AgentBoardOriginKind> for BoardOriginKind {
    fn from(value: AgentBoardOriginKind) -> Self {
        match value {
            AgentBoardOriginKind::Grid => Self::Grid,
            AgentBoardOriginKind::Drill => Self::Drill,
        }
    }
}

#[derive(Debug, Args)]
pub struct SetAppearanceArgs {
    #[arg(long, value_enum)]
    pub inactive_layer_display: Option<AgentInactiveLayerDisplayMode>,

    #[arg(long, value_enum)]
    pub net_color_display: Option<AgentNetColorDisplayMode>,

    #[arg(long, value_enum)]
    pub board_flip: Option<AgentBoardFlipMode>,

    #[arg(long, value_enum)]
    pub ratsnest_display: Option<AgentRatsnestDisplayMode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentInactiveLayerDisplayMode {
    Normal,
    Dimmed,
    Hidden,
}

impl From<AgentInactiveLayerDisplayMode> for InactiveLayerDisplayMode {
    fn from(value: AgentInactiveLayerDisplayMode) -> Self {
        match value {
            AgentInactiveLayerDisplayMode::Normal => Self::Normal,
            AgentInactiveLayerDisplayMode::Dimmed => Self::Dimmed,
            AgentInactiveLayerDisplayMode::Hidden => Self::Hidden,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentNetColorDisplayMode {
    All,
    Ratsnest,
    Off,
}

impl From<AgentNetColorDisplayMode> for NetColorDisplayMode {
    fn from(value: AgentNetColorDisplayMode) -> Self {
        match value {
            AgentNetColorDisplayMode::All => Self::All,
            AgentNetColorDisplayMode::Ratsnest => Self::Ratsnest,
            AgentNetColorDisplayMode::Off => Self::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentBoardFlipMode {
    Normal,
    FlippedX,
}

impl From<AgentBoardFlipMode> for BoardFlipMode {
    fn from(value: AgentBoardFlipMode) -> Self {
        match value {
            AgentBoardFlipMode::Normal => Self::Normal,
            AgentBoardFlipMode::FlippedX => Self::FlippedX,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentRatsnestDisplayMode {
    AllLayers,
    VisibleLayers,
}

impl From<AgentRatsnestDisplayMode> for RatsnestDisplayMode {
    fn from(value: AgentRatsnestDisplayMode) -> Self {
        match value {
            AgentRatsnestDisplayMode::AllLayers => Self::AllLayers,
            AgentRatsnestDisplayMode::VisibleLayers => Self::VisibleLayers,
        }
    }
}

#[derive(Debug, Args)]
pub struct ApiBoardItemsArgs {
    #[arg(long = "type", alias = "type-code", value_name = "TYPE", value_parser = parse_pcb_type_code)]
    pub type_codes: Vec<i32>,

    #[arg(long)]
    pub details: bool,
}

#[derive(Debug, Args)]
pub struct TypeCodesArgs {
    #[arg(long = "type", alias = "type-code", value_name = "TYPE", value_parser = parse_pcb_type_code)]
    pub type_codes: Vec<i32>,
}

#[derive(Debug, Args)]
pub struct ItemIdsArgs {
    #[arg(required = true)]
    pub item_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct LookupItemIdsArgs {
    #[arg(required = true)]
    pub item_ids: Vec<String>,

    /// Return success with missing=true if KiCad reports the IDs are absent.
    #[arg(long)]
    pub missing_ok: bool,
}

#[derive(Debug, Args)]
pub struct ItemsByNetArgs {
    #[arg(long = "type", alias = "type-code", value_name = "TYPE", value_parser = parse_pcb_type_code)]
    pub type_codes: Vec<i32>,

    #[arg(required = true)]
    pub nets: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ItemsByNetClassArgs {
    #[arg(long = "type", alias = "type-code", value_name = "TYPE", value_parser = parse_pcb_type_code)]
    pub type_codes: Vec<i32>,

    #[arg(required = true)]
    pub net_classes: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ConnectedItemsArgs {
    #[arg(long = "type", alias = "type-code", value_name = "TYPE", value_parser = parse_pcb_type_code)]
    pub type_codes: Vec<i32>,

    #[arg(required = true)]
    pub item_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct NetNamesArgs {
    #[arg(required = true)]
    pub nets: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PadShapeAsPolygonArgs {
    #[arg(long = "pad-id", required = true)]
    pub pad_ids: Vec<String>,

    #[arg(long = "layer", alias = "layer-id", value_parser = parse_layer_id)]
    pub layer_id: i32,
}

#[derive(Debug, Args)]
pub struct PadstackPresenceArgs {
    #[arg(long = "item-id", required = true)]
    pub item_ids: Vec<String>,

    #[arg(long = "layer", alias = "layer-id", required = true, value_parser = parse_layer_id)]
    pub layer_ids: Vec<i32>,
}

#[derive(Debug, Args)]
pub struct BoundingBoxesArgs {
    #[arg(long = "item-id", required = true)]
    pub item_ids: Vec<String>,

    /// Include footprint reference/value and other child text in the returned extents.
    #[arg(long)]
    pub include_child_text: bool,
}

#[derive(Debug, Args)]
pub struct HitTestArgs {
    pub item_id: String,

    #[arg(value_parser = parse_point_nm)]
    pub at: PointNm,

    #[arg(long, default_value_t = 0)]
    pub tolerance_nm: i32,
}

#[derive(Debug, Args)]
pub struct ZoneIdsArgs {
    #[arg(long = "zone-id")]
    pub zone_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct EndCommitArgs {
    #[arg(long)]
    pub session_id: String,

    #[arg(long, value_enum)]
    pub action: AgentCommitAction,

    #[arg(long, default_value = "kicad-ipc-cli")]
    pub message: String,
}

#[derive(Debug, Args)]
pub struct RawItemsArgs {
    #[arg(long, conflicts_with = "file")]
    pub json: Option<String>,

    #[arg(long, conflicts_with = "json")]
    pub file: Option<PathBuf>,

    #[arg(long)]
    pub container_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct CreateBoardTextArgs {
    /// Text string to create.
    #[arg(long)]
    pub text: String,

    /// Text position, e.g. `10mm,20mm` or `1000000nm,2500000nm`.
    #[arg(long, value_parser = parse_point_nm)]
    pub at: PointNm,

    /// Board layer name or id. Defaults to visible front silkscreen.
    #[arg(long, value_parser = parse_layer_id, default_value = "F.SilkS")]
    pub layer: i32,

    /// Text height.
    #[arg(long = "height", value_parser = parse_distance_nm, default_value = "1mm")]
    pub height_nm: i64,

    /// Text width. Defaults to --height.
    #[arg(long = "width", value_parser = parse_distance_nm)]
    pub width_nm: Option<i64>,

    /// Text stroke width.
    #[arg(long = "stroke-width", value_parser = parse_distance_nm, default_value = "0.15mm")]
    pub stroke_width_nm: i64,

    #[arg(long)]
    pub font: Option<String>,

    #[arg(long)]
    pub angle_degrees: Option<f64>,

    #[arg(long)]
    pub line_spacing: Option<f64>,

    #[arg(long)]
    pub bold: bool,

    #[arg(long)]
    pub italic: bool,

    #[arg(long)]
    pub underlined: bool,

    #[arg(long)]
    pub mirrored: bool,

    #[arg(long)]
    pub multiline: bool,

    #[arg(long)]
    pub keep_upright: bool,

    #[arg(long)]
    pub knockout: bool,

    #[arg(long)]
    pub locked: bool,

    #[arg(long)]
    pub hyperlink: Option<String>,

    #[arg(long)]
    pub container_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct CreateBoardTextsArgs {
    /// JSON file containing a board_texts array or a bare array.
    #[arg(long)]
    pub file: PathBuf,

    /// Optional target container id for all created text items.
    #[arg(long)]
    pub container_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct RawCommandArgs {
    #[arg(long, conflicts_with = "file")]
    pub json: Option<String>,

    #[arg(long, conflicts_with = "json")]
    pub file: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentCommitAction {
    Commit,
    Drop,
}

impl From<AgentCommitAction> for CommitAction {
    fn from(value: AgentCommitAction) -> Self {
        match value {
            AgentCommitAction::Commit => Self::Commit,
            AgentCommitAction::Drop => Self::Drop,
        }
    }
}

#[derive(Debug, Args)]
pub struct ParseCreateArgs {
    #[arg(long, conflicts_with = "file")]
    pub text: Option<String>,

    #[arg(long, conflicts_with = "text")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SetTitleBlockArgs {
    #[arg(long)]
    pub title: Option<String>,

    #[arg(long)]
    pub date: Option<String>,

    #[arg(long)]
    pub revision: Option<String>,

    #[arg(long)]
    pub company: Option<String>,

    #[arg(long = "comment")]
    pub comments: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SaveCopyArgs {
    pub path: PathBuf,

    #[arg(long)]
    pub overwrite: bool,

    #[arg(long)]
    pub include_project: bool,
}

fn parse_key_value(value: &str) -> Result<(String, String), String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| "expected KEY=VALUE".to_string())?;
    if key.is_empty() {
        return Err("key cannot be empty".to_string());
    }
    Ok((key.to_string(), value.to_string()))
}

pub fn parse_layer_id(value: &str) -> Result<i32, String> {
    BoardLayerInfo::id_from_name(value).ok_or_else(|| {
        format!(
            "unknown board layer `{value}`; use a name like F.SilkS, BL_F_SilkS, or a numeric id"
        )
    })
}

pub fn parse_pcb_type_code(value: &str) -> Result<i32, String> {
    if let Ok(code) = value.parse::<i32>() {
        return Ok(code);
    }

    PcbObjectTypeCode::from_name(value)
        .map(|object_type| object_type.code)
        .ok_or_else(|| {
            format!(
                "unknown PCB object type `{value}`; use names like track, footprint, pad, text, silkscreen-text, or a numeric code"
            )
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;
    use kicad_ipc_rs::PcbObjectTypeCode;

    use super::{parse_layer_id, parse_pcb_type_code};
    use super::{
        ApiBoardCommand, ApiCommand, ApiItemsCommand, Cli, Command, ComponentGroupsCommand,
        SelectCommand,
    };

    #[test]
    fn parses_layer_names_and_numeric_ids() {
        assert_eq!(parse_layer_id("F.SilkS"), Ok(40));
        assert_eq!(parse_layer_id("BL_F_SilkS"), Ok(40));
        assert_eq!(parse_layer_id("40"), Ok(40));
    }

    #[test]
    fn parses_friendly_pcb_type_names_and_numeric_codes() {
        assert_eq!(
            parse_pcb_type_code("track"),
            Ok(PcbObjectTypeCode::new_trace().code)
        );
        assert_eq!(
            parse_pcb_type_code("silkscreen-text"),
            Ok(PcbObjectTypeCode::new_text().code)
        );
        assert_eq!(
            parse_pcb_type_code("KOT_PCB_FOOTPRINT"),
            Ok(PcbObjectTypeCode::new_footprint().code)
        );
        assert_eq!(parse_pcb_type_code("12345"), Ok(12345));
    }

    #[test]
    fn select_by_id_parses_with_mode() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "select",
            "by-id",
            "abc",
            "def",
            "--mode",
            "add",
        ]);
        let Command::Select(args) = cli.command else {
            panic!("expected select command");
        };
        let SelectCommand::ById(args) = args.command else {
            panic!("expected by-id command");
        };
        assert_eq!(args.item_ids, ["abc", "def"]);
        assert_eq!(format!("{:?}", args.mode), "Add");
    }

    #[test]
    fn get_by_id_missing_ok_parses() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "api",
            "items",
            "get-by-id",
            "abc",
            "--missing-ok",
        ]);
        let Command::Api(args) = cli.command else {
            panic!("expected api command");
        };
        let ApiCommand::Items(args) = args.command else {
            panic!("expected api items command");
        };
        let ApiItemsCommand::GetById(args) = args.command else {
            panic!("expected get-by-id command");
        };
        assert_eq!(args.item_ids, ["abc"]);
        assert!(args.missing_ok);
    }

    #[test]
    fn component_group_suggest_subcommand_is_not_available() {
        let err = Cli::try_parse_from(["kicad-ipc-cli", "component-groups", "suggest"])
            .expect_err("component-groups suggest must stay removed");
        assert!(err
            .to_string()
            .contains("unrecognized subcommand 'suggest'"));
    }

    #[test]
    fn parses_component_group_keep_existing_flag() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "component-groups",
            "apply",
            "--plan",
            "groups.json",
            "--keep-existing",
        ]);

        let Command::ComponentGroups(args) = cli.command else {
            panic!("expected component-groups command");
        };
        let ComponentGroupsCommand::Apply(args) = args.command;
        assert_eq!(args.plan, PathBuf::from("groups.json"));
        assert!(args.keep_existing);
        assert!(args.replace_existing);
    }

    #[test]
    fn parses_component_group_replace_existing_false() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "component-groups",
            "apply",
            "--plan",
            "groups.json",
            "--replace-existing=false",
        ]);

        let Command::ComponentGroups(args) = cli.command else {
            panic!("expected component-groups command");
        };
        let ComponentGroupsCommand::Apply(args) = args.command;
        assert_eq!(args.plan, PathBuf::from("groups.json"));
        assert!(!args.replace_existing);
        assert!(!args.keep_existing);
    }

    #[test]
    fn bounding_boxes_excludes_child_text_by_default() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "api",
            "board",
            "bounding-boxes",
            "--item-id",
            "abc",
        ]);

        let Command::Api(args) = cli.command else {
            panic!("expected api command");
        };
        let ApiCommand::Board(args) = args.command else {
            panic!("expected api board command");
        };
        let ApiBoardCommand::BoundingBoxes(args) = args.command else {
            panic!("expected bounding-boxes command");
        };
        assert!(!args.include_child_text);
    }

    #[test]
    fn bounding_boxes_can_include_child_text_explicitly() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "api",
            "board",
            "bounding-boxes",
            "--item-id",
            "abc",
            "--include-child-text",
        ]);

        let Command::Api(args) = cli.command else {
            panic!("expected api command");
        };
        let ApiCommand::Board(args) = args.command else {
            panic!("expected api board command");
        };
        let ApiBoardCommand::BoundingBoxes(args) = args.command else {
            panic!("expected bounding-boxes command");
        };
        assert!(args.include_child_text);
    }
}
