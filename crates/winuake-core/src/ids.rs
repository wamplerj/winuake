//! Identifiers handed out by the workspace. Plain numbers so they cross the IPC boundary unchanged.

use std::fmt;

/// Ported from yakuake `Session::m_sessionId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// Ported from yakuake `Terminal::m_terminalId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalId(pub u64);

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Hands out session and terminal ids. Each counter starts at 0 and never reuses a value, like Yakuake's static counters.
#[derive(Debug, Clone, Default)]
pub struct IdAllocator {
    next_session: u64,
    next_terminal: u64,
}

impl IdAllocator {
    pub fn next_session(&mut self) -> SessionId {
        let id = SessionId(self.next_session);
        self.next_session += 1;
        id
    }

    pub fn next_terminal(&mut self) -> TerminalId {
        let id = TerminalId(self.next_terminal);
        self.next_terminal += 1;
        id
    }
}
