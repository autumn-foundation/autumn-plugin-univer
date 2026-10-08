//! [`Workbook`]: typed Univer workbook data (`IWorkbookData`).
//!
//! Use it to seed a [`Spreadsheet`](crate::Spreadsheet) and to read a saved
//! snapshot. The model types the fields that apps use: ids, names, sheet
//! order, sizes and cells. It keeps every other field in `extra`, so a
//! snapshot round-trips with no loss (styles, merges, row heights, …).
//!
//! # Invariants
//!
//! [`Workbook::validate`] holds iff all of these are true:
//!
//! 1. The workbook id is not empty.
//! 2. `sheet_order` has no duplicates.
//! 3. `sheet_order` and the keys of `sheets` hold the same ids.
//! 4. Each sheet's `id` equals its key, and is not empty.
//! 5. Each cell is in bounds: `row < row_count`, `col < column_count`.
//! 6. Each number cell is finite.
//! 7. Sizes are in [`Limits`]: sheets, rows, columns and total cells.
//!
//! [`WorkbookBuilder::build`] returns only valid workbooks.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// Univer's default row count for a new sheet.
const DEFAULT_ROWS: u32 = 1000;

/// Univer's default column count for a new sheet.
const DEFAULT_COLUMNS: u32 = 20;

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Makes an id. [`Workbook::validate`] rejects an empty id.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// The id text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self::new(id)
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id)
            }
        }
    };
}

string_id!(
    /// The id of a [`Workbook`].
    WorkbookId
);
string_id!(
    /// The id of a [`Sheet`] in a workbook.
    SheetId
);

/// A cell position. Both parts start at zero: `A1` is `(0, 0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellRef {
    /// The row index.
    pub row: u32,
    /// The column index.
    pub col: u32,
}

impl CellRef {
    /// Makes a cell position.
    #[must_use]
    pub const fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }
}

impl fmt::Display for CellRef {
    /// Writes A1 notation, for example `B3` for `(2, 1)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut letters = Vec::new();
        let mut n = u64::from(self.col) + 1;
        while n > 0 {
            let rem = u8::try_from((n - 1) % 26).unwrap_or(0);
            letters.push(char::from(b'A' + rem));
            n = (n - 1) / 26;
        }
        letters.iter().rev().try_for_each(|c| write!(f, "{c}"))?;
        write!(f, "{}", u64::from(self.row) + 1)
    }
}

/// A cell value (Univer `v`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CellValue {
    /// A boolean.
    Bool(bool),
    /// A number. It must be finite.
    Number(f64),
    /// Text.
    Text(String),
}

/// The largest integer that an `f64` holds exactly (2⁵³).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;

impl Serialize for CellValue {
    /// Writes an integral number as a JSON integer (`84`, not `84.0`), as
    /// JavaScript does.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Bool(b) => s.serialize_bool(*b),
            Self::Text(t) => s.serialize_str(t),
            #[allow(clippy::cast_possible_truncation)] // Exact: checked below.
            Self::Number(n) if n.fract() == 0.0 && n.abs() <= MAX_SAFE_INTEGER => {
                s.serialize_i64(*n as i64)
            }
            Self::Number(n) => s.serialize_f64(*n),
        }
    }
}

impl From<&str> for CellValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<String> for CellValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<f64> for CellValue {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i32> for CellValue {
    fn from(v: i32) -> Self {
        Self::Number(f64::from(v))
    }
}

impl From<bool> for CellValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

/// The cell type hint (Univer `t`, `CellValueType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    /// `1`: text.
    Text,
    /// `2`: number.
    Number,
    /// `3`: boolean.
    Bool,
    /// `4`: text that looks like a number, kept as text.
    ForceText,
    /// Any other code. Kept for round-trips.
    Other(u8),
}

impl CellType {
    const fn code(self) -> u8 {
        match self {
            Self::Text => 1,
            Self::Number => 2,
            Self::Bool => 3,
            Self::ForceText => 4,
            Self::Other(c) => c,
        }
    }

    const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Text,
            2 => Self::Number,
            3 => Self::Bool,
            4 => Self::ForceText,
            c => Self::Other(c),
        }
    }
}

impl Serialize for CellType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.code())
    }
}

impl<'de> Deserialize<'de> for CellType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u8::deserialize(d).map(Self::from_code)
    }
}

/// One cell (Univer `ICellData`).
///
/// ```rust
/// use autumn_plugin_univer::{Cell, CellValue};
///
/// assert_eq!(Cell::number(2.5).value, Some(CellValue::Number(2.5)));
/// assert_eq!(Cell::formula("=A1*2").formula.as_deref(), Some("=A1*2"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// The value. For a formula cell, the last computed value.
    #[serde(rename = "v", default, skip_serializing_if = "Option::is_none")]
    pub value: Option<CellValue>,
    /// The type hint.
    #[serde(rename = "t", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CellType>,
    /// The formula, for example `=SUM(A1:A3)`.
    #[serde(rename = "f", default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// The style: a style id (key of [`Workbook::styles`]) or an inline
    /// style object.
    #[serde(rename = "s", default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Value>,
    /// Other Univer fields (`p`, `si`, `custom`, …), kept as is.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Cell {
    /// A text cell.
    pub fn text(v: impl Into<String>) -> Self {
        Self {
            value: Some(CellValue::Text(v.into())),
            kind: Some(CellType::Text),
            ..Self::default()
        }
    }

    /// A number cell. [`Workbook::validate`] rejects a non-finite number.
    #[must_use]
    pub fn number(v: f64) -> Self {
        Self {
            value: Some(CellValue::Number(v)),
            kind: Some(CellType::Number),
            ..Self::default()
        }
    }

    /// A boolean cell.
    #[must_use]
    pub fn bool(v: bool) -> Self {
        Self {
            value: Some(CellValue::Bool(v)),
            kind: Some(CellType::Bool),
            ..Self::default()
        }
    }

    /// A formula cell. Univer computes the value in the browser.
    pub fn formula(f: impl Into<String>) -> Self {
        Self {
            formula: Some(f.into()),
            ..Self::default()
        }
    }

    /// Sets the style id (a key of [`Workbook::styles`]).
    #[must_use]
    pub fn with_style(mut self, style_id: impl Into<String>) -> Self {
        self.style = Some(Value::String(style_id.into()));
        self
    }
}

