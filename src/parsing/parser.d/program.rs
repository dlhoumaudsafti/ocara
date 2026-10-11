/// Parsing du programme principal

use crate::parsing::ast::{
    ClassDecl, ConstDecl, EnumDecl, FuncDecl, GenericDecl, ImportDecl, InterfaceDecl, ModuleDecl, Program,
    RuntimeBlock, RuntimeImport,
};
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};
use super::when::Variant;

impl Parser {
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut program = Program::new();

        // Parser le namespace optionnel en début de fichier
        if self.check_exact(&TokenKind::Namespace) {
            self.advance(); // consommer 'namespace'
            
            // namespace . ou namespace identifier[.identifier]*
            if self.check_exact(&TokenKind::Dot) {
                self.advance(); // consommer '.'
                program.namespace = Some(".".to_string()); // namespace racine explicite
            } else {
                // Doit être un identifiant (ou plusieurs séparés par des points)
                let tok = self.current().clone();
                if let TokenKind::Ident(_) = tok.kind {
                    let mut ns_parts = vec![tok.lexeme.clone()];
                    self.advance();
                    
                    // Lire les parties supplémentaires du namespace (ex: configs.components)
                    while self.check_exact(&TokenKind::Dot) {
                        self.advance(); // consommer '.'
                        let tok = self.current().clone();
                        if let TokenKind::Ident(_) = tok.kind {
                            ns_parts.push(tok.lexeme.clone());
                            self.advance();
                        } else {
                            return Err(ParseError::new(
                                "expected identifier after '.' in namespace declaration".to_string(),
                                self.span(),
                            ));
                        }
                    }
                    
                    program.namespace = Some(ns_parts.join("."));
                } else {
                    return Err(ParseError::new(
                        "expected '.' or identifier after 'namespace'".to_string(),
                        self.span(),
                    ));
                }
            }
        }

        let mut items: Vec<(Variant, TopItem)> = Vec::new();
        while !self.check_exact(&TokenKind::Eof) {
            let start = self.pos;
            match self.parse_top_level() {
                Ok(item) => items.push(item),
                Err(e) => {
                    if let Err(e) = self.recover_top(e, start) {
                        if !self.recover { return Err(e); }
                        self.errors.push(e);
                        break;
                    }
                }
            }
        }
        let (variants, items): (Vec<Variant>, Vec<TopItem>) = items.into_iter().unzip();
        let keep = self.select_or_note(&variants)?;
        for (item, keep) in items.into_iter().zip(keep) {
            if keep {
                item.push_into(&mut program);
            }
        }

        program.type_refs = std::mem::take(&mut self.type_refs);
        Ok(program)
    }

    /// Déclaration de premier niveau, précédée de ses clauses `when`.
    fn parse_top_level(&mut self) -> ParseResult<(Variant, TopItem)> {
        let clause = self.parse_when_clause()?;
        let span = self.span();
        let item = match self.peek_kind().clone() {
            TokenKind::Import => TopItem::Import(self.parse_import()?),
            TokenKind::Runtime => TopItem::RuntimeImport(self.parse_runtime_import()?),
            TokenKind::Init | TokenKind::Main | TokenKind::Error
            | TokenKind::Success | TokenKind::Exit => TopItem::RuntimeBlock(self.parse_runtime_block()?),
            TokenKind::Const => TopItem::Const(self.parse_const_decl()?),
            TokenKind::Module => TopItem::Module(self.parse_module()?),
            TokenKind::Enum => TopItem::Enum(self.parse_enum()?),
            TokenKind::Class => TopItem::Class(self.parse_class()?),
            TokenKind::Struct => TopItem::Class(self.parse_struct()?),
            TokenKind::Generic => TopItem::Generic(self.parse_generic()?),
            TokenKind::Interface => TopItem::Interface(self.parse_interface()?),
            // `async function ...` au niveau du module
            TokenKind::Function | TokenKind::Async => TopItem::Function(self.parse_func()?),
            other => {
                return Err(ParseError::new(
                    format!("unexpected top-level declaration: {:?}", other),
                    self.span(),
                ))
            }
        };
        Ok((Variant { key: item.key(), clause, span }, item))
    }

    /// Sélection des variantes ; en mode tolérant, une erreur est notée et
    /// tout est gardé.
    pub(super) fn select_or_note(&mut self, variants: &[Variant]) -> ParseResult<Vec<bool>> {
        match self.select_variants(variants) {
            Ok(keep) => Ok(keep),
            Err(e) if self.recover => {
                self.errors.push(e);
                Ok(vec![true; variants.len()])
            }
            Err(e) => Err(e),
        }
    }
}

enum TopItem {
    Import(ImportDecl),
    RuntimeImport(RuntimeImport),
    RuntimeBlock(RuntimeBlock),
    Const(ConstDecl),
    Module(ModuleDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Generic(GenericDecl),
    Interface(InterfaceDecl),
    Function(FuncDecl),
}

impl TopItem {
    /// Clé des variantes : même genre et même nom (types partagent un nom).
    fn key(&self) -> String {
        match self {
            TopItem::Import(i) => format!("import:{}:{:?}:{:?}", i.path.join("."), i.file_path, i.alias),
            TopItem::RuntimeImport(r) => match &r.kind {
                Some(kind) => format!("runtime:{:?}", kind),
                None => format!("runtime:{}", r.path.join(".")),
            },
            TopItem::RuntimeBlock(b) => format!("block:{:?}", b.kind),
            TopItem::Const(c) => format!("const:{}", c.name),
            TopItem::Module(m) => format!("type:{}", m.name),
            TopItem::Enum(e) => format!("type:{}", e.name),
            TopItem::Class(c) => format!("type:{}", c.name),
            TopItem::Generic(g) => format!("type:{}", g.name),
            TopItem::Interface(i) => format!("type:{}", i.name),
            TopItem::Function(f) => format!("fn:{}", f.name),
        }
    }

    fn push_into(self, program: &mut Program) {
        match self {
            TopItem::Import(i) => program.imports.push(i),
            TopItem::RuntimeImport(r) => program.runtime_imports.push(r),
            TopItem::RuntimeBlock(b) => program.runtime_blocks.push(b),
            TopItem::Const(c) => program.consts.push(c),
            TopItem::Module(m) => program.modules.push(m),
            TopItem::Enum(e) => program.enums.push(e),
            TopItem::Class(c) => program.classes.push(c),
            TopItem::Generic(g) => program.generics.push(g),
            TopItem::Interface(i) => program.interfaces.push(i),
            TopItem::Function(f) => program.functions.push(f),
        }
    }
}
