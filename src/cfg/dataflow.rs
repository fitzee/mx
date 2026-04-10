//! Generic forward dataflow framework over CFG.
//!
//! Parameterized by a `ForwardAnalysis` trait that defines the lattice
//! (state type), transfer functions, and join operation.  The framework
//! iterates to a fixpoint using a worklist algorithm.

use super::{BlockId, Cfg, Terminator};
use std::collections::VecDeque;

/// Trait for a forward dataflow analysis.
///
/// `State` is the abstract state (lattice element) propagated through
/// the CFG.  It must implement Clone + PartialEq so the framework can
/// detect fixpoint (no state change → stop iterating).
pub trait ForwardAnalysis {
    /// The abstract state propagated at each program point.
    type State: Clone + PartialEq;

    /// Initial state at the CFG entry block.
    fn entry_state(&self) -> Self::State;

    /// Bottom element — the initial state for non-entry blocks before
    /// any predecessor has been processed.  For "must" analyses (e.g.
    /// definitely assigned), this should be the TOP of the lattice
    /// (all bits set) so that the first real predecessor narrows it.
    fn bottom(&self) -> Self::State;

    /// Join two states at a merge point (block with multiple preds).
    /// For "must" analyses: intersection.  For "may" analyses: union.
    fn join(&self, a: &Self::State, b: &Self::State) -> Self::State;

    /// Transfer function: transform state after processing one block.
    /// Receives the block index and the current incoming state.
    /// Returns the outgoing state after the block's statements and terminator.
    fn transfer(&self, cfg: &Cfg, block_id: BlockId, state: &Self::State) -> Self::State;
}

/// Result of a forward dataflow analysis: the state at entry and exit
/// of each block.
pub struct DataflowResult<S> {
    /// State at the entry of each block (after join from predecessors).
    pub block_in: Vec<S>,
    /// State at the exit of each block (after transfer function).
    pub block_out: Vec<S>,
}

/// Run a forward dataflow analysis to fixpoint.
///
/// Requires `cfg.preds_valid == true` (call `cfg.compute_preds()` first).
pub fn solve_forward<A: ForwardAnalysis>(analysis: &A, cfg: &Cfg) -> DataflowResult<A::State> {
    assert!(cfg.preds_valid, "compute_preds() must be called before dataflow");

    let n = cfg.blocks.len();
    let mut block_in: Vec<A::State> = vec![analysis.bottom(); n];
    let mut block_out: Vec<A::State> = vec![analysis.bottom(); n];

    // Entry block starts with the entry state
    block_in[cfg.entry] = analysis.entry_state();
    block_out[cfg.entry] = analysis.transfer(cfg, cfg.entry, &block_in[cfg.entry]);

    // Worklist: start with successors of the entry block
    let mut worklist: VecDeque<BlockId> = VecDeque::new();
    if let Some(ref term) = cfg.blocks[cfg.entry].terminator {
        for succ in term.successors() {
            if !worklist.contains(&succ) {
                worklist.push_back(succ);
            }
        }
    }

    while let Some(bid) = worklist.pop_front() {
        // Join incoming states from all predecessors
        let preds = &cfg.preds[bid];
        let new_in = if preds.is_empty() {
            analysis.entry_state()
        } else {
            let mut joined = block_out[preds[0]].clone();
            for &pred in &preds[1..] {
                joined = analysis.join(&joined, &block_out[pred]);
            }
            joined
        };

        // Apply transfer function
        let new_out = analysis.transfer(cfg, bid, &new_in);

        // If output changed, update and enqueue successors
        if new_out != block_out[bid] {
            block_in[bid] = new_in;
            block_out[bid] = new_out;
            if let Some(ref term) = cfg.blocks[bid].terminator {
                for succ in term.successors() {
                    if !worklist.contains(&succ) {
                        worklist.push_back(succ);
                    }
                }
            }
        }
    }

    DataflowResult { block_in, block_out }
}