impl From<CellValue> for Cell {
    fn from(v: CellValue) -> Self {
        match v {
            CellValue::Text(t) => Self::text(t),
            CellValue::Number(n) => Self::number(n),
            CellValue::Bool(b) => Self::bool(b),
        }
    }
}

/// Cells by row, then by column (Univer `cellData`).
pub(crate) type CellMatrix = BTreeMap<u32, BTreeMap<u32, Cell>>;

/// (De)serializes `cellData`. JSON object keys are strings; this module
/// parses them as indexes. (`serde(flatten)` on [`Sheet`] blocks the
/// built-in integer-key support.)
mod cell_matrix {
    use super::{BTreeMap, Cell, CellMatrix};
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(cells: &CellMatrix, s: S) -> Result<S::Ok, S::Error> {
        let as_text: BTreeMap<String, BTreeMap<String, &Cell>> = cells
            .iter()
            .map(|(r, row)| {
                (
                    r.to_string(),
                    row.iter().map(|(c, cell)| (c.to_string(), cell)).collect(),
                )
            })
            .collect();
        as_text.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<CellMatrix, D::Error> {
        let raw =
            Option::<BTreeMap<String, Option<BTreeMap<String, Option<Cell>>>>>::deserialize(d)?;
        let mut out = CellMatrix::new();
        for (r, row) in raw.unwrap_or_default() {
            let r: u32 = r
                .parse()
                .map_err(|_| D::Error::custom(format!("bad row index `{r}`")))?;
            let cells = out.entry(r).or_default();
            for (c, cell) in row.unwrap_or_default() {
                let c: u32 = c
                    .parse()
                    .map_err(|_| D::Error::custom(format!("bad column index `{c}`")))?;
                if let Some(cell) = cell {
                    cells.insert(c, cell);
                }
            }
            if cells.is_empty() {
                out.remove(&r);
            }
        }
        Ok(out)
    }
}

const fn default_rows() -> u32 {
    DEFAULT_ROWS
}

const fn default_columns() -> u32 {
    DEFAULT_COLUMNS
}

/// One sheet (Univer `IWorksheetData`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sheet {
    /// The sheet id. It equals its key in [`Workbook::sheets`].
    pub id: SheetId,
    /// The tab name.
    #[serde(default)]
    pub name: String,
    /// The number of rows. Univer's default is 1000.
    #[serde(default = "default_rows")]
    pub row_count: u32,
    /// The number of columns. Univer's default is 20.
    #[serde(default = "default_columns")]
    pub column_count: u32,
    #[serde(rename = "cellData", default, with = "cell_matrix")]
    cells: CellMatrix,
    /// Other Univer fields (`mergeData`, `rowData`, `freeze`, …), kept as is.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Sheet {
    /// Makes an empty sheet with Univer's default size (1000 × 20).
    pub fn new(id: impl Into<SheetId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            row_count: DEFAULT_ROWS,
            column_count: DEFAULT_COLUMNS,
            cells: CellMatrix::new(),
            extra: Map::new(),
        }
    }

    /// Makes a sheet from rows of cells, from `A1` down.
    ///
    /// ```rust
    /// use autumn_plugin_univer::{Cell, Sheet};
    ///
    /// let sheet = Sheet::from_rows("s1", "People", [
    ///     vec![Cell::text("Name"), Cell::text("Age")],
    ///     vec![Cell::text("Ada"), Cell::number(36.0)],
    /// ]);
    /// assert_eq!(sheet.cell(1, 1), Some(&Cell::number(36.0)));
    /// ```
    pub fn from_rows<R, C>(id: impl Into<SheetId>, name: impl Into<String>, rows: R) -> Self
    where
        R: IntoIterator<Item = C>,
        C: IntoIterator<Item = Cell>,
    {
        rows.into_iter()
            .zip(0_u32..)
            .fold(Self::new(id, name), |sheet, (cells, r)| {
                sheet.with_row(r, cells)
            })
    }

    /// Sets one cell. The sheet grows to hold it.
    #[must_use]
    pub fn with_cell(mut self, row: u32, col: u32, cell: impl Into<Cell>) -> Self {
        self.set_cell(row, col, cell);
        self
    }

    /// Sets cells in one row, from column `A`. The sheet grows to hold them.
    #[must_use]
    pub fn with_row(mut self, row: u32, cells: impl IntoIterator<Item = Cell>) -> Self {
        for (cell, col) in cells.into_iter().zip(0_u32..) {
            self.set_cell(row, col, cell);
        }
        self
    }

    /// Sets the size. Cells outside the new size make the workbook invalid.
    #[must_use]
    pub const fn with_size(mut self, row_count: u32, column_count: u32) -> Self {
        self.row_count = row_count;
        self.column_count = column_count;
        self
    }

    /// Sets one cell. The sheet grows to hold it.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: impl Into<Cell>) {
        self.row_count = self.row_count.max(row.saturating_add(1));
        self.column_count = self.column_count.max(col.saturating_add(1));
        self.cells.entry(row).or_default().insert(col, cell.into());
    }

    /// Removes one cell and returns it.
    pub fn remove_cell(&mut self, row: u32, col: u32) -> Option<Cell> {
        let cells = self.cells.get_mut(&row)?;
        let cell = cells.remove(&col);
        if cells.is_empty() {
            self.cells.remove(&row);
        }
        cell
    }

    /// The cell at `(row, col)`, if set.
    #[must_use]
    pub fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(&row)?.get(&col)
    }

    /// The value at `(row, col)`, if set.
    #[must_use]
    pub fn value(&self, row: u32, col: u32) -> Option<&CellValue> {
        self.cell(row, col)?.value.as_ref()
    }

    /// Every set cell, in row-major order.
    pub fn cells(&self) -> impl Iterator<Item = (CellRef, &Cell)> {
        self.cells
            .iter()
            .flat_map(|(&r, row)| row.iter().map(move |(&c, cell)| (CellRef::new(r, c), cell)))
    }

    /// The number of set cells.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.cells.values().map(BTreeMap::len).sum()
    }

    /// The values as dense rows, from `A1` to the last set cell.
    ///
    /// ```rust
    /// use autumn_plugin_univer::{Cell, CellValue, Sheet};
    ///
    /// let sheet = Sheet::new("s1", "S").with_cell(1, 1, Cell::number(5.0));
    /// assert_eq!(sheet.to_rows(), vec![
    ///     vec![None, None],
    ///     vec![None, Some(CellValue::Number(5.0))],
    /// ]);
    /// ```
    #[must_use]
    pub fn to_rows(&self) -> Vec<Vec<Option<CellValue>>> {
        let Some(&last_row) = self.cells.keys().next_back() else {
            return Vec::new();
        };
        let width = self
            .cells
            .values()
            .filter_map(|row| row.keys().next_back())
            .max()
            .map_or(0, |&c| c as usize + 1);
        (0..=last_row)
            .map(|r| {
                (0..width)
                    .map(|c| {
                        u32::try_from(c)
                            .ok()
                            .and_then(|c| self.value(r, c))
                            .cloned()
                    })
                    .collect()
            })
            .collect()
    }
}

