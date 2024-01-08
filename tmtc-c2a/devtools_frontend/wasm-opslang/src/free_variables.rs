use opslang_syn::typedef::*;
use std::collections::HashSet;

struct Scanner<'bound, 'input> {
    bound: &'bound HashSet<&'bound str>,
    set: HashSet<&'input str>,
}

impl<'bound, 'input> Scanner<'bound, 'input> {
    fn new(bound: &'bound HashSet<&'bound str>) -> Self {
        Scanner {
            bound,
            set: HashSet::new(),
        }
    }

    fn stmt(&mut self, stmt: &'input ReservedControl<'input>) {
        use ReservedControl::*;
        match stmt {
            Call(_) => (),
            WaitSec(ws) => self.expr(&ws.sec),
            WaitUntil(wu) => self.expr(&wu.condition),
            CheckValue(cv) => self.expr(&cv.condition),
            Command(cmd) => {
                for arg in &cmd.args {
                    self.expr(arg);
                }
            }
            Let(l) => self.expr(&l.rhs),
            Get(_) => (), // TODO: implement
        }
    }

    fn expr(&mut self, expr: &'input Expr<'input>) {
        match expr {
            Expr::Variable(VariablePath { raw }) => {
                if !self.bound.contains(raw) {
                    self.set.insert(raw);
                }
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

pub fn stmt<'input>(
    stmt: &'input ReservedControl<'input>,
    bounded: &HashSet<&str>,
) -> HashSet<&'input str> {
    let mut scanner = Scanner::new(bounded);
    scanner.stmt(stmt);
    scanner.set
}
