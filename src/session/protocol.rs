//! The session protocol v1 (design 14.4, 14.5.6; `command.md` "Session
//! Protocol (v1)"): the envelope, every client and server message, and the
//! routing of server messages.
//!
//! Each message is a `kind` plus a `body` object. The enums are adjacently
//! tagged so serde maps `{"kind": .., "body": ..}` straight onto them; the
//! codec (`codec.rs`) adds `v`, `seq` and `re` and classifies errors.

use serde::{Deserialize, Serialize};

use crate::session::changes::Change;

/// The protocol version this session speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// One framed message: `{v, seq, kind, body}` plus `re` on replies.
#[derive(Clone, PartialEq, Debug)]
pub struct Envelope<T> {
    pub v: u32,
    /// A per-sender counter.
    pub seq: u64,
    /// On replies only: the client `seq` being answered.
    pub re: Option<u64>,
    /// The message; its kind is `body.kind()`.
    pub body: T,
}

impl<T> Envelope<T> {
    /// A version-1 envelope.
    #[must_use]
    pub fn new(seq: u64, re: Option<u64>, body: T) -> Envelope<T> {
        Envelope {
            v: PROTOCOL_VERSION,
            seq,
            re,
            body,
        }
    }
}

/// A byte range in the document revision named alongside it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct WireSpan {
    pub start: u32,
    pub end: u32,
}

impl WireSpan {
    #[must_use]
    pub const fn new(start: u32, end: u32) -> WireSpan {
        WireSpan { start, end }
    }
}

/// A source reference with its revision and form generation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireSrcRef {
    pub file: String,
    pub span: WireSpan,
    pub doc_revision: u64,
    pub form_gen: u64,
}

/// A diagnostic on the wire.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireDiag {
    pub code: String,
    /// `error`, `warning` or `hint`.
    pub severity: String,
    pub message: String,
    pub span: WireSpan,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    /// `[num, den]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat: Option<[i64; 2]>,
}

/// A site's enclosing call (`site.call`, TASK-010 G3): the nearest
/// enclosing symbol-headed call the literal is an argument of, directly or
/// inside a list or pattern argument.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireCall {
    pub name: String,
    pub head: WireSpan,
    /// 1-based among same-named calls in the top-level form.
    pub ordinal: u16,
    /// 0-based argument index.
    pub arg: u16,
    /// The named-argument keyword, else the declared parameter at `arg`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

/// A tweak site's tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireTier {
    Direct,
    Reeval,
    Manual,
}

/// Which Decided site kind a site is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireOrigin {
    PatternLiteral,
    Binding,
    InstDefault,
}

/// One tweak site.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WireSite {
    pub id: u32,
    pub span: WireSpan,
    pub tier: WireTier,
    pub origin: WireOrigin,
    pub value: f64,
    pub form_gen: u64,
    /// The `BindingKey` spelling, when the site is labeled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The nearest enclosing call (TASK-010 G3); absent with no enclosing
    /// call (e.g. a `let`/`inst`/`fn` header literal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<WireCall>,
}

/// A number, or a boolean for `set-var`. Integers stay integers.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireValue {
    Bool(bool),
    Int(i64),
    Float(f64),
}

/// A `set-tweak` value: a JSON number.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireNum {
    Int(i64),
    Float(f64),
}

impl WireNum {
    #[must_use]
    pub fn as_f64(self) -> f64 {
        match self {
            #[allow(clippy::cast_precision_loss)]
            WireNum::Int(n) => n as f64,
            WireNum::Float(x) => x,
        }
    }
}

/// A `learn` target: a `BindingKey` spelling or a tweak id.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LearnTarget {
    Id(u32),
    Key(String),
}

