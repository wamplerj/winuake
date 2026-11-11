// Ported from yakuake app/sessionstack.cpp, app/tabbar.cpp, app/mainwindow.cpp (Q_SCRIPTABLE methods) @21c6210
//! Maps scripting calls (Yakuake's D-Bus method names) onto a `Workspace`. The caller applies the returned effects, exactly as it does for
//! shortcuts, so a scripted change looks the same as one made by hand.

use crate::protocol::{self, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use serde_json::{Value, json};
use winuake_core::ids::{SessionId, TerminalId};
use winuake_core::sessions::session::{SessionKind, TerminalSwitch};
use winuake_core::sessions::split_tree::GrowthDirection;
use winuake_core::sessions::workspace::{
    CloseOutcome, Confirmation, PaneGeometry, SplitHow, SplitTarget, Workspace, WorkspaceEffect,
};

/// Default pixel step of `tryGrowTerminal*`.
const DEFAULT_GROW_PIXELS: i64 = 10;

/// What a script can ask of the app beyond the workspace.
pub trait ScriptHost {
    fn toggle_window_state(&mut self);
    /// Types `text` into a terminal. False when the terminal does not exist.
    fn send_text(&mut self, terminal: TerminalId, text: &str) -> bool;
    /// Pixel area the panes occupy, for decisions that depend on size (auto split, grow).
    fn pane_geometry(&self) -> PaneGeometry;
}

pub struct ScriptSession<'a, H: ScriptHost> {
    workspace: &'a mut Workspace,
    host: &'a mut H,
    effects: Vec<WorkspaceEffect>,
}

struct Arguments<'a> {
    values: &'a [Value],
}

impl Arguments<'_> {
    fn missing(position: usize) -> RpcError {
        RpcError::new(INVALID_PARAMS, format!("argument {} is missing", position + 1))
    }

    fn integer(&self, position: usize) -> Result<i64, RpcError> {
        let value = self.values.get(position).ok_or_else(|| Self::missing(position))?;
        match value {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_f64().filter(|float| float.fract() == 0.0).map(|float| float as i64))
                .ok_or_else(|| {
                    RpcError::new(INVALID_PARAMS, format!("argument {} must be a whole number", position + 1))
                }),
            Value::String(text) => text.trim().parse().map_err(|_| {
                RpcError::new(INVALID_PARAMS, format!("argument {} must be a whole number", position + 1))
            }),
            _ => Err(RpcError::new(INVALID_PARAMS, format!("argument {} must be a whole number", position + 1))),
        }
    }

    fn integer_or(&self, position: usize, default: i64) -> Result<i64, RpcError> {
        if self.values.get(position).is_none_or(Value::is_null) { Ok(default) } else { self.integer(position) }
    }

    fn flag(&self, position: usize) -> Result<bool, RpcError> {
        match self.values.get(position).ok_or_else(|| Self::missing(position))? {
            Value::Bool(flag) => Ok(*flag),
            Value::String(text) if text.eq_ignore_ascii_case("true") => Ok(true),
            Value::String(text) if text.eq_ignore_ascii_case("false") => Ok(false),
            _ => Err(RpcError::new(INVALID_PARAMS, format!("argument {} must be true or false", position + 1))),
        }
    }

    fn flag_or(&self, position: usize, default: bool) -> Result<bool, RpcError> {
        if self.values.get(position).is_none_or(Value::is_null) { Ok(default) } else { self.flag(position) }
    }

    fn text(&self, position: usize) -> Result<String, RpcError> {
        match self.values.get(position).ok_or_else(|| Self::missing(position))? {
            Value::String(text) => Ok(text.clone()),
            Value::Number(number) => Ok(number.to_string()),
            Value::Bool(flag) => Ok(flag.to_string()),
            _ => Err(RpcError::new(INVALID_PARAMS, format!("argument {} must be text", position + 1))),
        }
    }
}

