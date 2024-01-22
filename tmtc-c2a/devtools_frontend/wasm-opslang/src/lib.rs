use opslang_syn::typedef::*;
use wasm_bindgen::prelude::*;
use web_sys::js_sys;

mod free_variables;
mod union_value;
use union_value::UnionValue;

#[wasm_bindgen]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[derive(Debug)]
enum RuntimeError {
    ParseIntError(std::num::ParseIntError),
    ParseFloatError(std::num::ParseFloatError),
    Unimplemented(&'static str),
    TypeError(&'static str, &'static str),
    CheckValueFailure,
    JsOriginError(JsValue),
    Other(String),
    DivideByZero,
}

type Result<T, E = RuntimeError> = std::result::Result<T, E>;

#[wasm_bindgen(typescript_custom_section)]
const TS_SECTION_DRIVER: &str = r#"
interface Driver{
    sendCommand(
      prefix: string,
      component: string,
      executingComponent: string | undefined,
      timeIndicator: Value | undefined,
      command: string,
      params: Value[]
    ) : Promise<void>;
    waitMilliseconds(msecs : number) : Promise<void>;
    resolveVariable(variablePath : string) : Value | undefined;
    setLocalVariable(ident : string, value : Value);
    print(value : Value) : Promise<void>;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "Driver", typescript_type = "Driver")]
    pub type Driver;

    #[wasm_bindgen(catch, method, js_name = "sendCommand")]
    pub async fn send_command(
        this: &Driver,
        prefix: &str,
        component: &str,
        executingComponent: Option<&str>,
        time_indicator: Option<UnionValue>,
        command_name: &str,
        args: Vec<UnionValue>,
    ) -> Result<(), JsValue>;

    // これもControlStatusで扱うべきかもしれない
    #[wasm_bindgen(method, js_name = "waitMilliseconds")]
    pub async fn wait_milliseconds(this: &Driver, msecs: usize);

    // ここをasyncにすると評価がasync再帰になってちょっと面倒
    // スタックマシンにするか？
    #[wasm_bindgen(method, js_name = "resolveVariable")]
    pub fn resolve_variable(this: &Driver, variable_path: &str) -> Option<UnionValue>;

    // mutableな状態管理はExecutor側に任せることにする
    #[wasm_bindgen(method, js_name = "setLocalVariable")]
    pub fn set_local_variable(this: &Driver, ident: &str, value: UnionValue);

    #[wasm_bindgen(catch, method, js_name = "print")]
    pub async fn print(this: &Driver, value: UnionValue) -> Result<(), JsValue>;
}

#[derive(Debug)]
enum Value {
    Integer(i64),
    Double(f64),
    Bool(bool),
    Array(Vec<Value>),
    String(String),
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Double(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl Value {
    fn type_name(&self) -> &'static str {
        use Value::*;
        match self {
            Integer(_) => "integer",
            Double(_) => "double",
            Bool(_) => "bool",
            Array(_) => "array",
            String(_) => "string",
        }
    }

    fn integer(&self) -> Result<i64> {
        match self {
            Value::Integer(x) => Ok(*x),
            _ => type_err("integer", self),
        }
    }

    fn double(&self) -> Result<f64> {
        match self {
            Value::Double(x) => Ok(*x),
            _ => type_err("double", self),
        }
    }

    fn bool(&self) -> Result<bool> {
        match self {
            Value::Bool(x) => Ok(*x),
            _ => type_err("bool", self),
        }
    }

    fn array(&self) -> Result<&Vec<Value>> {
        match self {
            Value::Array(x) => Ok(x),
            _ => type_err("array", self),
        }
    }

    fn string(&self) -> Result<&str> {
        match self {
            Value::String(x) => Ok(x),
            _ => type_err("string", self),
        }
    }
}

fn type_err<T>(expected: &'static str, e: &Value) -> Result<T> {
    Err(RuntimeError::TypeError(expected, e.type_name()))
}

fn unimpl<T>(s: &'static str) -> Result<T> {
    Err(RuntimeError::Unimplemented(s))
}

struct Runner {
    driver: Driver,
}

impl Runner {
    pub(crate) fn expr(&self, e: &Expr) -> Result<Value> {
        use Expr::*;
        match e {
            Variable(variable_path) => self.variable(variable_path),
            TlmRef(tlm_id) => self.tlmref(tlm_id),
            Literal(l) => self.literal(l),
            UnOp(unop, e) => self.unop(unop, e),
            BinOp(binop, left, right) => self.binop(binop, left, right),
            FunCall(_fun, _args) => unimpl("expr.funcall"),
        }
    }