/// An empty body (`{}`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Empty {}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct EvalBody {
    pub file: String,
    /// The full document text at `doc_revision`.
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<WireSpan>,
    pub doc_revision: u64,
    pub edit_epoch: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StopBody {
    pub slot: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SetVarBody {
    pub file: String,
    pub name: String,
    pub value: WireValue,
    pub defining_form_gen: u64,
    pub edit_epoch: u64,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SetTweakBody {
    pub file: String,
    pub id: u32,
    pub form_gen: u64,
    pub value: WireNum,
    pub edit_epoch: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DocChangedBody {
    pub file: String,
    pub doc_revision: u64,
    pub base_revision: u64,
    pub changes: Vec<Change>,
    /// New-revision byte ranges.
    pub dirty: Vec<WireSpan>,
    pub edit_epoch: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LearnBody {
    pub file: String,
    pub binding: LearnTarget,
    pub cc: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<u8>,
    pub edit_epoch: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct SubscribeBody {
    pub telemetry: bool,
    pub levels: bool,
    pub diagnostics: bool,
}

/// A message from a client.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "kebab-case")]
pub enum ClientMsg {
    Eval(EvalBody),
    Hush(Empty),
    Stop(StopBody),
    SetVar(SetVarBody),
    SetTweak(SetTweakBody),
    DocChanged(DocChangedBody),
    Learn(LearnBody),
    Subscribe(SubscribeBody),
    #[serde(rename = "manifest?")]
    ManifestReq(Empty),
}

impl ClientMsg {
    /// Every client kind.
    pub const KINDS: [&'static str; 9] = [
        "eval",
        "hush",
        "stop",
        "set-var",
        "set-tweak",
        "doc-changed",
        "learn",
        "subscribe",
        "manifest?",
    ];

    /// The message kind.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            ClientMsg::Eval(_) => "eval",
            ClientMsg::Hush(_) => "hush",
            ClientMsg::Stop(_) => "stop",
            ClientMsg::SetVar(_) => "set-var",
            ClientMsg::SetTweak(_) => "set-tweak",
            ClientMsg::DocChanged(_) => "doc-changed",
            ClientMsg::Learn(_) => "learn",
            ClientMsg::Subscribe(_) => "subscribe",
            ClientMsg::ManifestReq(_) => "manifest?",
        }
    }
}

/// One evaluated form of an `eval-result`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireForm {
    pub span: WireSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<WireDiag>,
    pub form_gen: u64,
}

/// `#@ midi ch:` and other file-wide directive settings.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct WireFileLevel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_ch: Option<u8>,
}

/// One directive of the table.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireDirective {
    pub span: WireSpan,
    /// `positional` or `addressed`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<WireSpan>,
    pub trailing: bool,
}

/// One label of the table.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireLabel {
    pub name: String,
    pub spans: Vec<WireSpan>,
    pub ambiguous: bool,
}

/// One resolved panel binding of the table.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub span: WireSpan,
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<u8>,
    pub directive: WireSpan,
}

/// The directive table summary of an `eval-result` (13.5).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct WireDirectives {
    pub file_level: WireFileLevel,
    pub entries: Vec<WireDirective>,
    #[serde(default)]
    pub labels: Vec<WireLabel>,
    #[serde(default)]
    pub bindings: Vec<WireBinding>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EvalResultBody {
    pub file: String,
    pub doc_revision: u64,
    pub forms: Vec<WireForm>,
    pub diagnostics: Vec<WireDiag>,
    pub sites: Vec<WireSite>,
    pub directives: WireDirectives,
}

/// What a rejected write targeted.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StaleTarget {
    Id(u32),
    Name(String),
}

