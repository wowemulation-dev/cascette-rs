//! BPSV document with string interning for memory efficiency

use crate::error::Result;
use crate::interner::{InternedValue, StringInterner};
use crate::schema::BpsvSchema;
use crate::value::BpsvValue;
use std::sync::Arc;

/// A BPSV document that uses string interning to reduce memory usage
///
/// This is particularly effective for config files where the same values
/// appear repeatedly across rows (e.g., region codes, server names, etc.)
#[derive(Debug, Clone)]
pub struct InternedBpsvDocument {
    /// The schema for this document
    schema: Arc<BpsvSchema>,
    /// Rows of interned values
    rows: Vec<InternedRow>,
    /// Sequence number from the document
    sequence_number: Option<u32>,
    /// String interner used by this document
    interner: StringInterner,
}

/// A row of interned values
#[derive(Debug, Clone)]
pub struct InternedRow {
    values: Vec<InternedValue>,
}

impl InternedBpsvDocument {
    /// Create a new interned document from a regular document
    pub fn from_document(doc: crate::document::BpsvDocument<'_>) -> Self {
        let interner = StringInterner::with_capacity(100);
        let mut interned_rows = Vec::with_capacity(doc.rows().len());

        // Save the schema and sequence number before consuming the document
        let schema = Arc::new(doc.schema().clone());
        let sequence_number = doc.sequence_number();

        // Convert each row to use interned strings
        for row in doc.into_owned_rows() {
            let mut interned_values = Vec::with_capacity(row.len());

            // Parse typed values if needed
            let typed_values = if let Some(typed) = row.typed_values {
                typed
            } else {
                // Parse raw values to typed
                let mut typed = Vec::new();
                for (value, field) in row.raw_values.iter().zip(schema.fields()) {
                    if let Ok(typed_value) = BpsvValue::parse(value, &field.field_type) {
                        typed.push(typed_value);
                    } else {
                        typed.push(BpsvValue::Empty);
                    }
                }
                typed
            };

            // Intern the typed values
            for value in typed_values {
                interned_values.push(InternedValue::from_bpsv_value(value, &interner));
            }

            interned_rows.push(InternedRow {
                values: interned_values,
            });
        }

        Self {
            schema,
            rows: interned_rows,
            sequence_number,
            interner,
        }
    }

    /// Parse and create an interned document directly from BPSV data
    pub fn parse(data: &str) -> Result<Self> {
        let doc = crate::document::BpsvDocument::parse(data)?;
        Ok(Self::from_document(doc))
    }

    /// Get the schema
    pub fn schema(&self) -> &BpsvSchema {
        &self.schema
    }

    /// Get all rows
    pub fn rows(&self) -> &[InternedRow] {
        &self.rows
    }

    /// Get the sequence number
    pub fn sequence_number(&self) -> Option<u32> {
        self.sequence_number
    }

    /// Get memory statistics for this document
    pub fn memory_stats(&self) -> crate::interner::MemoryStats {
        self.interner.memory_usage()
    }

    /// Get the interner hit rate
    pub fn interner_hit_rate(&self) -> f64 {
        self.interner.hit_rate()
    }

    /// Find rows where a field matches a value
    pub fn find_rows(&self, field_name: &str, value: &str) -> Vec<&InternedRow> {
        let field_index = match self.schema.get_field(field_name) {
            Some(field) => field.index,
            None => return vec![],
        };

        self.rows
            .iter()
            .filter(|row| {
                row.values
                    .get(field_index)
                    .and_then(|v| v.as_str())
                    .map(|s| s == value)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get a specific row by index
    pub fn get_row(&self, index: usize) -> Option<&InternedRow> {
        self.rows.get(index)
    }

    /// Get the number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Check if the document is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl InternedRow {
    /// Get a value by field index
    pub fn get(&self, index: usize) -> Option<&InternedValue> {
        self.values.get(index)
    }

    /// Get a value by field name
    pub fn get_by_name(&self, field_name: &str, schema: &BpsvSchema) -> Option<&InternedValue> {
        schema
            .get_field(field_name)
            .and_then(|field| self.get(field.index))
    }

    /// Get all values
    pub fn values(&self) -> &[InternedValue] {
        &self.values
    }

    /// Get the number of values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the row is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