    pub fn variable(&self, variable_path: &VariablePath) -> Result<Value> {
        self.driver
            .resolve_variable(&variable_path.raw)
            .map(Into::into)
            .ok_or_else(|| RuntimeError::Other(format!("variable {} not found", variable_path.raw)))
    }

    pub fn tlmref(&self, variable_path: &VariablePath) -> Result<Value> {
        //FIXME: prefixing with "$" is a dirty hack
        self.driver
            .resolve_variable(format!("${}", variable_path.raw).as_str())
            .map(Into::into)
            .ok_or_else(|| RuntimeError::Other(format!("variable {} not found", variable_path.raw)))
    }

    fn literal(&self, l: &Literal) -> Result<Value> {
        use Literal::*;
        match l {
            Array(es) => es
                .iter()
                .map(|e| self.expr(e))
                .collect::<Result<_, _>>()
                .map(Value::Array),
            Numeric(num, None) => self.numeric(num),
            Numeric(_, Some(_)) => unimpl("lit.numeric_suffix"),
            String(s) => Ok(Value::String((*s).to_owned())),
            DateTime(_dt) => unimpl("expr.datetime"),
            TlmId(_tlm_id) => unimpl("expr.tlm_id"),
        }
    }

    fn numeric(&self, num: &Numeric) -> Result<Value> {
        use Numeric::*;
        match num {
            Integer(s, prefix) => {
                use IntegerPrefix::*;
                let base = match prefix {
                    Hexadecimal => 16,
                    Decimal => 10,
                    Octal => 8,
                    Binary => 2,
                };
                i64::from_str_radix(s, base)
                    .map(Value::Integer)
                    .map_err(RuntimeError::ParseIntError)
            }
            Float(s) => s
                .parse()
                .map(Value::Double)
                .map_err(RuntimeError::ParseFloatError),
        }
    }

    fn binop(&self, op: &BinOpKind, left: &Expr, right: &Expr) -> Result<Value> {
        use BinOpKind::*;
        match op {
            Compare(comp_op) => self.compare(comp_op, left, right),
            If => self.bool_binop(|x, y| x || !y, left, right),
            And => self.bool_binop(bool::min, left, right),
            Or => self.bool_binop(bool::max, left, right),
            Mul => self.num_binop(
                |x, y| Ok((x * y).into()),
                |x, y| Ok((x * y).into()),
                left,
                right,
            ),
            Div => self.num_binop(
                |x, y| {
                    if y == 0 {
                        Err(RuntimeError::DivideByZero)
                    } else {
                        Ok((x / y).into())
                    }
                },
                |x, y| Ok((x / y).into()),
                left,
                right,
            ),
            Add => self.num_binop(
                |x, y| Ok((x + y).into()),
                |x, y| Ok((x + y).into()),
                left,
                right,
            ),
            Sub => self.num_binop(
                |x, y| Ok((x - y).into()),
                |x, y| Ok((x - y).into()),
                left,
                right,
            ),
            Mod => {
                let left = self.expr(left)?.integer()?;
                let right = self.expr(right)?.integer()?;
                if right == 0 {
                    Err(RuntimeError::DivideByZero)
                } else {
                    Ok((left % right).into())
                }
            }
            In => {
                let left = self.expr(left)?;
                let right = self.expr(right)?;
                let right = right.array()?;
                if right.len() != 2 {
                    return Err(RuntimeError::Other(
                        "the second operand must have two elements".to_owned(),
                    ));
                }

                use Value::*;
                match left {
                    Integer(x) => {
                        let start = right[0].integer()?;
                        let end = right[1].integer()?;
                        Ok((start <= x && x <= end).into())
                    }
                    Double(x) => {
                        let start = right[0].double()?;
                        let end = right[1].double()?;
                        Ok((start <= x && x <= end).into())
                    }
                    Bool(x) => {
                        let start = right[0].bool()?;
                        let end = right[1].bool()?;
                        Ok((start <= x && x <= end).into())
                    }
                    _ => type_err("comparable", &left),
                }
            }
        }
    }

