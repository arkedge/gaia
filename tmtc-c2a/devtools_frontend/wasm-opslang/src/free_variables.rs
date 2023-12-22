use opslang_syn::typedef::*;
use std::collections::HashSet;

struct Scanner<'bound> {
    bound: &'bound HashSet<&'bound str>,
    set: HashSet<String>,
}

impl<'bound> Scanner<'bound> {
    fn new(bound: &'bound HashSet<&'bound str>) -> Self {
        Scanner {
            bound,
            set: HashSet::new(),
        }
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
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Variable(VariablePath { raw }) => {
                if !self.bound.contains(raw.as_str()) {
                    self.set.insert(raw.to_owned());
                }
            }
            Expr::TlmRef(VariablePath { raw }) => {
                //FIXME: prefixing with "$" is a dirty hack
                let path = format!("${}", raw);
                if !self.bound.contains(&path[..]) {
                    self.set.insert(path);
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

pub fn stmt(stmt: &SingleStatement, bounded: &HashSet<&str>) -> HashSet<String> {
    let mut scanner = Scanner::new(bounded);
    scanner.stmt(stmt);
    scanner.set
}