impl Sheet {
    /// Checks invariants 4 to 7 for the sheet stored under `key`.
    fn validate(&self, key: &SheetId, limits: &Limits) -> Result<(), WorkbookError> {
        if key.as_str().is_empty() || self.id.as_str().is_empty() {
            return Err(WorkbookError::EmptySheetId);
        }
        if &self.id != key {
            return Err(WorkbookError::SheetIdMismatch {
                key: key.clone(),
                id: self.id.clone(),
            });
        }
        if self.row_count > limits.max_rows {
            return Err(WorkbookError::TooManyRows {
                sheet: key.clone(),
                count: self.row_count,
                max: limits.max_rows,
            });
        }
        if self.column_count > limits.max_columns {
            return Err(WorkbookError::TooManyColumns {
                sheet: key.clone(),
                count: self.column_count,
                max: limits.max_columns,
            });
        }
        for (at, cell) in self.cells() {
            if at.row >= self.row_count || at.col >= self.column_count {
                return Err(WorkbookError::CellOutOfBounds {
                    sheet: key.clone(),
                    cell: at,
                });
            }
            if matches!(cell.value, Some(CellValue::Number(n)) if !n.is_finite()) {
                return Err(WorkbookError::NonFiniteNumber {
                    sheet: key.clone(),
                    cell: at,
                });
            }
        }
        Ok(())
    }
}

/// Size limits for [`Workbook::validate_with`].
///
/// The defaults match the Excel grid (1 048 576 × 16 384) and cap the work
/// that one request can cause: 200 sheets and 1 000 000 set cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The maximum number of sheets.
    pub max_sheets: usize,
    /// The maximum `row_count` of a sheet.
    pub max_rows: u32,
    /// The maximum `column_count` of a sheet.
    pub max_columns: u32,
    /// The maximum number of set cells in all sheets.
    pub max_cells: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_sheets: 200,
            max_rows: 1_048_576,
            max_columns: 16_384,
            max_cells: 1_000_000,
        }
    }
}