/// Why a write was rejected (14.5.6 authority rules).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StaleReason {
    StaleFormGen,
    EditInvalidated,
    UnreconciledEdit,
    SupersededDefinition,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StaleBindingBody {
    pub target: StaleTarget,
    pub reason: StaleReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_form_gen: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DirectiveEditBody {
    pub file: String,
    pub doc_revision: u64,
    pub span: WireSpan,
    pub expected: String,
    pub text: String,
}

/// One parameter's editor metadata (`editor-decl`, TASK-010 G2).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WireParamMeta {
    pub name: String,
    /// The control-table wire id; absent for a pattern-function parameter
    /// (no control-table row).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctl: Option<u16>,
    pub range: [f32; 2],
    /// `linear`, `log`, `stepped`, or a read-only payload marker.
    pub curve: String,
    /// `none`, `db`, `s`, `ms`, `hz`, `st`, `m`, or a payload stride label.
    pub unit: String,
    /// Parameters drawn together (bands of a multiband unit share one).
    pub group: u32,
    /// The instrument/template default (DDRUM-006 addition; an old session
    /// omits it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<f32>,
    /// Human-readable editor label (DDRUM-006 addition; an old session
    /// omits it, so a client falls back to prettifying `name` itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Enum domain names in index order (DDRUM-006 addition); omitted, not
    /// merely empty, for a non-enum parameter, to keep the payload small.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

/// One builtin's editor declaration (`manifest.editors`, TASK-010 G2).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WireEditorDecl {
    pub name: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiband: Option<bool>,
    pub params: Vec<WireParamMeta>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ManifestBody {
    pub sounds: Vec<String>,
    pub synths: Vec<String>,
    pub controls: Vec<String>,
    /// Every builtin's `EditorDecl` plus the pattern-function table
    /// (TASK-010 G2, `session::editors::editor_decls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editors: Option<Vec<WireEditorDecl>>,
}

/// A `protocol-error` code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    BadJson,
    UnsupportedVersion,
    UnknownKind,
    BadBody,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::BadJson => "bad-json",
            ErrorCode::UnsupportedVersion => "unsupported-version",
            ErrorCode::UnknownKind => "unknown-kind",
            ErrorCode::BadBody => "bad-body",
        }
    }
}

/// A malformed client message; the connection stays open.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
}

impl ProtocolError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> ProtocolError {
        ProtocolError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ProtocolError {}

/// One changed top-level name of a `bindings` batch.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireChanged {
    pub name: String,
    pub value: String,
    pub form_gen: u64,
}

/// A form's final state in a pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireState {
    Ok,
    Failed,
    Blocked,
}

/// One scheduled form of a `bindings` batch.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireFormState {
    pub name: String,
    pub state: WireState,
    /// The committed (restored) display value.
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<WireDiag>,
}

/// ONE batch per completed reactive pass (14.5.5).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BindingsBody {
    pub pass: u64,
    pub changed: Vec<WireChanged>,
    pub sites: Vec<WireSite>,
    pub states: Vec<WireFormState>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireClear {
    pub slot: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct DiagBody {
    pub add: Vec<WireDiag>,
    pub clear: Vec<WireClear>,
}

/// One realized event.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WirePlaying {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    pub slot: String,
    pub beat: [i64; 2],
    pub time: f64,
    pub dur: [i64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<WireSrcRef>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct PlayingBody {
    pub events: Vec<WirePlaying>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WireLevel {
    pub source: String,
    pub rms: f64,
    /// The host FFT bands (TASK-010 G4); `:master` only in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bands: Option<[f32; 8]>,
}

/// One analyzer unit's current cells (`levels.analyzers`, TASK-010 G4).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WireAnalyzer {
    /// `:master` or the bus name.
    pub bus: String,
    pub kind: String,
    /// The unit's first analysis cell (its constant `id` parameter).
    pub id: u32,
    pub cells: Vec<f32>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct LevelsBody {
    pub levels: Vec<WireLevel>,
    /// Every `EffectKind::Analyzer` unit of the installed bus graph with a
    /// constant `id` (TASK-010 G4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzers: Option<Vec<WireAnalyzer>>,
}

/// The transport's clock state (`tempo.clock`, TASK-010 G4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WireClock {
    /// `internal` or `midi`.
    pub source: String,
    /// Whether the slave is locked (not freewheeling); present only under
    /// `midi`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
}

/// Processing-time transport snapshot; unavailable latency is explicit.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TransportSample {
    pub epoch: String,
    pub sample_time: f64,
    pub cycle: [i64; 2],
    pub bpm: f64,
    pub beats_per_cycle: f64,
    pub running: bool,
    pub latency_seconds: Option<f64>,
    pub latency_kind: String,
    pub uncertainty_seconds: Option<f64>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TempoBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportSample>,
    pub bpm: f64,
    pub beats_per_cycle: i64,
    pub cycle: [i64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<WireClock>,
}

/// A message from the session.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "kebab-case")]
pub enum ServerMsg {
    EvalResult(EvalResultBody),
    StaleBinding(StaleBindingBody),
    DirectiveEdit(DirectiveEditBody),
    Manifest(ManifestBody),
    ProtocolError(ProtocolError),
    Bindings(BindingsBody),
    Diag(DiagBody),
    Playing(PlayingBody),
    Levels(LevelsBody),
    Tempo(TempoBody),
}

/// A subscriber topic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topic {
    /// `bindings`: every connection.
    Bindings,
    /// `tempo`: every connection.
    Tempo,
    /// `diag`: connections subscribed to diagnostics.
    Diagnostics,
    /// `playing`: connections subscribed to telemetry.
    Telemetry,
    /// `levels`: connections subscribed to levels.
    Levels,
}

/// Where a server message goes (14.5.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    Requester,
    Broadcast(Topic),
}

