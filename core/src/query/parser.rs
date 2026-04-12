use super::types::{
    BuiltInField, ClassPattern, ComparisonOp, Condition, FieldRef, FromClause, LogicalOp, Query,
    QueryParseError, SelectClause, Value, WhereClause,
};

pub fn parse_query(input: &str) -> Result<Query, QueryParseError> {
    let mut parser = Parser::new(input);
    parser.parse_query()
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_query(&mut self) -> Result<Query, QueryParseError> {
        self.expect_keyword("SELECT")?;
        let select = self.parse_select_clause()?;
        self.expect_keyword("FROM")?;
        let from = self.parse_from_clause()?;
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
        self.skip_ws();
        if !self.is_eof() {
            return Err(self.error("expected end of query"));
        }

        Ok(Query {
            select,
            from,
            filter,
            limit,
        })
    }

    fn parse_select_clause(&mut self) -> Result<SelectClause, QueryParseError> {
        self.skip_ws();
        if self.consume_char('*') {
            return Ok(SelectClause::All);
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

    fn parse_from_clause(&mut self) -> Result<FromClause, QueryParseError> {
        self.skip_ws();
        let instanceof = self.consume_keyword("INSTANCEOF");
        let pattern = self.parse_quoted_string()?;
        let class_pattern = if pattern.contains('*') {
            ClassPattern::Glob(pattern)
        } else {
            ClassPattern::Exact(pattern)
        };

        Ok(FromClause {
            class_pattern,
            instanceof,
        })
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
        let op = self.parse_comparison_op()?;
        let value = self.parse_value()?;
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
        if self.consume_keyword("INSTANCEOF") {
            return Ok(ComparisonOp::InstanceOf);
        }
        Err(self.error("expected comparison operator"))
    }

    fn parse_value(&mut self) -> Result<Value, QueryParseError> {
        self.skip_ws();
        if self.peek_char() == Some('"') {
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
        if !self.consume_char('"') {
            return Err(self.error("expected quoted string"));
        }

        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == '"' {
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