/// Why a workbook is not valid. See the module docs for the invariants.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum WorkbookError {
    /// Invariant 1.
    #[error("the workbook id is empty")]
    EmptyWorkbookId,
    /// Invariant 4.
    #[error("a sheet id is empty")]
    EmptySheetId,
    /// Invariant 2.
    #[error("sheet `{0}` is in the sheet order more than once")]
    DuplicateSheet(SheetId),
    /// Invariant 3: in the order, not in the sheets.
    #[error("sheet `{0}` is in the sheet order but has no data")]
    MissingSheet(SheetId),
    /// Invariant 3: in the sheets, not in the order.
    #[error("sheet `{0}` has data but is not in the sheet order")]
    UnorderedSheet(SheetId),
    /// Invariant 4.
    #[error("sheet key `{key}` holds a sheet with id `{id}`")]
    SheetIdMismatch {
        /// The key in `sheets`.
        key: SheetId,
        /// The `id` field of the sheet.
        id: SheetId,
    },
    /// Invariant 5.
    #[error("cell {cell} of sheet `{sheet}` is outside the sheet size")]
    CellOutOfBounds {
        /// The sheet.
        sheet: SheetId,
        /// The cell.
        cell: CellRef,
    },
    /// Invariant 6.
    #[error("cell {cell} of sheet `{sheet}` holds a number that is not finite")]
    NonFiniteNumber {
        /// The sheet.
        sheet: SheetId,
        /// The cell.
        cell: CellRef,
    },
    /// Invariant 7.
    #[error("the workbook has {count} sheets; the limit is {max}")]
    TooManySheets {
        /// The sheet count.
        count: usize,
        /// The limit.
        max: usize,
    },
    /// Invariant 7.
    #[error("sheet `{sheet}` has {count} rows; the limit is {max}")]
    TooManyRows {
        /// The sheet.
        sheet: SheetId,
        /// The row count.
        count: u32,
        /// The limit.
        max: u32,
    },
    /// Invariant 7.
    #[error("sheet `{sheet}` has {count} columns; the limit is {max}")]
    TooManyColumns {
        /// The sheet.
        sheet: SheetId,
        /// The column count.
        count: u32,
        /// The limit.
        max: u32,
    },
    /// Invariant 7.
    #[error("the workbook has more than {max} set cells")]
    TooManyCells {
        /// The limit.
        max: usize,
    },
}

/// A workbook (Univer `IWorkbookData`).
///
/// Build one with [`Workbook::builder`], or read one from JSON with
/// [`WorkbookSnapshot`](crate::WorkbookSnapshot) or `serde_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workbook {
    /// The workbook id.
    pub id: WorkbookId,
    /// The workbook name.
    #[serde(default)]
    pub name: String,
    /// The sheet ids, in tab order.
    #[serde(default)]
    pub sheet_order: Vec<SheetId>,
    /// The sheets, by id.
    #[serde(default)]
    pub sheets: BTreeMap<SheetId, Sheet>,
    /// Named styles. [`Cell::style`] can refer to a key.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub styles: Map<String, Value>,
    /// Other Univer fields (`appVersion`, `locale`, `resources`, …), kept as
    /// is.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Workbook {
    /// Starts a builder. [`WorkbookBuilder::build`] validates the result.
    pub fn builder(id: impl Into<WorkbookId>, name: impl Into<String>) -> WorkbookBuilder {
        WorkbookBuilder {
            workbook: Self {
                id: id.into(),
                name: name.into(),
                sheet_order: Vec::new(),
                sheets: BTreeMap::new(),
                styles: Map::new(),
                extra: Map::new(),
            },
        }
    }

    /// A workbook with no sheets. Univer adds one empty sheet on load.
    pub fn empty(id: impl Into<WorkbookId>) -> Self {
        Self::builder(id, "").workbook
    }

    /// The sheet with this id.
    #[must_use]
    pub fn sheet(&self, id: &str) -> Option<&Sheet> {
        self.sheets.get(&SheetId::new(id))
    }

    /// The sheet with this id, for changes.
    pub fn sheet_mut(&mut self, id: &str) -> Option<&mut Sheet> {
        self.sheets.get_mut(&SheetId::new(id))
    }

    /// The sheets in tab order. Ids with no sheet are skipped.
    pub fn sheets_in_order(&self) -> impl Iterator<Item = &Sheet> {
        self.sheet_order.iter().filter_map(|id| self.sheets.get(id))
    }

    /// The first sheet in tab order.
    #[must_use]
    pub fn first_sheet(&self) -> Option<&Sheet> {
        self.sheets_in_order().next()
    }

    /// Checks the invariants with the default [`Limits`].
    ///
    /// # Errors
    ///
    /// Returns the first broken invariant.
    pub fn validate(&self) -> Result<(), WorkbookError> {
        self.validate_with(&Limits::default())
    }

    /// Checks the invariants with these limits.
    ///
    /// # Errors
    ///
    /// Returns the first broken invariant.
    pub fn validate_with(&self, limits: &Limits) -> Result<(), WorkbookError> {
        if self.id.as_str().is_empty() {
            return Err(WorkbookError::EmptyWorkbookId);
        }
        if self.sheets.len() > limits.max_sheets {
            return Err(WorkbookError::TooManySheets {
                count: self.sheets.len(),
                max: limits.max_sheets,
            });
        }
        // Invariants 2 and 3: the order is a permutation of the keys.
        let mut seen = std::collections::BTreeSet::new();
        for id in &self.sheet_order {
            if !seen.insert(id) {
                return Err(WorkbookError::DuplicateSheet(id.clone()));
            }
            if !self.sheets.contains_key(id) {
                return Err(WorkbookError::MissingSheet(id.clone()));
            }
        }
        if let Some(id) = self.sheets.keys().find(|id| !seen.contains(id)) {
            return Err(WorkbookError::UnorderedSheet(id.clone()));
        }
        let mut total = 0_usize;
        for (key, sheet) in &self.sheets {
            sheet.validate(key, limits)?;
            total = total.saturating_add(sheet.cell_count());
            if total > limits.max_cells {
                return Err(WorkbookError::TooManyCells {
                    max: limits.max_cells,
                });
            }
        }
        Ok(())
    }

    /// The workbook as JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        // Cannot fail: every map key is a string and every value is JSON.
        // (`serde_json` writes a non-finite number as `null`.)
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }
}

