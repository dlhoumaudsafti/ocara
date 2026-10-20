//! Couverture par ligne (`--coverage`) : chaque instruction source lowered
//! est un point `(fichier, ligne)` ; son exécution appelle `__cov_hit(n)`.
//! En tête de `main`, `__cov_init` puis `__cov_point` enregistrent toute la
//! table auprès du runtime (runtime/src/coverage.rs), qui écrit à la sortie
//! du programme les points atteints dans le fichier `$OCARA_COVERAGE_FILE`.
//! Une instruction jamais atteignable (après un `return`) reste un point,
//! jamais marqué.

use std::collections::HashMap;

use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::parsing::token::Span;

pub fn mark_stmt(builder: &mut LowerBuilder, span: &Span) {
    // Instruction synthétisée sans position (ligne 0) : pas un point.
    if span.line == 0 { return; }
    let Some(points) = builder.module.coverage.as_mut() else { return };
    let file = span.file.clone().unwrap_or_else(|| builder.module.source_file.clone());
    points.push((file, span.line));
    let index = (points.len() - 1) as i64;
    if builder.is_terminated() {
        return;
    }
    let value = builder.new_value();
    builder.emit(Inst::ConstInt { dest: value.clone(), value: index });
    builder.emit(Inst::Call { dest: None, func: "__cov_hit".into(), args: vec![value], ret_ty: IrType::Void });
}

/// Enregistre la table des points en tête de `main`.
pub fn register_points(module: &mut IrModule) {
    let Some(points) = module.coverage.clone() else { return };
    let mut interned: HashMap<&str, u32> = HashMap::new();
    let files: Vec<u32> = points.iter()
        .map(|(file, _)| *interned.entry(file.as_str()).or_insert_with(|| module.intern_string(file)))
        .collect();
    let Some(main) = module.functions.iter_mut().find(|f| f.name == "main") else { return };
    let mut prologue = Vec::with_capacity(points.len() * 4 + 2);
    let count = main.new_value();
    prologue.push(Inst::ConstInt { dest: count.clone(), value: points.len() as i64 });
    prologue.push(Inst::Call { dest: None, func: "__cov_init".into(), args: vec![count], ret_ty: IrType::Void });
    for (index, ((_, line), file)) in points.iter().zip(files).enumerate() {
        let (idx, path, at) = (main.new_value(), main.new_value(), main.new_value());
        prologue.push(Inst::ConstInt { dest: idx.clone(), value: index as i64 });
        prologue.push(Inst::ConstStr { dest: path.clone(), idx: file });
        prologue.push(Inst::ConstInt { dest: at.clone(), value: *line as i64 });
        prologue.push(Inst::Call { dest: None, func: "__cov_point".into(), args: vec![idx, path, at], ret_ty: IrType::Void });
    }
    main.blocks[0].insts.splice(0..0, prologue);
}

#[cfg(test)]
mod tests {
    use super::register_points;
    use crate::ir::inst::Inst;
    use crate::lower::builder::lower_program_with;
    use crate::parsing::{lexer::Lexer, parser::Parser};

    fn lower(src: &str, coverage: bool) -> crate::ir::module::IrModule {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = Parser::new(tokens).parse_program().expect("parse");
        lower_program_with(&program, "<test>", coverage)
    }

    const SRC: &str = "function f(n:int): int {\n    if n smaller 0 {\n        return 0\n    }\n    return n\n}\nfunction main(): int {\n    return f(1)\n}\n";

    fn calls<'a>(module: &'a crate::ir::module::IrModule, func: &str, callee: &str) -> usize {
        module.functions.iter().find(|f| f.name == func).expect("fonction")
            .blocks.iter().flat_map(|b| &b.insts)
            .filter(|i| matches!(i, Inst::Call { func, .. } if func == callee))
            .count()
    }

    #[test]
    fn every_statement_is_a_point_with_its_line() {
        let module = lower(SRC, true);
        let lines: Vec<usize> = module.coverage.as_ref().expect("points").iter().map(|(_, l)| *l).collect();
        assert_eq!(lines, vec![2, 3, 5, 8]);
        assert_eq!(calls(&module, "f", "__cov_hit"), 3);
    }

    #[test]
    fn main_registers_the_table_first() {
        let mut module = lower(SRC, true);
        register_points(&mut module);
        let main = module.functions.iter().find(|f| f.name == "main").expect("main");
        assert!(matches!(&main.blocks[0].insts[1], Inst::Call { func, .. } if func == "__cov_init"));
        assert_eq!(calls(&module, "main", "__cov_point"), 4);
    }

    #[test]
    fn nothing_is_instrumented_without_the_flag() {
        let module = lower(SRC, false);
        assert!(module.coverage.is_none());
        assert_eq!(calls(&module, "f", "__cov_hit"), 0);
    }
}
