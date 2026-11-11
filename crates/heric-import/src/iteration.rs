//! Iteration data for the collection runner.

/// Where iteration data came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationSource {
    /// A JSON file, which is an array of objects.
    Json,
    /// A CSV file, whose header row names the columns.
    Csv,
}

/// Data for a collection run, one row per iteration.
///
/// The runner substitutes a row's values into the request under the iteration
/// scope, which sits below the environment but above the collection. The parser
/// has to tolerate the shapes a spreadsheet produces: quoted separators, CRLF
/// line endings and a row shorter than the header (`IO-040`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterationData {
    /// The column names, in file order.
    pub columns: Vec<String>,
    /// The rows.
    pub rows: Vec<IterationRow>,
    /// Where it came from.
    pub source: Option<IterationSource>,
}

/// One iteration's values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterationRow {
    /// Values, keyed by column name.
    pub values: std::collections::BTreeMap<String, String>,
}

impl IterationData {
    /// How many iterations the data describes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there is no data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The row for an iteration, counting from 1.
    ///
    /// The original numbers iterations from 1, which is what `insomnia.info`
    /// reports and what a user sees in the runner.
    #[must_use]
    pub fn row(&self, iteration: u32) -> Option<&IterationRow> {
        let index = usize::try_from(iteration).ok()?.checked_sub(1)?;
        self.rows.get(index)
    }

    /// Parses CSV text.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ImportError::Parse`] when the header row is missing or a
    /// row has more fields than the header.
    pub fn from_csv(text: &str) -> Result<Self, crate::ImportError> {
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header = lines.next().ok_or_else(|| crate::ImportError::Parse {
            format: "CSV",
            detail: "the file has no header row".to_owned(),
        })?;
        let columns: Vec<String> = parse_csv_row(header)
            .into_iter()
            .map(|name| name.trim().to_owned())
            .collect();
        if columns.is_empty() {
            return Err(crate::ImportError::Parse {
                format: "CSV",
                detail: "the file has no header row".to_owned(),
            });
        }

        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            let fields = parse_csv_row(line);
            if fields.len() > columns.len() {
                return Err(crate::ImportError::Parse {
                    format: "CSV",
                    detail: format!(
                        "row {} has {} fields but the header has {}",
                        index + 2,
                        fields.len(),
                        columns.len()
                    ),
                });
            }
            let mut values = std::collections::BTreeMap::new();
            for (column, value) in columns.iter().zip(fields) {
                values.insert(column.clone(), value);
            }
            rows.push(IterationRow { values });
        }

        Ok(Self {
            columns,
            rows,
            source: Some(IterationSource::Csv),
        })
    }
}

/// Splits one CSV row, honouring quoted fields and doubled quotes.
///
/// Written rather than taken from a crate: CSV's escaping rules for a runner's
/// data file are ten lines, and a dependency here would be a dependency on every
/// platform Heric ships.
fn parse_csv_row(line: &str) -> Vec<String> {
    let line = line.trim_end_matches('\r');
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' if quoted => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    let _ = chars.next();
                } else {
                    quoted = false;
                }
            }
            '"' => quoted = true,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_file_becomes_columns_and_rows() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "name,age
ann,30
bob,41
",
        )?;
        assert_eq!(data.columns, vec!["name", "age"]);
        assert_eq!(data.len(), 2);
        assert_eq!(
            data.row(1)
                .ok_or("row 1")?
                .values
                .get("name")
                .map(String::as_str),
            Some("ann")
        );
        assert_eq!(
            data.row(2)
                .ok_or("row 2")?
                .values
                .get("age")
                .map(String::as_str),
            Some("41")
        );
        assert_eq!(data.row(3), None);
        Ok(())
    }

    #[test]
    fn a_quoted_separator_stays_in_one_field() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "a,b
\"x,y\",z
",
        )?;
        assert_eq!(
            data.row(1)
                .ok_or("row 1")?
                .values
                .get("a")
                .map(String::as_str),
            Some("x,y")
        );
        assert_eq!(
            data.row(1)
                .ok_or("row 1")?
                .values
                .get("b")
                .map(String::as_str),
            Some("z")
        );
        Ok(())
    }

    #[test]
    fn a_doubled_quote_becomes_one_quote() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "a
\"he said \"\"hi\"\"\"
",
        )?;
        assert_eq!(
            data.row(1)
                .ok_or("row 1")?
                .values
                .get("a")
                .map(String::as_str),
            Some("he said \"hi\"")
        );
        Ok(())
    }

    #[test]
    fn crlf_line_endings_are_tolerated() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "a,b
1,2
",
        )?;
        assert_eq!(
            data.row(1)
                .ok_or("row 1")?
                .values
                .get("b")
                .map(String::as_str),
            Some("2")
        );
        Ok(())
    }

    #[test]
    fn a_short_row_fills_the_columns_it_has() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "a,b,c
1,2
",
        )?;
        let row = data.row(1).ok_or("row 1")?;
        assert_eq!(row.values.get("a").map(String::as_str), Some("1"));
        assert_eq!(row.values.get("c"), None);
        Ok(())
    }

    #[test]
    fn a_row_longer_than_the_header_is_an_error() {
        assert!(
            IterationData::from_csv(
                "a,b
1,2,3
"
            )
            .is_err()
        );
    }

    #[test]
    fn an_empty_file_is_an_error() {
        assert!(IterationData::from_csv("").is_err());
    }

    #[test]
    fn iteration_numbers_start_at_one() -> Result<(), Box<dyn std::error::Error>> {
        let data = IterationData::from_csv(
            "a
x
",
        )?;
        assert!(data.row(0).is_none());
        assert!(data.row(1).is_some());
        Ok(())
    }
}