/// Builds a valid [`Workbook`]. See [`Workbook::builder`].
///
/// ```rust
/// use autumn_plugin_univer::{Cell, Sheet, Workbook};
///
/// let workbook = Workbook::builder("wb", "Report")
///     .sheet(Sheet::new("s1", "Data").with_cell(0, 0, Cell::text("Hi")))
///     .style("bold", serde_json::json!({ "bl": 1 }))
///     .build()
///     .expect("valid");
/// assert_eq!(workbook.first_sheet().map(|s| s.name.as_str()), Some("Data"));
/// ```
#[derive(Debug, Clone)]
#[must_use]
pub struct WorkbookBuilder {
    workbook: Workbook,
}

impl WorkbookBuilder {
    /// Adds a sheet at the end of the tab order. A sheet with the same id
    /// replaces the old one and keeps its position.
    pub fn sheet(mut self, sheet: Sheet) -> Self {
        if !self.workbook.sheet_order.contains(&sheet.id) {
            self.workbook.sheet_order.push(sheet.id.clone());
        }
        self.workbook.sheets.insert(sheet.id.clone(), sheet);
        self
    }

    /// Adds a named style (Univer `IStyleData`).
    pub fn style(mut self, id: impl Into<String>, style: Value) -> Self {
        self.workbook.styles.insert(id.into(), style);
        self
    }

    /// Sets an other top-level Univer field, for example `locale`.
    pub fn extra(mut self, key: impl Into<String>, value: Value) -> Self {
        self.workbook.extra.insert(key.into(), value);
        self
    }

    /// Validates with the default [`Limits`] and returns the workbook.
    ///
    /// # Errors
    ///
    /// Returns the first broken invariant.
    pub fn build(self) -> Result<Workbook, WorkbookError> {
        self.build_with(&Limits::default())
    }

    /// Validates with these limits and returns the workbook.
    ///
    /// # Errors
    ///
    /// Returns the first broken invariant.
    pub fn build_with(self, limits: &Limits) -> Result<Workbook, WorkbookError> {
        self.workbook.validate_with(limits)?;
        Ok(self.workbook)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    /// A snapshot that Univer 1.0.3 wrote (`FWorkbook.save()`), trimmed.
    const UNIVER_SNAPSHOT: &str = r#"{
        "id": "wb0",
        "sheetOrder": ["s1", "s2"],
        "name": "Budget",
        "appVersion": "1.0.3",
        "locale": "enUS",
        "styles": { "Ab12": { "bl": 1, "fs": 14 } },
        "sheets": {
            "s1": {
                "id": "s1", "name": "Sheet A", "tabColor": "", "hidden": 0,
                "freeze": { "xSplit": 0, "ySplit": 1, "startRow": 1, "startColumn": -1 },
                "rowCount": 20, "columnCount": 5, "zoomRatio": 1,
                "scrollTop": 0, "scrollLeft": 0,
                "defaultColumnWidth": 88, "defaultRowHeight": 24,
                "mergeData": [{ "startRow": 3, "endRow": 3, "startColumn": 0, "endColumn": 1 }],
                "cellData": {
                    "0": { "0": { "v": 42, "t": 2 }, "1": { "f": "=A1*2", "v": 84, "t": 2, "si": "x1" } },
                    "1": { "0": { "v": "hello", "t": 1, "s": "Ab12" } },
                    "2": { "4": { "v": true, "t": 3 } },
                    "5": {}
                },
                "rowData": { "0": { "h": 30, "ah": 30 } },
                "columnData": {},
                "showGridlines": 1, "rightToLeft": 0
            },
            "s2": { "id": "s2", "name": "Empty", "cellData": {} }
        },
        "resources": [{ "name": "SHEET_DEFINED_NAME_PLUGIN", "data": "" }]
    }"#;

    fn snapshot() -> Workbook {
        serde_json::from_str(UNIVER_SNAPSHOT).expect("parses")
    }

    fn sample() -> Workbook {
        Workbook::builder("wb", "W")
            .sheet(Sheet::new("a", "A").with_cell(0, 0, Cell::text("x")))
            .sheet(Sheet::new("b", "B"))
            .build()
            .expect("valid")
    }

    // ---- Serde -----------------------------------------------------------

    #[test]
    fn reads_a_univer_snapshot() {
        let wb = snapshot();
        assert_eq!(wb.id.as_str(), "wb0");
        assert_eq!(wb.name, "Budget");
        assert_eq!(wb.sheet_order, [SheetId::new("s1"), SheetId::new("s2")]);
        let s1 = wb.sheet("s1").expect("s1");
        assert_eq!((s1.row_count, s1.column_count), (20, 5));
        assert_eq!(s1.value(0, 0), Some(&CellValue::Number(42.0)));
        assert_eq!(
            s1.cell(0, 1).and_then(|c| c.formula.as_deref()),
            Some("=A1*2")
        );
        assert_eq!(s1.value(1, 0), Some(&CellValue::Text("hello".into())));
        assert_eq!(
            s1.cell(1, 0).and_then(|c| c.style.clone()),
            Some(json!("Ab12"))
        );
        assert_eq!(s1.value(2, 4), Some(&CellValue::Bool(true)));
        assert_eq!(s1.cell(0, 0).and_then(|c| c.kind), Some(CellType::Number));
        assert_eq!(s1.cell_count(), 4, "empty rows are dropped");
        assert!(wb.validate().is_ok());
    }