#[derive(Clone, Copy)]
enum SwitchOperation {
    IsSession,
    SetSession,
    IsTerminal,
    SetTerminal,
    SessionHasEnabled,
    SessionHasDisabled,
}

struct SwitchCall {
    operation: SwitchOperation,
    switch: TerminalSwitch,
}

/// Recognizes the three parallel families: `isSessionKeyboardInputEnabled`, `setTerminalMonitorSilenceEnabled`,
/// `hasTerminalsWithMonitorActivityDisabled`, and so on.
fn switch_call(method: &str) -> Option<SwitchCall> {
    const FAMILIES: [(&str, TerminalSwitch); 3] = [
        ("KeyboardInput", TerminalSwitch::KeyboardInput),
        ("MonitorActivity", TerminalSwitch::MonitorActivity),
        ("MonitorSilence", TerminalSwitch::MonitorSilence),
    ];
    for (name, switch) in FAMILIES {
        let operation = if method == format!("isSession{name}Enabled") {
            SwitchOperation::IsSession
        } else if method == format!("setSession{name}Enabled") {
            SwitchOperation::SetSession
        } else if method == format!("isTerminal{name}Enabled") {
            SwitchOperation::IsTerminal
        } else if method == format!("setTerminal{name}Enabled") {
            SwitchOperation::SetTerminal
        } else if method == format!("hasTerminalsWith{name}Enabled") {
            SwitchOperation::SessionHasEnabled
        } else if method == format!("hasTerminalsWith{name}Disabled") {
            SwitchOperation::SessionHasDisabled
        } else {
            continue;
        };
        return Some(SwitchCall { operation, switch });
    }
    None
}

fn id_value<T>(id: Option<T>, number: impl FnOnce(T) -> u64) -> Value {
    id.map_or(json!(-1), |id| json!(number(id)))
}

impl<'a, H: ScriptHost> ScriptSession<'a, H> {
    pub fn new(workspace: &'a mut Workspace, host: &'a mut H) -> Self {
        Self { workspace, host, effects: Vec::new() }
    }

    /// Effects of every call so far; the app applies them.
    pub fn into_effects(self) -> Vec<WorkspaceEffect> {
        self.effects
    }