impl ServerMsg {
    /// Every server kind.
    pub const KINDS: [&'static str; 10] = [
        "eval-result",
        "stale-binding",
        "directive-edit",
        "manifest",
        "protocol-error",
        "bindings",
        "diag",
        "playing",
        "levels",
        "tempo",
    ];

    /// The message kind.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            ServerMsg::EvalResult(_) => "eval-result",
            ServerMsg::StaleBinding(_) => "stale-binding",
            ServerMsg::DirectiveEdit(_) => "directive-edit",
            ServerMsg::Manifest(_) => "manifest",
            ServerMsg::ProtocolError(_) => "protocol-error",
            ServerMsg::Bindings(_) => "bindings",
            ServerMsg::Diag(_) => "diag",
            ServerMsg::Playing(_) => "playing",
            ServerMsg::Levels(_) => "levels",
            ServerMsg::Tempo(_) => "tempo",
        }
    }

    /// The transport routing of this message (`command.md`).
    #[must_use]
    pub fn routing(&self) -> Route {
        match self {
            ServerMsg::EvalResult(_)
            | ServerMsg::StaleBinding(_)
            | ServerMsg::DirectiveEdit(_)
            | ServerMsg::Manifest(_)
            | ServerMsg::ProtocolError(_) => Route::Requester,
            ServerMsg::Bindings(_) => Route::Broadcast(Topic::Bindings),
            ServerMsg::Diag(_) => Route::Broadcast(Topic::Diagnostics),
            ServerMsg::Playing(_) => Route::Broadcast(Topic::Telemetry),
            ServerMsg::Levels(_) => Route::Broadcast(Topic::Levels),
            ServerMsg::Tempo(_) => Route::Broadcast(Topic::Tempo),
        }
    }
}

/// A connection's `subscribe` flags. `bindings` and `tempo` reach every
/// connection; the flags gate the other broadcast topics.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Subscription {
    pub telemetry: bool,
    pub levels: bool,
    pub diagnostics: bool,
}

impl Subscription {
    /// True when a message on `topic` reaches this connection.
    #[must_use]
    pub fn wants(self, topic: Topic) -> bool {
        match topic {
            Topic::Bindings | Topic::Tempo => true,
            Topic::Diagnostics => self.diagnostics,
            Topic::Telemetry => self.telemetry,
            Topic::Levels => self.levels,
        }
    }
}

impl From<SubscribeBody> for Subscription {
    fn from(b: SubscribeBody) -> Subscription {
        Subscription {
            telemetry: b.telemetry,
            levels: b.levels,
            diagnostics: b.diagnostics,
        }
    }
}