    #[test]
    fn missing_sizes_take_univer_defaults() {
        let s2 = snapshot().sheets.remove(&SheetId::new("s2")).expect("s2");
        assert_eq!(
            (s2.row_count, s2.column_count),
            (DEFAULT_ROWS, DEFAULT_COLUMNS)
        );
    }

    #[test]
    fn round_trip_keeps_every_field() {
        let original: Value = serde_json::from_str(UNIVER_SNAPSHOT).expect("json");
        let back: Value = serde_json::from_str(&snapshot().to_json()).expect("json");
        // Unknown fields survive.
        assert_eq!(back["appVersion"], original["appVersion"]);
        assert_eq!(back["resources"], original["resources"]);
        assert_eq!(back["styles"], original["styles"]);
        assert_eq!(
            back["sheets"]["s1"]["mergeData"],
            original["sheets"]["s1"]["mergeData"]
        );
        assert_eq!(
            back["sheets"]["s1"]["freeze"],
            original["sheets"]["s1"]["freeze"]
        );
        assert_eq!(
            back["sheets"]["s1"]["rowData"],
            original["sheets"]["s1"]["rowData"]
        );
        assert_eq!(
            back["sheets"]["s1"]["cellData"]["0"]["1"],
            original["sheets"]["s1"]["cellData"]["0"]["1"],
            "cell extras (si) survive"
        );
        // And a second trip is a fixed point.
        let again: Workbook = serde_json::from_value(back).expect("parses");
        assert_eq!(again, snapshot());
    }

    #[test]
    fn null_cells_and_rows_are_skipped() {
        let wb: Workbook = serde_json::from_value(json!({
            "id": "w", "sheetOrder": ["s"],
            "sheets": { "s": { "id": "s", "cellData": { "0": { "0": null, "1": { "v": 1 } }, "1": null } } }
        }))
        .expect("parses");
        let s = wb.sheet("s").expect("s");
        assert_eq!(s.cell_count(), 1);
        assert_eq!(s.value(0, 1), Some(&CellValue::Number(1.0)));
    }

    #[test]
    fn bad_cell_indexes_are_errors() {
        for bad in [
            json!({ "x": { "0": { "v": 1 } } }),
            json!({ "0": { "-1": { "v": 1 } } }),
        ] {
            let err = serde_json::from_value::<Workbook>(json!({
                "id": "w", "sheetOrder": ["s"],
                "sheets": { "s": { "id": "s", "cellData": bad } }
            }))
            .expect_err("rejected");
            assert!(err.to_string().contains("index"), "{err}");
        }
    }

    #[test]
    fn cell_type_codes_round_trip() {
        for code in 0_u8..=6 {
            let t = CellType::from_code(code);
            assert_eq!(t.code(), code);
            let json = serde_json::to_string(&t).expect("ser");
            assert_eq!(serde_json::from_str::<CellType>(&json).expect("de"), t);
        }
    }

    #[test]
    fn workbook_json_omits_empty_styles() {
        let json: Value = serde_json::from_str(&sample().to_json()).expect("json");
        assert!(json.get("styles").is_none());
        assert_eq!(json["sheetOrder"], json!(["a", "b"]));
        assert_eq!(
            json["sheets"]["a"]["cellData"]["0"]["0"],
            json!({ "v": "x", "t": 1 })
        );
    }

    // ---- Builder ---------------------------------------------------------

    #[test]
    fn builder_keeps_tab_order_and_replaces_same_ids() {
        let wb = Workbook::builder("wb", "W")
            .sheet(Sheet::new("a", "First"))
            .sheet(Sheet::new("b", "B"))
            .sheet(Sheet::new("a", "Again"))
            .build()
            .expect("valid");
        assert_eq!(wb.sheet_order, [SheetId::new("a"), SheetId::new("b")]);
        assert_eq!(wb.first_sheet().map(|s| s.name.as_str()), Some("Again"));
        let names: Vec<&str> = wb.sheets_in_order().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Again", "B"]);
    }

    #[test]
    fn builder_rejects_an_empty_id() {
        assert_eq!(
            Workbook::builder("", "W").build(),
            Err(WorkbookError::EmptyWorkbookId)
        );
    }

    #[test]
    fn builder_sets_styles_and_extras() {
        let wb = Workbook::builder("wb", "W")
            .style("bold", json!({ "bl": 1 }))
            .extra("locale", json!("frFR"))
            .build()
            .expect("valid");
        assert_eq!(wb.styles["bold"], json!({ "bl": 1 }));
        let json: Value = serde_json::from_str(&wb.to_json()).expect("json");
        assert_eq!(json["locale"], "frFR");
    }