    // add short-circuit?
    // short-circuitを入れると右辺の型がおかしくても通してしまう
    fn bool_binop(
        &self,
        op: impl Fn(bool, bool) -> bool,
        left: &Expr,
        right: &Expr,
    ) -> Result<Value> {
        let left = self.expr(left)?.bool()?;
        let right = self.expr(right)?.bool()?;
        Ok(op(left, right).into())
    }

    fn num_binop(
        &self,
        op_i64: impl Fn(i64, i64) -> Result<Value>,
        op_f64: impl Fn(f64, f64) -> Result<Value>,
        left: &Expr,
        right: &Expr,
    ) -> Result<Value> {
        use Value::*;
        let left = self.expr(left)?;
        match left {
            Integer(left) => {
                let right = self.expr(right)?.integer()?;
                op_i64(left, right)
            }
            Double(left) => {
                let right = self.expr(right)?.double()?;
                op_f64(left, right)
            }
            Bool(_) | Array(_) | String(_) => type_err("numeric", &left),
        }
    }

    fn unop(&self, op: &UnOpKind, e: &Expr) -> Result<Value> {
        use UnOpKind::*;
        use Value::*;
        match op {
            Neg => {
                let v = self.expr(e)?;
                match v {
                    Integer(x) => Ok(Integer(-x)),
                    Double(x) => Ok(Double(-x)),
                    Bool(x) => Ok(Bool(!x)),
                    Array(_) | String(_) => type_err("numeric or bool", &v),
                }
            }
        }
    }

    fn compare(&self, comp_op: &CompareBinOpKind, left: &Expr, right: &Expr) -> Result<Value> {
        let left = self.expr(left)?;
        let right = self.expr(right)?;

        use Value::*;
        let ord = match left {
            Integer(x) => Some(x.cmp(&right.integer()?)),
            Double(x) => x.partial_cmp(&right.double()?),
            Bool(x) => Some(x.cmp(&right.bool()?)),
            Array(_) => return type_err("comparable", &left),
            String(x) => Some(x[..].cmp(right.string()?)),
        };
        let ord = match ord {
            Some(ord) => ord,
            None => return Ok(false.into()),
        };

        use std::cmp::Ordering;
        use CompareBinOpKind::*;
        let b = match comp_op {
            GreaterEq => ord >= Ordering::Equal,
            LessEq => ord <= Ordering::Equal,
            Greater => ord == Ordering::Greater,
            Less => ord == Ordering::Less,
            NotEqual => ord != Ordering::Equal,
            Equal => ord == Ordering::Equal,
        };
        Ok(b.into())
    }

    async fn send_command(
        &self,
        prefix: &str,
        component: &str,
        executing_component: Option<&str>,
        time_indicator: Option<UnionValue>,
        command_name: &str,
        args: Vec<UnionValue>,
    ) -> Result<()> {
        self.driver
            .send_command(
                prefix,
                component,
                executing_component,
                time_indicator,
                command_name,
                args,
            )
            .await
            .map_err(RuntimeError::JsOriginError)
    }

