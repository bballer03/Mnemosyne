use super::types::{
    BuiltInField, ClassPattern, ComparisonOp, Condition, FieldRef, FromClause, LogicalOp, Query,
    QueryParseError, QueryStatement, SelectClause, TraversalFunction, Value, WhereClause,
    MAX_MULTI_CLASS_FROM_LIST_SIZE, MAX_OBJECTS_FIELD_HOPS,
};
use regex::Regex;

/// Bound on `FROM OBJECTS (<subquery>)` nesting: a query parsed at depth 0
/// (the top-level statement) may contain one subquery (parsed at depth 1),
/// but that inner query may not itself contain another `OBJECTS(...)`
/// subquery (M15 Slice 15.E §4.1 item 4 -- "bounded to one level of
/// nesting, not arbitrary recursion"). Mirrored, defense-in-depth, by
/// `executor::MAX_SUBQUERY_NESTING_DEPTH` for a `Query` assembled directly.
const MAX_SUBQUERY_NESTING_DEPTH: usize = 1;

pub fn parse_query(input: &str) -> Result<Query, QueryParseError> {
    let mut parser = Parser::new(input);
    let query = parser.parse_query_body(0)?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(parser.error("expected end of query"));
    }
    Ok(query)
}

/// Parses a full top-level OQL statement, which may be a single query or
/// two queries joined by `UNION` (M15 Slice 15.E §4.1 item 5). `parse_query`
/// above is left untouched -- it keeps parsing exactly one `Query` and
/// rejecting any trailing input (including a trailing `UNION ...`, exactly
/// as it always has) -- so every existing caller and test built around
/// `parse_query` continues to see identical behavior. Reach for this
/// function specifically to obtain `UNION` support.
pub fn parse_query_statement(input: &str) -> Result<QueryStatement, QueryParseError> {
    let mut parser = Parser::new(input);
    let left = parser.parse_query_body(0)?;
    parser.skip_ws();

    if parser.consume_keyword("UNION") {
        let right = parser.parse_query_body(0)?;
        parser.skip_ws();
        if !parser.is_eof() {
            return Err(parser.error("expected end of query"));
        }
        return Ok(QueryStatement::Union(left, right));
    }

    if !parser.is_eof() {
        return Err(parser.error("expected end of query"));
    }
    Ok(QueryStatement::Single(left))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Parses one `SELECT ... FROM ... [WHERE ...] [LIMIT ...]` body without
    /// requiring end-of-input afterwards -- shared by the top-level
    /// `parse_query`/`parse_query_statement` entry points (which each apply
    /// their own EOF/`UNION` handling) and by `try_parse_subquery_from_source`
    /// (which expects a closing `)` afterwards, not EOF). `depth` is this
    /// query's own subquery-nesting depth: 0 for a top-level query, 1 for a
    /// query reached through one `OBJECTS(...)` subquery.
    fn parse_query_body(&mut self, depth: usize) -> Result<Query, QueryParseError> {
        self.expect_keyword("SELECT")?;
        let select = self.parse_select_clause()?;
        self.expect_keyword("FROM")?;
        let from = self.parse_from_clause(depth)?;
        let filter = if self.consume_keyword("WHERE") {
            Some(self.parse_where_clause()?)
        } else {
            None
        };
        let limit = if self.consume_keyword("LIMIT") {
            Some(self.parse_usize()?)
        } else {
            None
        };

        Ok(Query {
            select,
            from,
            filter,
            limit,
        })
    }

    fn parse_select_clause(&mut self) -> Result<SelectClause, QueryParseError> {
        self.skip_ws();
        if self.consume_keyword("DISTINCT") {
            if !self.consume_keyword("OBJECTS") {
                return Err(self.error(
                    "DISTINCT is only supported with OBJECTS projection \
                     (SELECT DISTINCT OBJECTS ...); \
                     SELECT DISTINCT * / field lists are unsupported",
                ));
            }
            let field = self.parse_field_ref()?;
            validate_objects_field_hops(&field)?;
            return Ok(SelectClause::DistinctObjects(field));
        }
        if self.consume_char('*') {
            return Ok(SelectClause::All);
        }
        if self.consume_keyword("OBJECTS") {
            let field = self.parse_field_ref()?;
            validate_objects_field_hops(&field)?;
            return Ok(SelectClause::Objects(field));
        }

        let mut fields = vec![self.parse_field_ref()?];
        loop {
            self.skip_ws();
            if !self.consume_char(',') {
                break;
            }
            fields.push(self.parse_field_ref()?);
        }
        Ok(SelectClause::Fields(fields))
    }

    fn parse_from_clause(&mut self, depth: usize) -> Result<FromClause, QueryParseError> {
        self.skip_ws();
        let instanceof = self.consume_keyword("INSTANCEOF");

        if !instanceof {
            if let Some(traversal) = self.try_parse_traversal_function()? {
                return Ok(FromClause {
                    class_pattern: ClassPattern::Traversal(traversal),
                    instanceof: false,
                });
            }
            if let Some(subquery) = self.try_parse_subquery_from_source(depth)? {
                return Ok(FromClause {
                    class_pattern: ClassPattern::Subquery(Box::new(subquery)),
                    instanceof: false,
                });
            }
        }

        let class_pattern = self.parse_class_pattern_list()?;

        Ok(FromClause {
            class_pattern,
            instanceof,
        })
    }

    /// Parses one quoted class pattern, then any comma-separated siblings
    /// (M22 Slice 22.B). A lone pattern stays `Exact`/`Glob` for compatible
    /// serialization; two or more collapse into `ClassPattern::Multi`.
    fn parse_class_pattern_list(&mut self) -> Result<ClassPattern, QueryParseError> {
        let mut patterns = vec![self.parse_single_quoted_class_pattern()?];

        loop {
            self.skip_ws();
            if !self.consume_char(',') {
                break;
            }
            self.skip_ws();
            patterns.push(self.parse_single_quoted_class_pattern()?);
            if patterns.len() > MAX_MULTI_CLASS_FROM_LIST_SIZE {
                return Err(self.error(format!(
                    "multi-class FROM list exceeds limit of {MAX_MULTI_CLASS_FROM_LIST_SIZE} class patterns"
                )));
            }
        }

        Ok(if patterns.len() == 1 {
            patterns.into_iter().next().expect("one pattern")
        } else {
            ClassPattern::Multi(patterns)
        })
    }

    fn parse_single_quoted_class_pattern(&mut self) -> Result<ClassPattern, QueryParseError> {
        let pattern = self.parse_quoted_string()?;
        if pattern.is_empty() {
            return Err(self.error("expected non-empty quoted class pattern"));
        }
        Ok(if pattern.contains('*') {
            ClassPattern::Glob(pattern)
        } else {
            ClassPattern::Exact(pattern)
        })
    }

    /// Recognizes `outbounds(<id>)` / `inbounds(<id>)` / `dominators(<id>)`
    /// as alternative `FROM` sources (M15 Slice 15.C). The argument is
    /// scoped to a literal object-id integer for this slice -- nested query
    /// arguments are Slice 15.E's job (subqueries). Returns `Ok(None)`
    /// without consuming input if none of the three keywords match, so the
    /// caller can fall back to the ordinary quoted class-pattern parse.
    fn try_parse_traversal_function(
        &mut self,
    ) -> Result<Option<TraversalFunction>, QueryParseError> {
        let keyword = if self.consume_keyword("outbounds") {
            "outbounds"
        } else if self.consume_keyword("inbounds") {
            "inbounds"
        } else if self.consume_keyword("dominators") {
            "dominators"
        } else {
            return Ok(None);
        };

        self.skip_ws();
        if !self.consume_char('(') {
            return Err(self.error(format!("expected '(' after '{keyword}'")));
        }
        let object_id = self.parse_object_id()?;
        self.skip_ws();
        if !self.consume_char(')') {
            return Err(self.error(format!("expected ')' to close '{keyword}(...)'")));
        }

        let traversal = match keyword {
            "outbounds" => TraversalFunction::Outbounds(object_id),
            "inbounds" => TraversalFunction::Inbounds(object_id),
            _ => TraversalFunction::Dominators(object_id),
        };

        Ok(Some(traversal))
    }

    /// Recognizes `OBJECTS (<subquery>)` as an alternative `FROM` source
    /// (M15 Slice 15.E), same "try, return `Ok(None)` if the keyword isn't
    /// there" shape as `try_parse_traversal_function` above. `depth` is the
    /// nesting depth of the query currently being parsed (0 for a top-level
    /// query); encountering `OBJECTS(...)` while already at
    /// `MAX_SUBQUERY_NESTING_DEPTH` is a hard parse error naming the bound,
    /// not a silent truncation or a fallback to some other parse path.
    fn try_parse_subquery_from_source(
        &mut self,
        depth: usize,
    ) -> Result<Option<Query>, QueryParseError> {
        if !self.consume_keyword("OBJECTS") {
            return Ok(None);
        }

        self.skip_ws();
        if !self.consume_char('(') {
            return Err(self.error("expected '(' after 'OBJECTS'"));
        }
        if depth >= MAX_SUBQUERY_NESTING_DEPTH {
            return Err(self.error(
                "subquery nesting depth exceeded: OQL subqueries support only one level of nesting",
            ));
        }

        let inner = self.parse_query_body(depth + 1)?;
        self.skip_ws();
        if !self.consume_char(')') {
            return Err(self.error("expected ')' to close 'OBJECTS(...)' subquery"));
        }

        Ok(Some(inner))
    }

    /// Parses an unsigned 64-bit object-id literal (no sign, unlike the
    /// general integer literal `parse_value` accepts for `WHERE` values).
    fn parse_object_id(&mut self) -> Result<u64, QueryParseError> {
        self.skip_ws();
        let start = self.pos;
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            self.pos += ch_len(self.peek_char().unwrap());
        }
        if self.pos == start {
            return Err(self.error("expected object id"));
        }

        let raw = &self.input[start..self.pos];
        raw.parse::<u64>()
            .map_err(|_| self.error(format!("invalid object id '{raw}'")))
    }

    fn parse_where_clause(&mut self) -> Result<WhereClause, QueryParseError> {
        let mut conditions = vec![self.parse_condition()?];
        let mut operators = Vec::new();

        loop {
            if self.consume_keyword("AND") {
                operators.push(LogicalOp::And);
            } else if self.consume_keyword("OR") {
                operators.push(LogicalOp::Or);
            } else {
                break;
            }
            conditions.push(self.parse_condition()?);
        }

        Ok(WhereClause {
            conditions,
            operators,
        })
    }

    fn parse_condition(&mut self) -> Result<Condition, QueryParseError> {
        let field = self.parse_field_ref()?;
        if self.consume_keyword("IS") {
            let op = if self.consume_keyword("NOT") {
                ComparisonOp::IsNotNull
            } else {
                ComparisonOp::IsNull
            };
            self.expect_keyword("NULL")?;
            return Ok(Condition {
                field,
                op,
                value: Value::Null,
            });
        }
        let op = self.parse_comparison_op()?;
        let value = self.parse_value()?;
        if op == ComparisonOp::RegexMatch {
            self.validate_regex_pattern(&value)?;
        }
        Ok(Condition { field, op, value })
    }

    fn parse_field_ref(&mut self) -> Result<FieldRef, QueryParseError> {
        self.skip_ws();
        if self.consume_char('@') {
            let ident = self.parse_identifier()?;
            let field = match ident.as_str() {
                "objectId" => BuiltInField::ObjectId,
                "className" => BuiltInField::ClassName,
                "shallowSize" => BuiltInField::ShallowSize,
                "retainedSize" => BuiltInField::RetainedSize,
                "objectAddress" => BuiltInField::ObjectAddress,
                "toString" => BuiltInField::ToString,
                "gcRootPath" => BuiltInField::GcRootPath,
                _ => return Err(self.error(format!("invalid built-in field '@{ident}'"))),
            };
            Ok(FieldRef::BuiltIn(field))
        } else {
            Ok(FieldRef::InstanceField(self.parse_identifier()?))
        }
    }

    fn parse_comparison_op(&mut self) -> Result<ComparisonOp, QueryParseError> {
        self.skip_ws();
        for (token, op) in [
            ("!=", ComparisonOp::Ne),
            (">=", ComparisonOp::Ge),
            ("<=", ComparisonOp::Le),
            // Must be checked before the plain "=" token below -- otherwise
            // "=" would greedily match first and leave a dangling "~" that
            // fails the subsequent value parse instead of being recognized
            // as the regex operator.
            ("=~", ComparisonOp::RegexMatch),
            ("=", ComparisonOp::Eq),
            (">", ComparisonOp::Gt),
            ("<", ComparisonOp::Lt),
        ] {
            if self.consume_token(token) {
                return Ok(op);
            }
        }
        if self.consume_keyword("LIKE") {
            return Ok(ComparisonOp::Like);
        }
        if self.consume_keyword("CONTAINS") {
            return Ok(ComparisonOp::Contains);
        }
        if self.consume_keyword("INSTANCEOF") {
            return Ok(ComparisonOp::InstanceOf);
        }
        Err(self.error("expected comparison operator"))
    }

    /// Eagerly validates a `=~` regex pattern at parse time so a malformed
    /// pattern fails fast, before any graph work, with a structured error
    /// rather than surfacing as a panic or a silent no-match deep inside
    /// query execution (M15 Slice 15.D, §6 R3). The compiled `regex::Regex`
    /// itself is discarded here -- `Condition`/`Value` derive
    /// `Serialize`/`Deserialize`/`Eq` and cannot hold a compiled regex, so
    /// the pattern travels onward as a plain string and the executor
    /// (re-)compiles it once per query evaluation.
    fn validate_regex_pattern(&self, value: &Value) -> Result<(), QueryParseError> {
        let Value::Str(pattern) = value else {
            return Err(self.error("=~ requires a quoted regex pattern"));
        };
        Regex::new(pattern)
            .map_err(|err| self.error(format!("invalid regex pattern '{pattern}': {err}")))?;
        Ok(())
    }

    fn parse_value(&mut self) -> Result<Value, QueryParseError> {
        self.skip_ws();
        if matches!(self.peek_char(), Some('"' | '\'')) {
            return Ok(Value::Str(self.parse_quoted_string()?));
        }
        if self.consume_keyword("null") {
            return Ok(Value::Null);
        }
        if self.consume_keyword("true") {
            return Ok(Value::Bool(true));
        }
        if self.consume_keyword("false") {
            return Ok(Value::Bool(false));
        }

        let start = self.pos;
        if self.peek_char() == Some('-') {
            self.pos += 1;
        }
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            self.pos += ch_len(self.peek_char().unwrap());
        }
        if self.pos == start || (self.pos == start + 1 && &self.input[start..self.pos] == "-") {
            return Err(self.error("expected value"));
        }

        let raw = &self.input[start..self.pos];
        let int = raw
            .parse::<i64>()
            .map_err(|_| self.error(format!("invalid integer literal '{raw}'")))?;
        Ok(Value::Int(int))
    }

    fn parse_quoted_string(&mut self) -> Result<String, QueryParseError> {
        self.skip_ws();
        let Some(delimiter @ ('"' | '\'')) = self.peek_char() else {
            return Err(self.error("expected quoted string"));
        };
        self.pos += delimiter.len_utf8();

        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == delimiter {
                let value = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Ok(value);
            }
            self.pos += ch_len(ch);
        }

        Err(self.error("unterminated quoted string"))
    }

    fn parse_identifier(&mut self) -> Result<String, QueryParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                self.pos += ch_len(ch);
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.error("expected identifier"));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_usize(&mut self) -> Result<usize, QueryParseError> {
        self.skip_ws();
        let start = self.pos;
        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            self.pos += ch_len(self.peek_char().unwrap());
        }
        if self.pos == start {
            return Err(self.error("expected integer"));
        }
        self.input[start..self.pos]
            .parse::<usize>()
            .map_err(|_| self.error("invalid integer"))
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), QueryParseError> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error(format!("expected keyword '{keyword}'")))
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        self.skip_ws();
        let rest = &self.input[self.pos..];
        if rest.len() < keyword.len() {
            return false;
        }
        let candidate = &rest[..keyword.len()];
        if !candidate.eq_ignore_ascii_case(keyword) {
            return false;
        }
        let boundary = rest[keyword.len()..].chars().next();
        if matches!(boundary, Some(ch) if ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
        self.pos += keyword.len();
        true
    }

    fn consume_token(&mut self, token: &str) -> bool {
        self.skip_ws();
        if self.input[self.pos..].starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        self.skip_ws();
        if self.peek_char() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch_len(ch);
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn error(&self, message: impl Into<String>) -> QueryParseError {
        QueryParseError::new(format!("{} at byte {}", message.into(), self.pos))
    }
}