    #[test]
    fn sheets_grow_to_hold_their_cells() {
        let s = Sheet::new("s", "S").with_cell(2000, 30, Cell::bool(false));
        assert_eq!((s.row_count, s.column_count), (2001, 31));
        let edge = Sheet::new("s", "S").with_cell(u32::MAX, u32::MAX, Cell::number(1.0));
        assert_eq!((edge.row_count, edge.column_count), (u32::MAX, u32::MAX));
    }

    #[test]
    fn rows_fill_from_column_a() {
        let s = Sheet::from_rows(
            "s",
            "S",
            [
                vec![Cell::text("a"), Cell::text("b")],
                vec![],
                vec![Cell::number(3.0)],
            ],
        );
        assert_eq!(
            s.to_rows(),
            vec![
                vec![Some("a".into()), Some("b".into())],
                vec![None, None],
                vec![Some(CellValue::Number(3.0)), None],
            ]
        );
        assert!(Sheet::new("e", "E").to_rows().is_empty());
    }

    #[test]
    fn cells_can_change_and_go() {
        let mut wb = sample();
        let sheet = wb.sheet_mut("a").expect("a");
        sheet.set_cell(1, 1, Cell::formula("=A1"));
        assert_eq!(sheet.remove_cell(0, 0), Some(Cell::text("x")));
        assert_eq!(sheet.remove_cell(0, 0), None);
        assert_eq!(sheet.remove_cell(9, 9), None);
        let cells: Vec<CellRef> = sheet.cells().map(|(r, _)| r).collect();
        assert_eq!(cells, [CellRef::new(1, 1)]);
        assert!(wb.sheet_mut("zzz").is_none());
    }

    #[test]
    fn cell_constructors_set_value_and_type() {
        assert_eq!(Cell::from(CellValue::from("t")), Cell::text("t"));
        assert_eq!(Cell::from(CellValue::from(2)), Cell::number(2.0));
        assert_eq!(Cell::from(CellValue::from(2.5)), Cell::number(2.5));
        assert_eq!(Cell::from(CellValue::from(true)), Cell::bool(true));
        assert_eq!(
            Cell::from(CellValue::from(String::from("s"))),
            Cell::text("s")
        );
        assert_eq!(Cell::text("x").with_style("b").style, Some(json!("b")));
    }

    #[test]
    fn cell_refs_print_in_a1_notation() {
        for (r, c, a1) in [
            (0, 0, "A1"),
            (2, 1, "B3"),
            (0, 25, "Z1"),
            (0, 26, "AA1"),
            (9, 701, "ZZ10"),
            (0, 702, "AAA1"),
        ] {
            assert_eq!(CellRef::new(r, c).to_string(), a1);
        }
    }

    #[test]
    fn ids_convert_and_print() {
        let id = SheetId::from(String::from("s"));
        assert_eq!(id.to_string(), "s");
        assert_eq!(WorkbookId::from("w").as_str(), "w");
        assert_eq!(Workbook::empty("w").sheets.len(), 0);
    }

    // ---- Validation ------------------------------------------------------

    fn invalid(mutate: impl FnOnce(&mut Workbook)) -> WorkbookError {
        let mut wb = sample();
        mutate(&mut wb);
        wb.validate().expect_err("invalid")
    }

    #[test]
    fn rejects_duplicate_order_entries() {
        let err = invalid(|wb| wb.sheet_order.push(SheetId::new("a")));
        assert_eq!(err, WorkbookError::DuplicateSheet(SheetId::new("a")));
    }

    #[test]
    fn rejects_ordered_ids_with_no_sheet() {
        let err = invalid(|wb| wb.sheet_order.push(SheetId::new("zzz")));
        assert_eq!(err, WorkbookError::MissingSheet(SheetId::new("zzz")));
    }

    #[test]
    fn rejects_sheets_missing_from_the_order() {
        let err = invalid(|wb| wb.sheet_order.retain(|id| id.as_str() != "b"));
        assert_eq!(err, WorkbookError::UnorderedSheet(SheetId::new("b")));
    }

    #[test]
    fn rejects_mismatched_sheet_ids() {
        let err = invalid(|wb| {
            wb.sheets.get_mut(&SheetId::new("b")).expect("b").id = SheetId::new("c");
        });
        assert_eq!(
            err,
            WorkbookError::SheetIdMismatch {
                key: SheetId::new("b"),
                id: SheetId::new("c")
            }
        );
    }

    #[test]
    fn rejects_empty_sheet_ids() {
        let err = invalid(|wb| {
            wb.sheet_order.push(SheetId::new(""));
            wb.sheets.insert(SheetId::new(""), Sheet::new("", "E"));
        });
        assert_eq!(err, WorkbookError::EmptySheetId);
    }

    #[test]
    fn rejects_cells_out_of_bounds() {
        let err = invalid(|wb| {
            let s = wb.sheet_mut("a").expect("a");
            s.set_cell(5, 5, Cell::text("x"));
            *s = s.clone().with_size(5, 6);
        });
        assert_eq!(
            err,
            WorkbookError::CellOutOfBounds {
                sheet: SheetId::new("a"),
                cell: CellRef::new(5, 5)
            }
        );
        let err = invalid(|wb| {
            let s = wb.sheet_mut("a").expect("a");
            *s = s.clone().with_size(1, 0);
        });
        assert!(
            matches!(err, WorkbookError::CellOutOfBounds { .. }),
            "{err}"
        );
    }

