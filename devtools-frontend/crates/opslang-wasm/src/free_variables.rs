use opslang_ast::*;
use std::collections::HashSet;

pub struct Variables {
    pub variables: HashSet<String>,
    pub telemetry_variables: HashSet<String>,
}

impl Variables {
    pub fn empty() -> Self {
        Variables {
            variables: HashSet::new(),
            telemetry_variables: HashSet::new(),
        }
    }

    pub fn from_statement(stmt: &SingleStatement) -> Self {
        let mut vs = Variables::empty();
        vs.stmt(stmt);
        vs
    }

    fn stmt(&mut self, stmt: &SingleStatement) {
        use SingleStatement::*;
        match stmt {
            Call(_) => (),
            Wait(w) => self.expr(&w.condition),
            Assert(c) => self.expr(&c.condition),
            AssertEq(c) => {
                self.expr(&c.left);
                self.expr(&c.right);
                if let Some(t) = &c.tolerance {
                    self.expr(t)
                }
            }

            Command(cmd) => {
                for arg in &cmd.args {
                    self.expr(arg);
                }
            }
            Let(l) => self.expr(&l.rhs),
            Print(p) => self.expr(&p.arg),
            Set(p) => self.expr(&p.expr),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Variable(VariablePath { raw }) => {
                self.variables.insert(raw.to_owned());
            }
            Expr::TlmRef(VariablePath { raw }) => {
                self.telemetry_variables.insert(raw.to_owned());
            }
            Expr::Literal(_) => {}
            Expr::UnOp(_, expr) => self.expr(expr),
            Expr::BinOp(_, lhs, rhs) => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::FunCall(fun, args) => {
                self.expr(fun);
                for arg in args {
                    self.expr(arg);
                }
            }
        }
    }
}
