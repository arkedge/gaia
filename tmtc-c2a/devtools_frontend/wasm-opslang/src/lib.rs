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
    sendCommand(prefix : string, component : string, commandName : string, args: Value[]) : Promise<void>;
    waitMilliseconds(msecs : number) : Promise<void>;
    resolveVariable(variablePath : string) : Value | undefined;
    setLocalVariable(ident : string, value : Value);
    get(variablePath : string) : Promise<void>;
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

    #[wasm_bindgen(catch, method, js_name = "get")]
    pub async fn get(this: &Driver, variable_path: &str) -> Result<(), JsValue>;
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
    pub(crate) fn expr(&self, e: &Expr<'_>) -> Result<Value> {
        use Expr::*;
        match e {
            Variable(variable_path) => self.variable(variable_path),
            Literal(l) => self.literal(l),
            UnOp(unop, e) => self.unop(unop, e),
            BinOp(binop, left, right) => self.binop(binop, left, right),
            FunCall(_fun, _args) => unimpl("expr.funcall"),
        }
    }

    pub fn variable(&self, variable_path: &VariablePath<'_>) -> Result<Value> {
        let r = self
            .driver
            .resolve_variable(variable_path.raw)
            .map(Into::into)
            .ok_or_else(|| {
                RuntimeError::Other(format!("variable {} not found", variable_path.raw))
            });
        r
    }

    fn literal(&self, l: &Literal<'_>) -> Result<Value> {
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
        }
    }

    fn numeric(&self, num: &Numeric<'_>) -> Result<Value> {
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

    fn binop(&self, op: &BinOpKind, left: &Expr<'_>, right: &Expr<'_>) -> Result<Value> {
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

    fn bool_binop(
        &self,
        op: impl Fn(bool, bool) -> bool,
        left: &Expr<'_>,
        right: &Expr<'_>,
    ) -> Result<Value> {
        let left = self.expr(left)?.bool()?;
        let right = self.expr(right)?.bool()?;
        Ok(op(left, right).into())
    }

    fn num_binop(
        &self,
        op_i64: impl Fn(i64, i64) -> Result<Value>,
        op_f64: impl Fn(f64, f64) -> Result<Value>,
        left: &Expr<'_>,
        right: &Expr<'_>,
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

    fn unop(&self, op: &UnOpKind, e: &Expr<'_>) -> Result<Value> {
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

    fn compare(
        &self,
        comp_op: &CompareBinOpKind,
        left: &Expr<'_>,
        right: &Expr<'_>,
    ) -> Result<Value> {
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
        command_name: &str,
        args: Vec<UnionValue>,
    ) -> Result<()> {
        self.driver
            .send_command(prefix, component, command_name, args)
            .await
            .map_err(RuntimeError::JsOriginError)
    }

    async fn exec_statement(&mut self, stmt: ReservedControl<'_>) -> Result<ControlStatus> {
        use ControlStatus::*;
        use ReservedControl::*;
        match stmt {
            Call(_) => unimpl("stmt.call"),
            WaitSec(w) => {
                let e = self.expr(&w.sec)?;
                let msecs = match e {
                    Value::Integer(n) => n * 1000,
                    Value::Double(x) => (x * 1000.0) as _,
                    _ => return Err(RuntimeError::TypeError("numeric", e.type_name())),
                };
                if msecs > 0 {
                    self.driver.wait_milliseconds(msecs as _).await;
                }
                Ok(Executed)
            }
            WaitUntil(c) => {
                let cond = self.expr(&c.condition)?;
                match cond {
                    Value::Bool(true) => Ok(Executed),
                    Value::Bool(false) => Ok(Retry),
                    _ => Err(RuntimeError::TypeError("bool", cond.type_name())),
                }
            }
            CheckValue(c) => {
                let cond = self.expr(&c.condition)?;
                match cond {
                    Value::Bool(true) => Ok(Executed),
                    Value::Bool(false) => Err(RuntimeError::CheckValueFailure),
                    _ => Err(RuntimeError::TypeError("bool", cond.type_name())),
                }
            }
            Command(command) => {
                if command.destinations.is_empty() {
                    return Err(RuntimeError::Other("empty command".to_owned()));
                }
                //TODO multiple destinations
                let dest = &command.destinations[0];
                let exec_method = dest.exec_method;
                let component = dest.component;
                if !command.name.starts_with("Cmd_") {
                    return Err(RuntimeError::Other("unknown command format".to_owned()));
                }
                let command_name = &command.name[4..];
                let args: Vec<_> = command
                    .args
                    .iter()
                    .filter_map(|e| self.expr(e).ok().map(Into::into))
                    .collect();
                self.send_command(exec_method, component, command_name, args)
                    .await?;
                Ok(Executed)
            }
            Let(l) => {
                let value = self.expr(&l.rhs)?;
                self.driver.set_local_variable(l.variable.raw, value.into());
                Ok(Executed)
            }
            Get(g) => {
                self.driver
                    .get(g.variable.raw)
                    .await
                    .map_err(RuntimeError::JsOriginError)?;
                Ok(Executed)
            }
        }
    }
}

#[wasm_bindgen]
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

#[wasm_bindgen(js_name = executeLine)]
pub async fn execute_line(
    driver: Driver,
    input: &str,
    stop_on_break: bool,
) -> Result<ControlStatus, String> {
    let mut runner = Runner { driver };
    let result = opslang_syn::parser::parse_row(input).map_err(|e| e.to_string())?;
    if result.breaks.is_some() && stop_on_break {
        return Ok(ControlStatus::Breaked);
    }
    if let Some(stmt) = result.content {
        runner
            .exec_statement(stmt)
            .await
            .map_err(|e| format!("{:?}", e))
    } else {
        Ok(ControlStatus::Executed)
    }
}

#[wasm_bindgen(js_name = freeVariables)]
pub fn free_variables(input: &str) -> Result<Vec<String>, String> {
    let result = opslang_syn::parser::parse_row(input).map_err(|e| e.to_string())?;
    if let Some(stmt) = result.content {
        use std::collections::HashSet;
        Ok(free_variables::stmt(
            &stmt,
            &HashSet::new(), // TODO: manage bound variables?
        )
        .into_iter()
        .map(ToOwned::to_owned)
        .collect())
    } else {
        Ok(vec![])
    }
}

#[wasm_bindgen(js_name = validateLine)]
pub fn validate_line(input: &str, line_num: usize) -> Result<(), String> {
    opslang_syn::parser::parse_row(input)
        .map(|_| ())
        .map_err(|mut e| {
            e.location.line += line_num;
            e.location.line -= 1; // because line numbers are 1-indexed
            e.to_string()
        })
}