    /// Handles one request line. `None` for a notification (a request without an id).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let request = match protocol::parse_request(line) {
            Ok(request) => request,
            Err(error) => return Some(protocol::error_line(&Value::Null, &error)),
        };
        let outcome = self.call(&request.method, &request.params);
        let id = request.id?;
        Some(match outcome {
            Ok(result) => protocol::success_line(&id, &result),
            Err(error) => protocol::error_line(&id, &error),
        })
    }

    fn session(&self, argument: i64) -> Option<SessionId> {
        let id = if argument < 0 { self.workspace.stack().active_session_id()? } else { SessionId(argument as u64) };
        self.workspace.stack().session(id).map(|session| session.id())
    }

    fn terminal(argument: i64) -> Option<TerminalId> {
        (argument >= 0).then_some(TerminalId(argument as u64))
    }

    fn apply(&mut self, mut effects: Vec<WorkspaceEffect>) {
        self.effects.append(&mut effects);
    }

    pub fn call(&mut self, method: &str, values: &[Value]) -> Result<Value, RpcError> {
        let arguments = Arguments { values };
        if let Some(call) = switch_call(method) {
            return self.switch(&call, &arguments);
        }
        match method {
            "toggleWindowState" => {
                self.host.toggle_window_state();
                Ok(Value::Null)
            }
            "tabTitle" => {
                let title = self
                    .session(arguments.integer(0)?)
                    .and_then(|id| self.workspace.tabs().title(id))
                    .unwrap_or_default();
                Ok(json!(title))
            }
            "setTabTitle" => {
                let (title, interactive) = (arguments.text(1)?, arguments.flag_or(2, true)?);
                if let Some(session) = self.session(arguments.integer(0)?) {
                    let effects = self.workspace.set_tab_title(session, &title, interactive);
                    self.apply(effects);
                }
                Ok(Value::Null)
            }
            "sessionAtTab" => {
                let index = arguments.integer(0)?;
                let found = usize::try_from(index).ok().and_then(|index| self.workspace.tabs().session_at(index));
                Ok(id_value(found, |id| id.0))
            }
            "addSession" => Ok(self.add_session(SessionKind::Single)),
            "addSessionTwoHorizontal" => Ok(self.add_session(SessionKind::TwoHorizontal)),
            "addSessionTwoVertical" => Ok(self.add_session(SessionKind::TwoVertical)),
            "addSessionQuad" => Ok(self.add_session(SessionKind::Quad)),
            "raiseSession" => {
                if let Some(session) = self.session(arguments.integer(0)?) {
                    let effects = self.workspace.raise_session(session);
                    self.apply(effects);
                }
                Ok(Value::Null)
            }
            "removeSession" => {
                let Some(session) = self.session(arguments.integer(0)?) else { return Ok(Value::Null) };
                match self.workspace.close_session(Some(session), Confirmation::Ask) {
                    CloseOutcome::Done(effects) => {
                        self.apply(effects);
                        Ok(Value::Null)
                    }
                    CloseOutcome::ConfirmationRequired(_) => Err(RpcError::new(
                        REFUSED,
                        "the session is locked against closing; call setSessionClosable first",
                    )),
                }
            }
            "removeTerminal" => {
                if let Some(terminal) = Self::terminal(arguments.integer(0)?) {
                    let effects = self.workspace.remove_terminal(terminal);
                    self.apply(effects);
                }
                Ok(Value::Null)
            }
            "splitSessionAuto" => self.split(&arguments, SplitHow::Auto, true),
            "splitSessionLeftRight" => self.split(&arguments, SplitHow::LeftRight, true),
            "splitSessionTopBottom" => self.split(&arguments, SplitHow::TopBottom, true),
            "splitTerminalLeftRight" => self.split(&arguments, SplitHow::LeftRight, false),
            "splitTerminalTopBottom" => self.split(&arguments, SplitHow::TopBottom, false),
            "tryGrowTerminalRight" => self.grow(GrowthDirection::Right, &arguments),
            "tryGrowTerminalLeft" => self.grow(GrowthDirection::Left, &arguments),
            "tryGrowTerminalTop" => self.grow(GrowthDirection::Up, &arguments),
            "tryGrowTerminalBottom" => self.grow(GrowthDirection::Down, &arguments),
            "activeSessionId" => Ok(id_value(self.workspace.stack().active_session_id(), |id| id.0)),
            "activeTerminalId" => Ok(id_value(self.workspace.stack().active_terminal_id(), |id| id.0)),
            "sessionIdList" => Ok(json!(join_ids(self.workspace.stack().session_ids().iter().map(|id| id.0)))),
            "terminalIdList" => Ok(json!(join_ids(self.workspace.stack().terminal_ids().iter().map(|id| id.0)))),
            "terminalIdsForSessionId" => {
                let ids = self
                    .session(arguments.integer(0)?)
                    .and_then(|id| self.workspace.stack().session(id))
                    .map(|session| session.terminal_ids());
                Ok(json!(match ids {
                    Some(ids) => join_ids(ids.iter().map(|id| id.0)),
                    None => "-1".to_owned(),
                }))
            }
            "sessionIdForTerminalId" => {
                let found = Self::terminal(arguments.integer(0)?)
                    .and_then(|terminal| self.workspace.stack().session_for_terminal(terminal));
                Ok(id_value(found, |id| id.0))
            }
            "runCommand" => {
                let command = arguments.text(0)?;
                if let Some(terminal) = self.workspace.stack().active_terminal_id() {
                    self.host.send_text(terminal, &format!("{command}\r"));
                }
                Ok(Value::Null)
            }
            "runCommandInTerminal" => {
                let (terminal, command) = (arguments.integer(0)?, arguments.text(1)?);
                if let Some(terminal) = Self::terminal(terminal) {
                    self.host.send_text(terminal, &format!("{command}\r"));
                }
                Ok(Value::Null)
            }
            "isSessionClosable" => {
                let closable = self
                    .session(arguments.integer(0)?)
                    .and_then(|id| self.workspace.stack().session(id))
                    .is_some_and(|session| session.closable());
                Ok(json!(closable))
            }
            "setSessionClosable" => {
                let (session, closable) = (arguments.integer(0)?, arguments.flag(1)?);
                if let Some(session) = self.session(session) {
                    self.workspace.set_session_closable(Some(session), closable);
                }
                Ok(Value::Null)
            }
            "hasUnclosableSessions" => Ok(json!(self.workspace.stack().has_unclosable_sessions())),
            other => Err(RpcError::new(METHOD_NOT_FOUND, format!("no such method: {other}"))),
        }
    }

    fn add_session(&mut self, kind: SessionKind) -> Value {
        let effects = self.workspace.add_session(kind);
        let created = effects.iter().find_map(|effect| match effect {
            WorkspaceEffect::SessionAdded { session, .. } => Some(*session),
            _ => None,
        });
        self.apply(effects);
        id_value(created, |id| id.0)
    }

    fn split(&mut self, arguments: &Arguments<'_>, how: SplitHow, by_session: bool) -> Result<Value, RpcError> {
        let argument = arguments.integer(0)?;
        let target = if by_session {
            match self.session(argument) {
                Some(session) => SplitTarget::SessionActive(session),
                None => return Ok(json!(-1)),
            }
        } else {
            match Self::terminal(argument) {
                Some(terminal) => SplitTarget::Terminal(terminal),
                None => return Ok(json!(-1)),
            }
        };
        let geometry = self.host.pane_geometry();
        let outcome = self.workspace.split_by_id(target, how, geometry);
        self.apply(outcome.effects);
        Ok(id_value(outcome.terminal, |id| id.0))
    }

    fn grow(&mut self, direction: GrowthDirection, arguments: &Arguments<'_>) -> Result<Value, RpcError> {
        let terminal = arguments.integer(0)?;
        let pixels = arguments.integer_or(1, DEFAULT_GROW_PIXELS)?.clamp(0, i64::from(i32::MAX)) as i32;
        let Some(terminal) = Self::terminal(terminal) else { return Ok(json!(-1)) };
        let geometry = self.host.pane_geometry();
        let grown = self.workspace.grow_terminal(terminal, direction, pixels, geometry);
        if let Some(session) = self.workspace.stack().session_for_terminal(terminal) {
            self.effects.push(WorkspaceEffect::LayoutChanged { session });
        }
        Ok(json!(grown.unwrap_or(-1)))
    }

    fn switch(&mut self, call: &SwitchCall, arguments: &Arguments<'_>) -> Result<Value, RpcError> {
        let switch = call.switch;
        match call.operation {
            SwitchOperation::IsSession => {
                let enabled = self
                    .session(arguments.integer(0)?)
                    .and_then(|id| self.workspace.stack().session(id))
                    .is_some_and(|session| session.all_enabled(switch));
                Ok(json!(enabled))
            }
            SwitchOperation::SetSession => {
                let (session, enabled) = (arguments.integer(0)?, arguments.flag(1)?);
                if let Some(session) = self.session(session) {
                    self.workspace.set_switch(session, None, switch, enabled);
                }
                Ok(Value::Null)
            }
            SwitchOperation::IsTerminal => {
                let terminal = Self::terminal(arguments.integer(0)?);
                let enabled = terminal.and_then(|terminal| {
                    let session = self.workspace.stack().session_for_terminal(terminal)?;
                    Some(self.workspace.stack().session(session)?.switch_state(switch, terminal))
                });
                Ok(json!(enabled.unwrap_or(false)))
            }
            SwitchOperation::SetTerminal => {
                let (terminal, enabled) = (arguments.integer(0)?, arguments.flag(1)?);
                if let Some(terminal) = Self::terminal(terminal)
                    && let Some(session) = self.workspace.stack().session_for_terminal(terminal)
                {
                    self.workspace.set_switch(session, Some(terminal), switch, enabled);
                }
                Ok(Value::Null)
            }
            SwitchOperation::SessionHasEnabled | SwitchOperation::SessionHasDisabled => {
                let wanted_enabled = matches!(call.operation, SwitchOperation::SessionHasEnabled);
                let found = self
                    .session(arguments.integer(0)?)
                    .and_then(|id| self.workspace.stack().session(id))
                    .is_some_and(|session| {
                        if wanted_enabled { session.any_enabled(switch) } else { session.any_disabled(switch) }
                    });
                Ok(json!(found))
            }
        }
    }
}

