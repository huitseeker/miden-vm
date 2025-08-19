use alloc::vec::Vec;
use std::ops::Index;

use thiserror::Error;

/// Simplified CSR format sparse matrix, which assumes the non-zero elements are slanted left, that
/// is rows contain an initial prefix in an all-dense representation, followed by an all-zero
/// suffix.
///
/// We follow the names used by scipy.
/// Detailed explanation here: <https://stackoverflow.com/questions/52299420/scipy-csr-matrix-understand-indptr>
///
/// We aim to use this with: cols == GROUP_SIZE and rows == BATCH_SIZE
#[derive(Debug)]
pub struct SparseMatrix<F: Default> {
    /// all non-zero values in the matrix
    pub data: Vec<F>,
    /// row information: indptr contains indexes into `self.data`` such that
    /// row i 's non-zero values are in `self.data[self.indptr[i]..self.indptr[i+1]]`
    //  should be serialized as a [u8; BATCH_SIZE + 1] since the maximal index is 72
    pub indptr: Vec<usize>,
    /// number of columns
    // should be serialized as a u8
    pub cols: usize,
    #[doc(hidden)]
    /// A copy of F::default for the Index trait
    _default: F,
}

#[derive(Error, Debug)]
pub enum SparseMatrixError {
    #[error("column index out of bounds (max {1}): {0}")]
    ColOutOfBounds(usize, usize),
    #[error("cannot comply with column index {1}: would break dense prefix for row {0}")]
    IndexedInsertionError(usize, usize),
    #[error("Cannot insert at row {0}, row is full.")]
    FullRow(usize),
}

impl<F: Default> SparseMatrix<F> {
    /// 0x0 empty matrix
    pub fn empty() -> Self {
        Self {
            data: vec![],
            indptr: vec![0],
            cols: 0,
            _default: F::default(),
        }
    }

    pub fn new(data: Vec<F>, indptr: Vec<usize>, cols: usize) -> Self {
        Self {
            data,
            indptr,
            cols,
            _default: F::default(),
        }
    }

    /// Add a new empty row to the matrix
    fn add_row(&mut self) {
        let current_len = self.data.len();
        self.indptr.push(current_len);
    }

    /// number of non-zero entries
    pub fn len(&self) -> usize {
        *self.indptr.last().unwrap()
    }

    /// empty matrix
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn num_rows(&self) -> usize {
        self.indptr.len() - 1
    }

    pub fn num_cols(&self) -> usize {
        self.cols
    }
}

impl<F: Default + Copy> SparseMatrix<F> {
    /// Get the element at (row, col), returning F::default() if not found
    pub fn get(&self, row: usize, col: usize) -> F {
        let row_start = self.indptr[row];
        let row_end = self.indptr[row + 1];
        let row_dense_len = row_end - row_start;

        if col >= row_dense_len {
            // lookup in the zero suffix
            F::default()
        } else {
            self.data[row_start + col]
        }
    }
}
impl<F: Default + PartialEq + Copy> SparseMatrix<F> {
    /// Set the element at (row, col)
    /// The insertion must respect the invariant that each row has a dense prefix,
    /// otherwise the operation returns an Error.
    /// If successful, returns the old value or None if was zero.
    pub fn set(
        &mut self,
        row: usize,
        col: usize,
        value: F,
    ) -> Result<Option<F>, SparseMatrixError> {
        if col > self.num_cols() {
            return Err(SparseMatrixError::ColOutOfBounds(col, self.num_cols()));
        }

        // Ensure we have enough rows
        while self.num_rows() <= row {
            self.add_row();
        }

        let row_start = self.indptr[row];
        let row_end = self.indptr[row + 1];
        let row_dense_len = row_end - row_start;

        if col == row_dense_len {
            // insertion in the zero suffix
            // Add new element if not zero
            if value != F::default() {
                self.insert_element_at(row, row_end, value);
                Ok(None)
            } else {
                Ok(None)
            }
        } else if col > row_dense_len {
            // Insertion strictly past the dense boundary
            Err(SparseMatrixError::IndexedInsertionError(row, col))
        } else {
            // insertion in the dense prefix
            let abs_pos = row_start + col;
            let old_value = self.data[abs_pos];
            // If setting to zero, remove the element
            if value == F::default() {
                self.remove_element_at(row, abs_pos);
                Ok(Some(old_value))
            } else {
                self.data[abs_pos] = value;
                Ok(Some(old_value))
            }
        }
    }

    /// Insert an element at the dense end of a row
    /// Returns an Error if the row is full. If the insertion succeeds, returns the column index
    /// of the newly inserted element.
    pub fn insert(&mut self, row: usize, value: F) -> Result<usize, SparseMatrixError> {
        // Ensure we have enough rows
        while self.num_rows() <= row {
            self.add_row();
        }

        let row_start = self.indptr[row];
        let row_end = self.indptr[row + 1];
        let row_dense_len = row_end - row_start;

        if row_dense_len == self.num_cols() {
            return Err(SparseMatrixError::FullRow(row));
        }
        // Add new element if not zero
        if value != F::default() {
            self.insert_element_at(row, row_end, value);
        }
        Ok(row_dense_len)
    }