fn ch_len(ch: char) -> usize {
    ch.len_utf8()
}

/// Rejects `SELECT OBJECTS` field paths longer than
/// `MAX_OBJECTS_FIELD_HOPS` at parse time (M22 Slice 22.C). Uses the same
/// alias-prefix convention as the executor's `parse_objects_field_hops`.
fn validate_objects_field_hops(field: &FieldRef) -> Result<(), QueryParseError> {
    let FieldRef::InstanceField(path) = field else {
        return Ok(());
    };

    let segments: Vec<&str> = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    let hop_count = match segments.as_slice() {
        [] => 0,
        [_] => 1,
        [_, hops @ ..] => hops.len(),
    };

    if hop_count > MAX_OBJECTS_FIELD_HOPS {
        return Err(QueryParseError::new(format!(
            "multi-hop OBJECTS exceeds limit of {MAX_OBJECTS_FIELD_HOPS} field hops: '{path}'"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_recognizes_objects_keyword() {
        let mut parser = Parser::new("OBJECTS");

        assert!(parser.consume_keyword("OBJECTS"));
        assert!(parser.is_eof());
    }

    #[test]
    fn lexer_recognizes_contains_operator() {
        let mut parser = Parser::new("CONTAINS");

        assert!(parser.consume_keyword("CONTAINS"));
        assert!(parser.is_eof());
    }

    #[test]
    fn lexer_recognizes_is_null_and_is_not_null_sequence() {
        let mut is_null = Parser::new("IS NULL");
        assert!(is_null.consume_keyword("IS"));
        assert!(is_null.consume_keyword("NULL"));
        assert!(is_null.is_eof());

        let mut is_not_null = Parser::new("IS NOT NULL");
        assert!(is_not_null.consume_keyword("IS"));
        assert!(is_not_null.consume_keyword("NOT"));
        assert!(is_not_null.consume_keyword("NULL"));
        assert!(is_not_null.is_eof());
    }

    #[test]
    fn lexer_recognizes_at_retained_size_pseudo_attribute() {
        let field = Parser::new("@retainedSize")
            .parse_field_ref()
            .expect("field should parse");

        assert_eq!(field, FieldRef::BuiltIn(BuiltInField::RetainedSize));
    }

    #[test]
    fn lexer_recognizes_at_to_string_pseudo_attribute() {
        let field = Parser::new("@toString")
            .parse_field_ref()
            .expect("field should parse");

        assert_eq!(field, FieldRef::BuiltIn(BuiltInField::ToString));
    }

    #[test]
    fn lexer_recognizes_at_gc_root_path_pseudo_attribute() {
        let field = Parser::new("@gcRootPath")
            .parse_field_ref()
            .expect("field should parse");

        assert_eq!(format!("{field:?}"), "BuiltIn(GcRootPath)");
    }

    #[test]
    fn parser_constructs_objects_select_clause() {
        let query = parse_query(r#"SELECT OBJECTS entries FROM "com.example.BigCache""#)
            .expect("query should parse");

        assert!(format!("{:?}", query.select).contains("Objects"));
        assert!(format!("{:?}", query.select).contains("entries"));
    }

    #[test]
    fn parser_constructs_is_null_condition() {
        let query =
            parse_query(r#"SELECT @objectId FROM "com.example.BigCache" WHERE entries IS NULL"#)
                .expect("query should parse");

        assert!(format!("{:?}", query.filter).contains("IsNull"));
    }

    #[test]
    fn parser_constructs_pseudo_attribute_field_ref() {
        let query = parse_query(r#"SELECT @gcRootPath FROM "com.example.BigCache""#)
            .expect("query should parse");

        assert!(format!("{:?}", query.select).contains("GcRootPath"));
    }

    #[test]
    fn parser_constructs_outbounds_traversal_from_clause() {
        let query = parse_query("SELECT * FROM outbounds(12345)").expect("query should parse");

        assert_eq!(
            query.from,
            FromClause {
                class_pattern: ClassPattern::Traversal(TraversalFunction::Outbounds(12345)),
                instanceof: false,
            }
        );
    }

    #[test]
    fn parser_constructs_inbounds_traversal_from_clause() {
        let query = parse_query("SELECT * FROM inbounds(999)").expect("query should parse");

        assert_eq!(
            query.from,
            FromClause {
                class_pattern: ClassPattern::Traversal(TraversalFunction::Inbounds(999)),
                instanceof: false,
            }
        );
    }

    #[test]
    fn parser_constructs_dominators_traversal_from_clause() {
        let query = parse_query("SELECT * FROM dominators(42)").expect("query should parse");

        assert_eq!(
            query.from,
            FromClause {
                class_pattern: ClassPattern::Traversal(TraversalFunction::Dominators(42)),
                instanceof: false,
            }
        );
    }

    #[test]
    fn parser_supports_traversal_from_combined_with_where_and_limit() {
        let query =
            parse_query(r#"SELECT @objectId FROM outbounds(1) WHERE @shallowSize > 0 LIMIT 5"#)
                .expect("query should parse");

        assert_eq!(
            query.from.class_pattern,
            ClassPattern::Traversal(TraversalFunction::Outbounds(1))
        );
        assert!(query.filter.is_some());
        assert_eq!(query.limit, Some(5));
    }

    #[test]
    fn parser_rejects_traversal_function_missing_open_paren() {
        let error = parse_query("SELECT * FROM outbounds 12345)").expect_err("should fail");
        assert!(error.to_string().contains("expected '('"));
    }

    #[test]
    fn parser_rejects_traversal_function_missing_close_paren() {
        let error = parse_query("SELECT * FROM outbounds(12345").expect_err("should fail");
        assert!(error.to_string().contains("expected ')'"));
    }

    #[test]
    fn parser_rejects_traversal_function_non_integer_argument() {
        let error = parse_query(r#"SELECT * FROM outbounds("abc")"#).expect_err("should fail");
        assert!(error.to_string().contains("expected object id"));
    }

    #[test]
    fn parser_rejects_negative_object_id_in_traversal_function() {
        let error = parse_query("SELECT * FROM outbounds(-1)").expect_err("should fail");
        assert!(error.to_string().contains("expected object id"));
    }
}