fn join_ids(ids: impl Iterator<Item = u64>) -> String {
    ids.map(|id| id.to_string()).collect::<Vec<_>>().join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use winuake_core::sessions::split_tree::PaneRect;

    #[derive(Default)]
    struct FakeHost {
        toggles: u32,
        typed: Vec<(TerminalId, String)>,
    }

    impl ScriptHost for FakeHost {
        fn toggle_window_state(&mut self) {
            self.toggles += 1;
        }

        fn send_text(&mut self, terminal: TerminalId, text: &str) -> bool {
            self.typed.push((terminal, text.to_owned()));
            true
        }

        fn pane_geometry(&self) -> PaneGeometry {
            PaneGeometry { area: PaneRect::new(0, 0, 1000, 400), divider: 0, minimum_extent: 10 }
        }
    }

    struct Rig {
        workspace: Workspace,
        host: FakeHost,
    }

    impl Rig {
        fn new() -> Self {
            Self { workspace: Workspace::default(), host: FakeHost::default() }
        }

        fn call(&mut self, method: &str, params: &[Value]) -> Result<Value, RpcError> {
            ScriptSession::new(&mut self.workspace, &mut self.host).call(method, params)
        }

        fn ok(&mut self, method: &str, params: &[Value]) -> Value {
            self.call(method, params).unwrap_or_else(|error| panic!("{method}: {error}"))
        }
    }

    #[test]
    fn given_no_sessions_when_asking_for_active_ids_then_minus_one_comes_back() {
        let mut rig = Rig::new();
        assert_eq!(rig.ok("activeSessionId", &[]), json!(-1));
        assert_eq!(rig.ok("activeTerminalId", &[]), json!(-1));
        assert_eq!(rig.ok("sessionIdList", &[]), json!(""));
        assert_eq!(rig.ok("terminalIdList", &[]), json!(""));
    }

    #[test]
    fn given_the_add_methods_when_called_then_sessions_with_the_right_terminal_counts_appear() {
        let mut rig = Rig::new();
        assert_eq!(rig.ok("addSession", &[]), json!(0));
        assert_eq!(rig.ok("addSessionTwoHorizontal", &[]), json!(1));
        assert_eq!(rig.ok("addSessionTwoVertical", &[]), json!(2));
        assert_eq!(rig.ok("addSessionQuad", &[]), json!(3));
        assert_eq!(rig.ok("sessionIdList", &[]), json!("0,1,2,3"));
        assert_eq!(rig.ok("terminalIdList", &[]).as_str().map(|list| list.split(',').count()), Some(9));
        assert_eq!(rig.ok("activeSessionId", &[]), json!(3));
        assert_eq!(rig.ok("terminalIdsForSessionId", &[json!(1)]), json!("1,2"));
        assert_eq!(rig.ok("terminalIdsForSessionId", &[json!(99)]), json!("-1"));
    }

    #[test]
    fn given_two_sessions_when_raising_and_looking_up_then_ids_and_tab_positions_agree() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        rig.ok("addSession", &[]);
        rig.ok("raiseSession", &[json!(0)]);
        assert_eq!(rig.ok("activeSessionId", &[]), json!(0));
        assert_eq!(rig.ok("sessionAtTab", &[json!(1)]), json!(1));
        assert_eq!(rig.ok("sessionAtTab", &[json!(5)]), json!(-1));
        assert_eq!(rig.ok("sessionAtTab", &[json!(-1)]), json!(-1));
        assert_eq!(rig.ok("sessionIdForTerminalId", &[json!(1)]), json!(1));
        assert_eq!(rig.ok("sessionIdForTerminalId", &[json!(42)]), json!(-1));
        rig.ok("raiseSession", &[json!(77)]);
        assert_eq!(rig.ok("activeSessionId", &[]), json!(0));
    }

    #[test]
    fn given_a_tab_when_retitled_then_the_new_title_is_read_back_and_unknown_ids_give_empty_text() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        assert_eq!(rig.ok("tabTitle", &[json!(0)]), json!("Shell"));
        rig.ok("setTabTitle", &[json!(0), json!("deploy")]);
        assert_eq!(rig.ok("tabTitle", &[json!(0)]), json!("deploy"));
        assert_eq!(rig.ok("tabTitle", &[json!(-1)]), json!("deploy"));
        assert_eq!(rig.ok("tabTitle", &[json!(9)]), json!(""));
        rig.ok("setTabTitle", &[json!(0), json!("auto"), json!(false)]);
        assert_eq!(
            rig.ok("tabTitle", &[json!(0)]),
            json!("deploy"),
            "an automatic title does not replace a pinned one"
        );
    }

    #[test]
    fn given_splits_by_session_and_by_terminal_when_called_then_new_terminal_ids_are_returned() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        assert_eq!(rig.ok("splitSessionLeftRight", &[json!(0)]), json!(1));
        assert_eq!(rig.ok("splitTerminalTopBottom", &[json!(0)]), json!(2));
        assert_eq!(rig.ok("splitSessionAuto", &[json!(-1)]), json!(3));
        assert_eq!(rig.ok("splitSessionTopBottom", &[json!(0)]), json!(4));
        assert_eq!(rig.ok("splitTerminalLeftRight", &[json!(3)]), json!(5));
        assert_eq!(rig.ok("terminalIdsForSessionId", &[json!(0)]), json!("0,1,2,3,4,5"));
        assert_eq!(rig.ok("splitSessionLeftRight", &[json!(9)]), json!(-1));
        assert_eq!(rig.ok("splitTerminalLeftRight", &[json!(99)]), json!(-1));
    }

    #[test]
    fn given_split_terminals_when_growing_then_pixels_default_to_ten_and_impossible_growth_is_minus_one() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        rig.ok("splitSessionLeftRight", &[json!(0)]);
        assert_eq!(rig.ok("tryGrowTerminalRight", &[json!(0)]), json!(10));
        assert_eq!(rig.ok("tryGrowTerminalRight", &[json!(0), json!(25)]), json!(25));
        assert_eq!(rig.ok("tryGrowTerminalLeft", &[json!(1), json!(5)]), json!(5));
        assert_eq!(rig.ok("tryGrowTerminalLeft", &[json!(0)]), json!(-1));
        assert_eq!(rig.ok("tryGrowTerminalTop", &[json!(0)]), json!(-1));
        assert_eq!(rig.ok("tryGrowTerminalBottom", &[json!(99)]), json!(-1));
    }

    #[test]
    fn given_sessions_and_terminals_when_removed_then_they_go_and_the_last_one_leaves_an_empty_workspace() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        rig.ok("splitSessionLeftRight", &[json!(0)]);
        rig.ok("removeTerminal", &[json!(1)]);
        assert_eq!(rig.ok("terminalIdsForSessionId", &[json!(0)]), json!("0"));
        rig.ok("addSession", &[]);
        rig.ok("removeSession", &[json!(0)]);
        assert_eq!(rig.ok("sessionIdList", &[]), json!("1"));
        rig.ok("removeSession", &[json!(-1)]);
        assert_eq!(rig.ok("sessionIdList", &[]), json!(""));
        rig.ok("removeSession", &[json!(5)]);
        rig.ok("removeTerminal", &[json!(5)]);
    }

    #[test]
    fn given_a_locked_session_when_removing_it_then_the_call_is_refused_until_it_is_unlocked() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        rig.ok("splitSessionLeftRight", &[json!(0)]);
        assert_eq!(rig.ok("isSessionClosable", &[json!(0)]), json!(true));
        rig.ok("setSessionClosable", &[json!(0), json!(false)]);
        assert_eq!(rig.ok("isSessionClosable", &[json!(-1)]), json!(false));
        assert_eq!(rig.ok("hasUnclosableSessions", &[]), json!(true));
        assert_eq!(rig.call("removeSession", &[json!(0)]).unwrap_err().code, REFUSED);
        rig.ok("removeTerminal", &[json!(1)]);
        assert_eq!(rig.ok("terminalIdsForSessionId", &[json!(0)]), json!("0,1"), "removeTerminal respects the lock");
        rig.ok("setSessionClosable", &[json!(0), json!(false)]);
        rig.ok("setSessionClosable", &[json!(0), json!(true)]);
        rig.ok("removeSession", &[json!(0)]);
        assert_eq!(rig.ok("hasUnclosableSessions", &[]), json!(false));
        assert_eq!(rig.ok("isSessionClosable", &[json!(9)]), json!(false));
    }

    #[test]
    fn given_commands_when_run_then_text_plus_enter_goes_to_the_right_terminal() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        rig.ok("addSession", &[]);
        rig.ok("runCommand", &[json!("dir")]);
        rig.ok("runCommandInTerminal", &[json!(0), json!("echo hi")]);
        rig.ok("runCommandInTerminal", &[json!(-5), json!("ignored")]);
        assert_eq!(rig.host.typed, vec![(TerminalId(1), "dir\r".to_owned()), (TerminalId(0), "echo hi\r".to_owned())]);
    }

    #[test]
    fn given_no_session_when_running_a_command_then_nothing_is_typed() {
        let mut rig = Rig::new();
        rig.ok("runCommand", &[json!("dir")]);
        assert!(rig.host.typed.is_empty());
    }

    #[test]
    fn given_the_toggle_call_when_made_then_the_host_toggles() {
        let mut rig = Rig::new();
        rig.ok("toggleWindowState", &[]);
        assert_eq!(rig.host.toggles, 1);
    }

    #[test]
    fn given_the_switch_families_when_set_and_read_then_session_and_terminal_levels_agree() {
        let mut rig = Rig::new();
        rig.ok("addSessionTwoHorizontal", &[]);
        for family in ["KeyboardInput", "MonitorActivity", "MonitorSilence"] {
            let default_on = family == "KeyboardInput";
            assert_eq!(rig.ok(&format!("isSession{family}Enabled"), &[json!(0)]), json!(default_on), "{family}");
            rig.ok(&format!("setTerminal{family}Enabled"), &[json!(1), json!(!default_on)]);
            assert_eq!(rig.ok(&format!("isTerminal{family}Enabled"), &[json!(1)]), json!(!default_on));
            assert_eq!(rig.ok(&format!("isTerminal{family}Enabled"), &[json!(0)]), json!(default_on));
            assert_eq!(
                rig.ok(&format!("isSession{family}Enabled"), &[json!(0)]),
                json!(false),
                "mixed means not all enabled"
            );
            assert_eq!(rig.ok(&format!("hasTerminalsWith{family}Enabled"), &[json!(0)]), json!(true));
            assert_eq!(rig.ok(&format!("hasTerminalsWith{family}Disabled"), &[json!(0)]), json!(true));
            rig.ok(&format!("setSession{family}Enabled"), &[json!(0), json!(true)]);
            assert_eq!(rig.ok(&format!("isSession{family}Enabled"), &[json!(-1)]), json!(true));
            assert_eq!(rig.ok(&format!("hasTerminalsWith{family}Disabled"), &[json!(0)]), json!(false));
        }
        assert_eq!(rig.ok("isTerminalMonitorSilenceEnabled", &[json!(99)]), json!(false));
        assert_eq!(rig.ok("isSessionKeyboardInputEnabled", &[json!(99)]), json!(false));
    }

    #[test]
    fn given_bad_calls_when_made_then_the_error_codes_name_the_problem() {
        let mut rig = Rig::new();
        assert_eq!(rig.call("noSuchMethod", &[]).unwrap_err().code, METHOD_NOT_FOUND);
        assert_eq!(rig.call("raiseSession", &[]).unwrap_err().code, INVALID_PARAMS);
        assert_eq!(rig.call("raiseSession", &[json!("abc")]).unwrap_err().code, INVALID_PARAMS);
        assert_eq!(rig.call("raiseSession", &[json!([1])]).unwrap_err().code, INVALID_PARAMS);
        assert_eq!(rig.call("setSessionClosable", &[json!(0), json!(7)]).unwrap_err().code, INVALID_PARAMS);
        assert_eq!(rig.call("setTabTitle", &[json!(0), json!([])]).unwrap_err().code, INVALID_PARAMS);
    }

    #[test]
    fn given_loose_argument_types_when_calling_then_numeric_text_and_integral_floats_are_accepted() {
        let mut rig = Rig::new();
        rig.ok("addSession", &[]);
        assert_eq!(rig.ok("tabTitle", &[json!("0")]), json!("Shell"));
        assert_eq!(rig.ok("tabTitle", &[json!(0.0)]), json!("Shell"));
        rig.ok("setSessionClosable", &[json!(0), json!("false")]);
        assert_eq!(rig.ok("isSessionClosable", &[json!(0)]), json!(false));
        rig.ok("setTabTitle", &[json!(0), json!(42)]);
        assert_eq!(rig.ok("tabTitle", &[json!(0)]), json!("42"));
    }

    #[test]
    fn given_request_lines_when_handled_then_replies_carry_ids_and_notifications_get_none() {
        let mut rig = Rig::new();
        let mut session = ScriptSession::new(&mut rig.workspace, &mut rig.host);
        let reply = session.handle_line(r#"{"jsonrpc":"2.0","id":9,"method":"addSession"}"#).expect("reply");
        assert_eq!(serde_json::from_str::<Value>(&reply).expect("json"), json!({"jsonrpc":"2.0","id":9,"result":0}));
        assert!(session.handle_line(r#"{"jsonrpc":"2.0","method":"toggleWindowState"}"#).is_none());
        let unknown = session.handle_line(r#"{"jsonrpc":"2.0","id":"a","method":"nope"}"#).expect("reply");
        assert!(unknown.contains("-32601") && unknown.contains("\"a\""));
        let broken = session.handle_line("not json").expect("reply");
        assert!(broken.contains("-32700") && broken.contains("null"));
        assert_eq!(rig.host.toggles, 1);
    }

    #[test]
    fn given_calls_that_change_the_workspace_when_finished_then_the_effects_are_returned_for_the_app_to_apply() {
        let mut rig = Rig::new();
        let mut session = ScriptSession::new(&mut rig.workspace, &mut rig.host);
        session.call("addSession", &[]).expect("ok");
        session.call("splitSessionLeftRight", &[json!(0)]).expect("ok");
        session.call("setTabTitle", &[json!(0), json!("x")]).expect("ok");
        let effects = session.into_effects();
        assert!(effects.iter().any(|effect| matches!(effect, WorkspaceEffect::SessionAdded { .. })));
        assert!(effects.iter().any(|effect| matches!(effect, WorkspaceEffect::TerminalAdded { .. })));
        assert!(effects.contains(&WorkspaceEffect::TabsChanged));
    }
}
