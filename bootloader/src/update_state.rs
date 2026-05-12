// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bootloader firmware update session state.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FirmwareUpdateState {
    active: bool,
    expected_block_idx: usize,
}

impl FirmwareUpdateState {
    pub fn reset(&mut self) {
        self.active = false;
        self.expected_block_idx = 0;
    }

    pub fn start(&mut self) {
        self.active = true;
        self.expected_block_idx = 0;
    }

    pub fn expected_block_idx(&self) -> usize {
        self.expected_block_idx
    }

    pub fn accepts(&self, block_idx: usize) -> bool {
        self.active && block_idx == self.expected_block_idx
    }

    pub fn record_accepted(&mut self) -> bool {
        let Some(next_block_idx) = self.expected_block_idx.checked_add(1) else {
            return false;
        };
        self.expected_block_idx = next_block_idx;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_session_rejects_writes() {
        let state = FirmwareUpdateState::default();
        assert!(!state.accepts(0));
    }

    #[test]
    fn new_session_accepts_first_block() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        assert!(state.accepts(0));
    }

    #[test]
    fn duplicate_block_is_rejected() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        assert!(state.record_accepted());
        assert!(!state.accepts(0));
        assert_eq!(state.expected_block_idx(), 1);
    }

    #[test]
    fn skipped_block_is_rejected() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        assert!(!state.accepts(1));
        assert_eq!(state.expected_block_idx(), 0);
    }

    #[test]
    fn reordered_block_is_rejected() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        assert!(state.record_accepted());
        assert!(state.record_accepted());
        assert!(!state.accepts(1));
        assert_eq!(state.expected_block_idx(), 2);
    }

    #[test]
    fn stale_packet_after_new_session_is_rejected() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        assert!(state.record_accepted());
        state.start();
        assert!(!state.accepts(1));
        assert_eq!(state.expected_block_idx(), 0);
    }

    #[test]
    fn reset_closes_active_session() {
        let mut state = FirmwareUpdateState::default();
        state.start();
        state.reset();
        assert!(!state.accepts(0));
        assert_eq!(state.expected_block_idx(), 0);
    }
}
