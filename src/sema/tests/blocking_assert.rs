/// Tests unitaires — `assertBlocksMainThread` / `assertIsBlocking` tranchées
/// à la compilation (src/sema/blocking_assert.rs).
use crate::parsing::ast::{Expr, Literal};
use crate::parsing::{lexer::Lexer, parser::Parser};
use crate::sema::symbols::SymbolTable;
use crate::sema::typecheck::TypeChecker;

const CLASSES: &str = "
class Srv extends HTTPServer {
    public method start(): void {
        self.run()
    }
    public method startInThread(): void {
        const t:Thread = use Thread()
        t.run(nameless(): void { self.run() })
        t.detach()
    }
}
";

/// Verdicts (`ok`, message) des assertions de `body`, dans l'ordre du source.
fn verdicts(body: &str) -> Vec<(bool, String)> {
    let src = format!(
        "import ocara.UnitTest\nimport ocara.HTTPServer\nimport ocara.Thread\nimport ocara.Tauri\n{}\nclass BlockTest extends UnitTest {{\n    public method blockTest(): int {{\n{}\n        return 0\n    }}\n}}\n",
        CLASSES, body
    );
    let tokens = Lexer::new(&src).tokenize().expect("lex ok");
    let program = Parser::new(tokens).parse_program().expect("parse ok");
    let mut symbols = SymbolTable::new();
    for decl in &program.imports { symbols.register_import(decl); }
    for decl in &program.classes { symbols.register_class(decl); }
    let mut checker = TypeChecker::new(&symbols);
    checker.check_program(&program);
    assert!(checker.errors.is_empty(), "{:?}", checker.errors.iter().map(|e| e.message()).collect::<Vec<_>>());
    let mut out: Vec<((usize, usize), (bool, String))> = checker.rewrites.calls.iter().filter_map(|(key, call)| {
        let Expr::StaticCall { method, args, .. } = call else { return None };
        if method != "__staticAssert" { return None; }
        let (Expr::Literal(Literal::Bool(ok), _), Expr::Literal(Literal::String(message), _)) = (&args[0], &args[1]) else { return None };
        Some(((key.1, key.2), (*ok, message.clone())))
    }).collect();
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, v)| v).collect()
}

#[test]
fn event_loop_occupies_the_main_thread() {
    let v = verdicts("        self::assertBlocksMainThread(use Tauri({\"title\": \"t\"}).run())");
    assert_eq!(v, vec![(true, "assertBlocksMainThread: Tauri.run occupies the main thread".to_string())]);
}

#[test]
fn blocking_propagates_through_user_methods() {
    let v = verdicts("        self::assertIsBlocking(use Srv().start())\n        var s:Srv = use Srv()\n        self::assertIsBlocking(s.start())");
    assert_eq!(v.len(), 2);
    assert!(v.iter().all(|(ok, m)| *ok && m.contains("Srv.start → HTTPServer.run")), "{:?}", v);
}

#[test]
fn nameless_bodies_and_plain_waits_are_told_apart() {
    let v = verdicts("        self::assertIsBlocking(use Srv().startInThread())\n        self::assertBlocksMainThread(Thread::sleep(1))\n        self::assertIsBlocking(Thread::sleep(1))");
    assert_eq!(v[0].0, false, "{:?}", v);
    assert_eq!(v[1], (false, "assertBlocksMainThread: Thread.sleep blocks the calling thread but does not occupy the main thread".to_string()));
    assert_eq!(v[2].0, true);
}
