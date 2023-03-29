
## std::crypto::hashes::native
| Procedure | Description |
| ----------- | ------------- |
| hash_memory | Hashes the memory `start_addr` to `end_addr`, handles odd number of elements.<br /><br />This requires `end_addr > start_addr`, otherwise the procedure will enter an infinite loop.<br /><br />`end_addr` is not inclusive.<br /><br />Stack transition:<br /><br />Input: [start_addr, end_addr, ...]<br /><br />Output: [H, ...]<br /><br />Cycles:<br /><br />even words: 44 cycles + 3 * words<br /><br />odd words: 56 cycles + 3 * words |
