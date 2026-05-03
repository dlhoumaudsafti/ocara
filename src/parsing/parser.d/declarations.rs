/// Parsing des déclarations (fonctions, classes, generics, modules, enums, interfaces, paramètres)

use crate::parsing::ast::*;
use crate::parsing::token::TokenKind;
use super::types::{Parser, ParseError, ParseResult};

impl Parser {
    // ── Paramètres ───────────────────────────────────────────────────────────

    pub(super) fn parse_param_list(&mut self) -> ParseResult<Vec<Param>> {
        let mut params = Vec::new();
        if self.check_exact(&TokenKind::RParen) {
            return Ok(params);
        }
        params.push(self.parse_param()?);
        while self.check_exact(&TokenKind::Comma) {
            self.advance();
            params.push(self.parse_param()?);
        }
        
        // Vérification 1 : si variadic présent, doit être le dernier paramètre
        if params.len() > 1 {
            for (i, param) in params.iter().enumerate() {
                if param.is_variadic && i != params.len() - 1 {
                    return Err(ParseError {
                        message: "le paramètre variadic doit être le dernier paramètre".to_string(),
                        span: param.span.clone(),
                    });
                }
            }
        }
        
        // Vérification 2 : les paramètres avec valeur par défaut doivent être après les paramètres obligatoires
        // (sauf si le dernier paramètre est variadic, qui peut suivre des paramètres avec default)
        let has_variadic_at_end = params.last().map(|p| p.is_variadic).unwrap_or(false);
        let params_to_check = if has_variadic_at_end {
            &params[..params.len() - 1]  // exclure le variadic de la vérification
        } else {
            &params[..]
        };
        
        let mut found_default = false;
        for param in params_to_check {
            if param.default_value.is_some() {
                found_default = true;
            } else if found_default {
                return Err(ParseError {
                    message: format!(
                        "le paramètre '{}' sans valeur par défaut ne peut pas suivre un paramètre avec valeur par défaut",
                        param.name
                    ),
                    span: param.span.clone(),
                });
            }
        }
        
        Ok(params)
    }