    async fn exec_statement<'bc>(
        &mut self,
        block_context: BlockContext<'bc>,
        stmt: &SingleStatement,
    ) -> Result<ControlStatus> {
        use ControlStatus::*;
        use SingleStatement::*;
        match stmt {
            Call(_) => unimpl("stmt.call"),
            Wait(_w) => unimpl("stmt.wait"),
            //WaitSec(w) => {
            //    let e = self.expr(&w.sec)?;
            //    let msecs = match e {
            //        Value::Integer(n) => n * 1000,
            //        Value::Double(x) => (x * 1000.0) as _,
            //        _ => return Err(RuntimeError::TypeError("numeric", e.type_name())),
            //    };
            //    if msecs > 0 {
            //        self.driver.wait_milliseconds(msecs as _).await;
            //    }
            //    Ok(Executed)
            //}
            //WaitUntil(c) => {
            //    let cond = self.expr(&c.condition)?;
            //    match cond {
            //        Value::Bool(true) => Ok(Executed),
            //        Value::Bool(false) => Ok(Retry),
            //        _ => Err(RuntimeError::TypeError("bool", cond.type_name())),
            //    }
            //}
            Assert(c) => {
                let cond = self.expr(&c.condition)?;
                match cond {
                    Value::Bool(true) => Ok(Executed),
                    Value::Bool(false) => Err(RuntimeError::CheckValueFailure),
                    _ => Err(RuntimeError::TypeError("bool", cond.type_name())),
                }
            }
            AssertEq(_a) => unimpl("stmt.assert_eq"),
            Command(command) => {
                let receiver = command
                    .destination
                    .receiver
                    .as_ref()
                    .or(block_context.default_destination)
                    .ok_or_else(|| RuntimeError::Other("no receiver".to_owned()))?;
                let executor = command.destination.executor.as_ref();
                let ti = if let Some(ti) = &command.destination.time_indicator {
                    Some(self.expr(ti)?.into())
                } else {
                    None
                };

                let args: Result<Vec<_>> = command
                    .args
                    .iter()
                    .map(|e| self.expr(e).map(Into::into))
                    .collect();

                self.send_command(
                    receiver.exec_method.as_str(),
                    receiver.component.as_str(),
                    executor.map(|e| e.component.as_str()),
                    ti,
                    &command.name,
                    args?,
                )
                .await?;
                //TODO: apply delay
                Ok(Executed)
            }
            Let(l) => {
                let value = self.expr(&l.rhs)?;
                self.driver
                    .set_local_variable(&l.variable.raw, value.into());
                Ok(Executed)
            }
            Print(p) => {
                let arg = self.expr(&p.arg)?;
                self.driver
                    .print(arg.into())
                    .await
                    .map_err(RuntimeError::JsOriginError)?;
                Ok(Executed)
            }
        }
    }
}

#[wasm_bindgen]
#[derive(Debug)]
pub enum ControlStatus {
    // Stopped at a breakpoint
    // Executor (i.e. the caller of `execute_line`) should stop execution, and execute this line
    // again when resuming execution
    Breaked,

    // Executor shold proceed to the next line
    Executed,

    // Wait condition is not satisfied
    // Executor should execute this line again
    Retry,
}

//TODO: reimplement this
#[wasm_bindgen(js_name = validateLine)]
pub fn validate_line(_input: &str, _line_num: usize) -> Result<(), String> {
    Ok(())
    //opslang_syn::parser::parse_row(input)
    //    .map(|_| ())
    //    .map_err(|mut e| {
    //        e.location.line += line_num;
    //        e.location.line -= 1; // because line numbers are 1-indexed
    //        e.to_string()
    //    })
}

#[wasm_bindgen]
pub struct ParsedCode {
    ast: Vec<Statement>,
    line_offsets: Vec<usize>,
}

struct BlockContext<'a> {
    default_destination: Option<&'a Destination>,
    delay: Option<&'a Expr>,
}

