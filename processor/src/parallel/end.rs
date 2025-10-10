use core::ops::ControlFlow;

use miden_core::{Felt, Operation, Word};

use super::{CoreTraceFragmentGenerator, trace_builder::OperationTraceConfig};
use crate::fast::trace_state::NodeFlags;

impl CoreTraceFragmentGenerator {
    /// Adds a trace row for the END operation to the main trace fragment.
    ///
    /// This method creates a trace row that corresponds to the END operation that completes
    /// execution of a control flow node. It pops the node's information from the block stack
    /// and uses it to populate the appropriate trace columns.
    pub fn add_end_trace_row(&mut self, node_digest: Word) -> ControlFlow<()> {
        // Pop the block from stack and use its info for END operations
        let (ended_node_addr, flags) = self.update_decoder_state_on_node_end();

        self.add_end_trace_row_impl(node_digest, flags, ended_node_addr)
    }

    /// Implementation of the END trace row generation with explicit parameters.
    ///
    /// This method allows specifying the node digest, flags, and ended address directly,
    /// which is useful for cases where these values are computed separately.
    pub fn add_end_trace_row_impl(
        &mut self,
        node_digest: Word,
        flags: NodeFlags,
        ended_node_addr: Felt,
    ) -> ControlFlow<()> {
        let config = OperationTraceConfig {
            opcode: Operation::End.op_code(),
            hasher_state: (node_digest, flags.to_hasher_state_second_word()),
            addr: ended_node_addr,
        };

        // Reset the span context after completing the basic block
        self.span_context = None;

        self.add_control_flow_trace_row(config)
    }
}