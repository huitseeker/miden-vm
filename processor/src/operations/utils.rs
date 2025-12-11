use miden_air::RowIndex;

use super::Felt;
use crate::{ContextId, MemoryError, ONE, OperationError, ZERO};

/// Validates that two 2-word (8-element) memory ranges starting at `src_addr` and `dst_addr`
/// are within u32 bounds and do not overlap in the same cycle.
///
/// Uses half-open intervals: [addr, addr+8). If ranges overlap, returns an IllegalMemoryAccess
/// error pointing at the first destination word that would be written.
#[inline(always)]
pub(crate) fn validate_dual_word_stream_addrs(
    src_addr: Felt,
    dst_addr: Felt,
    ctx: ContextId,
    _clk: RowIndex,
) -> Result<(), OperationError> {
    // Convert to u32 and check end-exclusive bounds
    let src_addr_u64 = src_addr.as_int();
    let dst_addr_u64 = dst_addr.as_int();

    let src_addr_u32 = u32::try_from(src_addr_u64).map_err(|_| {
        OperationError::from(MemoryError::AddressOutOfBounds { addr: src_addr_u64 })
    })?;
    let src_end = src_addr_u32.checked_add(8).ok_or_else(|| {
        OperationError::from(MemoryError::AddressOutOfBounds { addr: src_addr_u64 })
    })?;

    let dst_addr_u32 = u32::try_from(dst_addr_u64).map_err(|_| {
        OperationError::from(MemoryError::AddressOutOfBounds { addr: dst_addr_u64 })
    })?;
    let dst_end = dst_addr_u32.checked_add(8).ok_or_else(|| {
        OperationError::from(MemoryError::AddressOutOfBounds { addr: dst_addr_u64 })
    })?;

    // Check for overlap between [src, src+8) and [dst, dst+8)
    if src_addr_u32 < dst_end && dst_addr_u32 < src_end {
        let dst_word2 = dst_addr_u32 + 4; // safe since dst_end computed above
        // We write dst first, then dst+4. Use the first that overlaps.
        let overlap_first = (dst_addr_u32 >= src_addr_u32) && (dst_addr_u32 < src_end);
        let offending_addr = if overlap_first { dst_addr_u32 } else { dst_word2 };
        return Err(OperationError::from(MemoryError::IllegalMemoryAccess {
            ctx,
            addr: offending_addr,
        }));
    }

    Ok(())
}

/// Asserts that the given value is a binary value (0 or 1).
#[inline(always)]
pub fn assert_binary(value: Felt) -> Result<Felt, OperationError> {
    if value != ZERO && value != ONE {
        Err(OperationError::NotBinaryValueOp { value })
    } else {
        Ok(value)
    }
}