    #[test]
    fn rejects_non_finite_numbers() {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = invalid(|wb| {
                wb.sheet_mut("a")
                    .expect("a")
                    .set_cell(0, 1, Cell::number(n));
            });
            assert_eq!(
                err,
                WorkbookError::NonFiniteNumber {
                    sheet: SheetId::new("a"),
                    cell: CellRef::new(0, 1)
                }
            );
        }
    }

    #[test]
    fn enforces_limits() {
        let wb = sample();
        let tight = |f: fn(&mut Limits)| {
            let mut l = Limits::default();
            f(&mut l);
            wb.validate_with(&l).expect_err("over the limit")
        };
        assert_eq!(
            tight(|l| l.max_sheets = 1),
            WorkbookError::TooManySheets { count: 2, max: 1 }
        );
        assert_eq!(
            tight(|l| l.max_rows = 10),
            WorkbookError::TooManyRows {
                sheet: SheetId::new("a"),
                count: 1000,
                max: 10
            }
        );
        assert_eq!(
            tight(|l| l.max_columns = 10),
            WorkbookError::TooManyColumns {
                sheet: SheetId::new("a"),
                count: 20,
                max: 10
            }
        );
        assert_eq!(
            tight(|l| l.max_cells = 0),
            WorkbookError::TooManyCells { max: 0 }
        );
        assert!(
            wb.validate_with(&Limits {
                max_cells: 1,
                ..Limits::default()
            })
            .is_ok()
        );
    }

    #[test]
    fn builder_applies_custom_limits() {
        let err = Workbook::builder("w", "W")
            .sheet(Sheet::new("a", "A"))
            .build_with(&Limits {
                max_sheets: 0,
                ..Limits::default()
            })
            .expect_err("over");
        assert_eq!(err, WorkbookError::TooManySheets { count: 1, max: 0 });
    }

    #[test]
    fn errors_explain_themselves() {
        let e = WorkbookError::CellOutOfBounds {
            sheet: SheetId::new("s"),
            cell: CellRef::new(0, 2),
        };
        assert_eq!(
            e.to_string(),
            "cell C1 of sheet `s` is outside the sheet size"
        );
    }

    // ---- Properties ------------------------------------------------------

    fn arb_cell() -> impl Strategy<Value = Cell> {
        prop_oneof![
            any::<String>().prop_map(Cell::text),
            (-1e12_f64..1e12).prop_map(Cell::number),
            any::<bool>().prop_map(Cell::bool),
            "=[A-Z]{1,2}[1-9]".prop_map(Cell::formula),
        ]
    }

    fn arb_sheet(id: String) -> impl Strategy<Value = Sheet> {
        prop::collection::vec((0_u32..50, 0_u32..30, arb_cell()), 0..20).prop_map(move |cells| {
            cells
                .into_iter()
                .fold(Sheet::new(id.clone(), id.clone()), |s, (r, c, cell)| {
                    s.with_cell(r, c, cell)
                })
        })
    }

    fn arb_workbook() -> impl Strategy<Value = Workbook> {
        prop::collection::btree_set("[a-z]{1,6}", 0..5).prop_flat_map(|ids| {
            let sheets: Vec<_> = ids.into_iter().map(arb_sheet).collect();
            sheets.prop_map(|sheets| {
                sheets
                    .into_iter()
                    .fold(Workbook::builder("wb", "W"), WorkbookBuilder::sheet)
                    .build()
                    .expect("builder output is valid")
            })
        })
    }

    proptest! {
        /// Post(build) = Ok(w) ⇒ valid(w), and JSON round-trips are exact.
        #[test]
        fn built_workbooks_are_valid_and_round_trip(wb in arb_workbook()) {
            prop_assert!(wb.validate().is_ok());
            let back: Workbook = serde_json::from_str(&wb.to_json()).expect("parses");
            prop_assert_eq!(&back, &wb);
            prop_assert!(back.validate().is_ok());
        }

        /// Shrinking a sheet below a set cell breaks invariant 5.
        #[test]
        fn shrinking_below_a_cell_is_invalid(wb in arb_workbook()) {
            for sheet in wb.sheets.values() {
                if let Some((at, _)) = sheet.cells().last() {
                    let mut broken = wb.clone();
                    let s = broken.sheets.get_mut(&sheet.id).expect("sheet");
                    s.row_count = at.row;
                    let is_bounds_error = matches!(
                        broken.validate(),
                        Err(WorkbookError::CellOutOfBounds { .. })
                    );
                    prop_assert!(is_bounds_error);
                }
            }
        }

        /// Any order that is not a permutation of the keys is invalid.
        #[test]
        fn order_must_be_a_permutation(wb in arb_workbook(), extra in "[A-Z]{1,3}") {
            let mut broken = wb.clone();
            broken.sheet_order.push(SheetId::new(extra));
            prop_assert!(broken.validate().is_err());
            if let Some(first) = wb.sheet_order.first() {
                let mut dup = wb.clone();
                dup.sheet_order.push(first.clone());
                prop_assert!(dup.validate().is_err());
                let mut short = wb.clone();
                short.sheet_order.remove(0);
                prop_assert!(short.validate().is_err());
            }
        }

        /// Arbitrary JSON never panics the reader or the validator.
        #[test]
        fn arbitrary_json_never_panics(s in ".{0,200}") {
            if let Ok(wb) = serde_json::from_str::<Workbook>(&s) {
                let _ = wb.validate();
            }
        }
    }
}