enum FoundRow<'a> {
    RowWithContext {
        block_context: BlockContext<'a>,
        row: &'a Row,
    },
    Empty, // found, but empty (e.g. opening/closisng brace)
}

impl ParsedCode {
    fn find_row(&self, line_num: usize) -> Option<FoundRow> {
        let offset = self.line_offsets[line_num - 1];
        let statement = self.ast.iter().find(|stmt| {
            let span = match stmt {
                Statement::Single(row) => &row.span,
                Statement::Block(block) => &block.span,
            };
            span.contains(&offset)
        })?;
        match statement {
            Statement::Single(row) => Some(FoundRow::RowWithContext {
                block_context: BlockContext {
                    default_destination: None,
                    delay: None,
                },
                row,
            }),
            Statement::Block(block) => {
                if block.rows.is_empty()  || //Empty block
                    offset < block.rows[0].span.start  || //Opening brace
                    offset > block.rows.last().unwrap().span.end
                // Closing brace
                {
                    Some(FoundRow::Empty)
                } else {
                    let row = block.rows.iter().find(|row| row.span.contains(&offset))?;
                    Some(FoundRow::RowWithContext {
                        block_context: BlockContext {
                            default_destination: block.default_destination.as_ref(),
                            delay: block.delay.as_ref(),
                        },
                        row,
                    })
                }
            }
        }
        //self.ast.iter().find(|row| row.span.contains(&offset))
    }
}

#[wasm_bindgen]
impl ParsedCode {
    #[wasm_bindgen(js_name = fromCode)]
    pub fn from_code(code: &str) -> Result<ParsedCode, JsValue> {
        let ast = opslang_syn::parser::parse_statements(code).map_err(|e| e.to_string())?;
        let newlines = code
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i);
        let line_offsets = std::iter::once(0).chain(newlines.map(|i| i + 1)).collect();
        Ok(ParsedCode { ast, line_offsets })
    }

    #[wasm_bindgen(js_name = executeLine)]
    pub async fn execute_line(
        &self,
        driver: Driver,
        stop_on_break: bool,
        line_num: usize,
    ) -> Result<ControlStatus, String> {
        let result = self.execute_line_(driver, stop_on_break, line_num).await;
        match &result {
            Ok(sc) => {
                log!("execute_line ok: {:?}", sc);
            }
            Err(e) => {
                log!("execute_line err: {}", e);
            }
        };
        result
    }

    pub async fn execute_line_(
        &self,
        driver: Driver,
        stop_on_break: bool,
        line_num: usize,
    ) -> Result<ControlStatus, String> {
        let mut runner = Runner { driver };

        let found_row = self
            .find_row(line_num)
            .ok_or_else(|| format!("line {} not found", line_num))?;

        let (block_context, row) = match found_row {
            FoundRow::Empty => return Ok(ControlStatus::Executed),
            FoundRow::RowWithContext { block_context, row } => (block_context, row),
        };

        if row.breaks.is_some() && stop_on_break {
            return Ok(ControlStatus::Breaked);
        }
        if let Some(stmt) = &row.content {
            runner
                .exec_statement(block_context, stmt)
                .await
                .map_err(|e| format!("{:?}", e))
        } else {
            Ok(ControlStatus::Executed)
        }
    }

    #[wasm_bindgen(js_name = freeVariables)]
    pub fn free_variables(&self, line_num: usize) -> Result<Vec<String>, String> {
        let found_row = self
            .find_row(line_num)
            .ok_or_else(|| format!("line {} not found", line_num))?;

        let row = match found_row {
            FoundRow::Empty => return Ok(vec![]),
            FoundRow::RowWithContext { row, .. } => row,
        };
        if let Some(stmt) = &row.content {
            use std::collections::HashSet;
            Ok(free_variables::stmt(
                stmt,
                &HashSet::new(), // TODO: manage bound variables?
            )
            .into_iter()
            .collect())
        } else {
            Ok(vec![])
        }
    }
}