    /// Insert a new element at the specified absolute position
    fn insert_element_at(&mut self, row: usize, abs_pos: usize, value: F) {
        // check we insert in the correct row, or extend it
        debug_assert!(self.indptr[row] <= abs_pos);
        debug_assert!(self.indptr[row + 1] >= abs_pos);

        // Insert into data and indices
        self.data.insert(abs_pos, value);

        // Update indptr for all subsequent rows
        for i in (row + 1)..self.indptr.len() {
            self.indptr[i] += 1;
        }
    }

    /// Remove an element at the specified absolute position
    fn remove_element_at(&mut self, row: usize, abs_pos: usize) {
        // check we remove in the correct row, or shrink it
        debug_assert!(self.indptr[row] <= abs_pos);
        debug_assert!(self.indptr[row + 1] > abs_pos);

        // Remove from data and indices
        self.data.remove(abs_pos);

        // Update indptr for all subsequent rows
        for i in (row + 1)..self.indptr.len() {
            self.indptr[i] -= 1;
        }
    }

    /// returns an iterator over non-zero values
    pub fn nnz_iter(&self) -> NonZeroIter<'_, F> {
        NonZeroIter {
            matrix: self,
            row: 0,
            i: 0,
            nnz: *self.indptr.last().unwrap(),
        }
    }

    /// Iterator over all matrix elements (including zeros) in row-major order
    pub fn iter_dense(&self) -> DenseIter<'_, F> {
        DenseIter::new(self)
    }

    /// Iterator over non-zero elements with (row, col, value)
    pub fn iter_nonzero(&self) -> NonZeroIter<'_, F> {
        self.nnz_iter()
    }
}

/// Iterator for dense matrix elements (including zeros) in row-major order
#[derive(Debug)]
pub struct DenseIter<'a, F: Copy + Default> {
    matrix: &'a SparseMatrix<F>,
    row: usize,
    col: usize,
    row_start: usize,
    row_end: usize,
    current_idx: usize,
}

impl<'a, F: Copy + Default + PartialEq> DenseIter<'a, F> {
    fn new(matrix: &'a SparseMatrix<F>) -> Self {
        let row_start = if matrix.num_rows() > 0 { matrix.indptr[0] } else { 0 };
        let row_end = if matrix.num_rows() > 0 { matrix.indptr[1] } else { 0 };
        DenseIter {
            matrix,
            row: 0,
            col: 0,
            row_start,
            row_end,
            current_idx: row_start,
        }
    }
}

impl<'a, F: Copy + Default> Iterator for DenseIter<'a, F> {
    type Item = F;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.matrix.num_rows() {
            return None;
        }

        let value = if self.current_idx < self.row_end {
            // Found the element we're looking for
            let result = self.matrix.data[self.current_idx];
            self.current_idx += 1;
            self.col += 1;
            result
        } else {
            // No more elements in this row, return default
            self.col += 1;
            F::default()
        };

        // Move to next row if we've finished this row
        if self.col >= self.matrix.num_cols() {
            self.col = 0;
            self.row += 1;

            // Update row pointers for the new row
            if self.row < self.matrix.num_rows() {
                self.row_start = self.matrix.indptr[self.row];
                self.row_end = self.matrix.indptr[self.row + 1];
                self.current_idx = self.row_start;
            }
        }

        Some(value)
    }
}

/// Iterator for sparse matrix (non-zero elements with row/column indices, i.e. C)) format)
#[derive(Debug)]
pub struct NonZeroIter<'a, F: Copy + Default + PartialEq> {
    matrix: &'a SparseMatrix<F>,
    row: usize,
    i: usize,
    nnz: usize,
}

impl<'a, F: Copy + Default + PartialEq> Iterator for NonZeroIter<'a, F> {
    type Item = (usize, usize, F);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.nnz {
            return None;
        }
        let row = self.row;
        let val = self.matrix.data[self.i];
        let row_start = self.matrix.indptr[self.row];
        let col = self.i - row_start;

        self.i += 1;

        // Advance to the next row if we've moved past the current row's data
        while self.row < self.matrix.num_rows() - 1 && self.i >= self.matrix.indptr[self.row + 1] {
            self.row += 1;
        }

        Some((row, col, val))
    }
}

impl<F: Copy + Default + PartialEq> std::ops::Index<usize> for SparseMatrix<F> {
    type Output = F;

    fn index(&self, index: usize) -> &Self::Output {
        // Calculate row and col from the linear index (row-major order)
        let rows = self.num_rows();
        let cols = self.num_cols();

        if rows == 0 || cols == 0 {
            return &self._default;
        }

        let row = index / cols;
        let col = index % cols;

        // Handle case where index is out of bounds
        if row >= rows {
            return &self._default;
        }

        // Get the element at (row, col), returning default if not found
        // Use the same logic as the get method but more efficient for single access
        let row_start = self.indptr[row];
        let row_end = self.indptr[row + 1];

        if col >= row_end - row_start {
            return &self._default;
        }

        &self.data[row_start + col]
    }
}