    fn parse_param(&mut self) -> ParseResult<Param> {
        let span = self.span();
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        
        // Vérifier si c'est un paramètre variadic
        let is_variadic = self.check_exact(&TokenKind::Variadic);
        if is_variadic {
            self.advance(); // consommer 'variadic'
            self.eat(&TokenKind::Lt)?; // '<'
            let ty = self.parse_type()?;
            self.eat(&TokenKind::Gt)?; // '>'
            
            // Les paramètres variadic ne peuvent pas avoir de valeur par défaut
            if self.check_exact(&TokenKind::Eq) {
                return Err(ParseError {
                    message: "un paramètre variadic ne peut pas avoir de valeur par défaut".to_string(),
                    span,
                });
            }
            
            Ok(Param { name, ty, is_variadic: true, default_value: None, span })
        } else {
            let ty = self.parse_type()?;
            
            // Vérifier s'il y a une valeur par défaut (= expression)
            let default_value = if self.check_exact(&TokenKind::Eq) {
                self.advance(); // consommer '='
                Some(self.parse_expr()?)
            } else {
                None
            };
            
            Ok(Param { name, ty, is_variadic: false, default_value, span })
        }
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    pub(super) fn parse_func(&mut self) -> ParseResult<FuncDecl> {
        let span = self.span();
        let is_async = self.check_exact(&TokenKind::Async);
        if is_async { self.advance(); }
        self.eat(&TokenKind::Function)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        let body = self.parse_block()?;
        Ok(FuncDecl { name, params, ret_ty, body, is_async, span })
    }

    fn parse_method_decl(&mut self) -> ParseResult<FuncDecl> {
        let span = self.span();
        let is_async = self.check_exact(&TokenKind::Async);
        if is_async { self.advance(); }
        self.eat(&TokenKind::Method)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        let body = self.parse_block()?;
        Ok(FuncDecl { name, params, ret_ty, body, is_async, span })
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    pub(super) fn parse_class(&mut self) -> ParseResult<ClassDecl> {
        let span = self.span();
        self.eat(&TokenKind::Class)?;
        let (name, _) = self.eat_ident()?;

        let extends = if self.check_exact(&TokenKind::Extends) {
            self.advance();
            Some(self.eat_ident()?.0)
        } else {
            None
        };

        let mut modules = Vec::new();
        if self.check_exact(&TokenKind::Modules) {
            self.advance();
            modules.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                modules.push(self.eat_ident()?.0);
            }
        }

        let mut implements = Vec::new();
        if self.check_exact(&TokenKind::Implements) {
            self.advance();
            implements.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                implements.push(self.eat_ident()?.0);
            }
        }

        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            members.push(self.parse_class_member()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(ClassDecl { name, extends, modules, implements, members, span })
    }

    fn parse_class_member(&mut self) -> ParseResult<ClassMember> {
        let span = self.span();

        // Constructeur : `init(...) { }`
        if self.check_exact(&TokenKind::Init) {
            self.advance();
            self.eat(&TokenKind::LParen)?;
            let params = self.parse_param_list()?;
            self.eat(&TokenKind::RParen)?;
            let body = self.parse_block()?;
            return Ok(ClassMember::Constructor { params, body, span });
        }

        // Visibilité obligatoire en premier : `public|private|protected`
        let vis = self.parse_visibility()?;

        // Modificateur statique optionnel après la visibilité : `public static method ...`
        let is_static = if self.check_exact(&TokenKind::Static) {
            self.advance();
            true
        } else {
            false
        };

        // Modificateur async optionnel : `public [static] async method ...`
        let _is_async_member = if self.check_exact(&TokenKind::Async) {
            self.advance();
            true
        } else {
            false
        };

        // `static` n'est autorisé que sur les méthodes
        if is_static && !self.check_exact(&TokenKind::Method) {
            return Err(ParseError::new(
                "'static' n'est autorisé que sur les méthodes (method)",
                self.span(),
            ));
        }

        // Constante de classe : `public const NAME:T = value`
        if self.check_exact(&TokenKind::Const) {
            self.advance();
            let (name, _) = self.eat_ident()?;
            self.eat(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            self.eat(&TokenKind::Eq)?;
            let value = self.parse_expr()?;
            return Ok(ClassMember::Const { vis, name, ty, value, span });
        }

        // Méthode : `public [static] method foo(...): T { }`
        if self.check_exact(&TokenKind::Method) {
            let decl = self.parse_method_decl()?;
            return Ok(ClassMember::Method { vis, is_static, decl, span });
        }

        // Champ : `vis property name:T`
        let _mutable = match self.peek_kind() {
            TokenKind::Property => { self.advance(); true }
            TokenKind::Var => {
                return Err(ParseError::new(
                    "'var' est interdit sur un champ de classe : utilisez 'property'".to_string(),
                    self.span(),
                ));
            }
            TokenKind::Scoped => {
                return Err(ParseError::new(
                    "'scoped' est interdit sur un champ de classe : un champ vit aussi longtemps que l'objet, utilisez 'property'".to_string(),
                    self.span(),
                ));
            }
            other => return Err(ParseError::new(
                format!("expected 'property', 'const' or 'method', found {:?}", other),
                self.span(),
            )),
        };
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::Colon)?;
        let ty = self.parse_type()?;
        Ok(ClassMember::Field { vis, mutable: true, name, ty, span })
    }

    fn parse_visibility(&mut self) -> ParseResult<Visibility> {
        let span = self.span();
        match self.peek_kind() {
            TokenKind::Public    => { self.advance(); Ok(Visibility::Public)    }
            TokenKind::Private   => { self.advance(); Ok(Visibility::Private)   }
            TokenKind::Protected => { self.advance(); Ok(Visibility::Protected) }
            other => Err(ParseError::new(
                format!("expected visibility (public/private/protected), found {:?}", other),
                span,
            )),
        }
    }

    // ── Générique ────────────────────────────────────────────────────────────

    pub(super) fn parse_generic(&mut self) -> ParseResult<GenericDecl> {
        let span = self.span();
        self.eat(&TokenKind::Generic)?;
        let (name, _) = self.eat_ident()?;

        // Parser les paramètres de type : <T, K, V = string>
        self.eat(&TokenKind::Lt)?;
        let mut type_params = Vec::new();
        
        loop {
            let pspan = self.span();
            let (pname, _) = self.eat_ident()?;
            
            // Vérifier PascalCase (première lettre majuscule)
            if !pname.chars().next().unwrap().is_uppercase() {
                return Err(ParseError::new(
                    format!("generic type parameter '{}' must be in PascalCase (start with uppercase)", pname),
                    pspan,
                ));
            }
            
            // Valeur par défaut optionnelle : T = string
            let default = if self.check_exact(&TokenKind::Eq) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            
            type_params.push(TypeParam { name: pname, default, span: pspan });
            
            if !self.check_exact(&TokenKind::Comma) {
                break;
            }
            self.advance(); // ','
        }
        
        self.eat(&TokenKind::Gt)?;

        // Extends avec arguments de type optionnels : extends Base<T>
        let mut extends = None;
        let mut extends_args = Vec::new();
        if self.check_exact(&TokenKind::Extends) {
            self.advance();
            extends = Some(self.eat_ident()?.0);
            
            // Arguments de type optionnels pour extends
            if self.check_exact(&TokenKind::Lt) {
                self.advance();
                extends_args.push(self.parse_type()?);
                while self.check_exact(&TokenKind::Comma) {
                    self.advance();
                    extends_args.push(self.parse_type()?);
                }
                self.eat(&TokenKind::Gt)?;
            }
        }

        // Modules
        let mut modules = Vec::new();
        if self.check_exact(&TokenKind::Modules) {
            self.advance();
            modules.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                modules.push(self.eat_ident()?.0);
            }
        }

        // Implements
        let mut implements = Vec::new();
        if self.check_exact(&TokenKind::Implements) {
            self.advance();
            implements.push(self.eat_ident()?.0);
            while self.check_exact(&TokenKind::Comma) {
                self.advance();
                implements.push(self.eat_ident()?.0);
            }
        }

        // Membres (comme une classe)
        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            members.push(self.parse_class_member()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(GenericDecl { 
            name, 
            type_params, 
            extends, 
            extends_args,
            modules, 
            implements, 
            members, 
            span 
        })
    }

    // ── Module (mixin) ───────────────────────────────────────────────────────

    pub(super) fn parse_module(&mut self) -> ParseResult<ModuleDecl> {
        let span = self.span();
        self.eat(&TokenKind::Module)?;
        let (name, _) = self.eat_ident()?;

        self.eat(&TokenKind::LBrace)?;
        let mut members = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            members.push(self.parse_class_member()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(ModuleDecl { name, members, span })
    }

    // ── Enum ─────────────────────────────────────────────────────────────────

    pub(super) fn parse_enum(&mut self) -> ParseResult<EnumDecl> {
        let span = self.span();
        self.eat(&TokenKind::Enum)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            let vspan = self.span();
            let (vname, _) = self.eat_ident()?;
            // Valeur explicite optionnelle : `Variant = 42`
            let value = if self.check_exact(&TokenKind::Eq) {
                self.advance();
                let vspan2 = self.span();
                match self.peek_kind().clone() {
                    TokenKind::LitInt(n) => {
                        self.advance();
                        Some(n)
                    }
                    other => {
                        return Err(ParseError::new(
                            format!("enum value must be an integer literal, found {:?}", other),
                            vspan2,
                        ));
                    }
                }
            } else {
                None
            };
            variants.push(EnumVariant { name: vname, value, span: vspan });
            // Virgule optionnelle entre variantes
            if self.check_exact(&TokenKind::Comma) {
                self.advance();
            }
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(EnumDecl { name, variants, span })
    }

    // ── Interface ────────────────────────────────────────────────────────────

    pub(super) fn parse_interface(&mut self) -> ParseResult<InterfaceDecl> {
        let span = self.span();
        self.eat(&TokenKind::Interface)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) {
            methods.push(self.parse_interface_method()?);
        }
        self.eat(&TokenKind::RBrace)?;

        Ok(InterfaceDecl { name, methods, span })
    }

    fn parse_interface_method(&mut self) -> ParseResult<InterfaceMethod> {
        let span = self.span();
        self.eat(&TokenKind::Method)?;
        let (name, _) = self.eat_ident()?;
        self.eat(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.eat(&TokenKind::RParen)?;
        self.eat(&TokenKind::Colon)?;
        let ret_ty = self.parse_type()?;
        Ok(InterfaceMethod { name, params, ret_ty, span })
    }
}
