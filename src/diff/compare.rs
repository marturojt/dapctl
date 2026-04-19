//! Produce a `Plan` by diffing two sorted lists of walker entries.

use super::plan::Plan;
use super::walker::Entry;
use crate::config::Verify;

pub fn compare(_src: &[Entry], _dst: &[Entry], _verify: Verify) -> Plan {
    Plan::default()
}
