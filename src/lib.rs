mod local_export_strip;
mod utils;

use local_export_strip::LocalExportStrip;
use swc_core::{
    common::{collections::AHashSet, util::take::Take, Mark, DUMMY_SP},
    ecma::{
        ast::*,
        utils::{
            member_expr, private_ident, quote_ident, quote_str, ExprFactory, IntoIndirectCall,
        },
        visit::{as_folder, noop_visit_mut_type, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::{plugin_transform, proxies::TransformPluginProgramMetadata},
};
use utils::emit_export_stmts;

#[derive(Debug)]
pub struct TransformVisitor {
    unresolved_mark: Mark,

    export_decl_id: AHashSet<Id>,
}

impl VisitMut for TransformVisitor {
    noop_visit_mut_type!();

    fn visit_mut_script(&mut self, _: &mut Script) {
        // skip
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        let mut strip = LocalExportStrip::default();
        n.visit_mut_with(&mut strip);

        let LocalExportStrip {
            has_export_assign,
            export,
            export_all,
            export_decl_id,
            ..
        } = strip;

        self.export_decl_id = export_decl_id;

        let mut stmts: Vec<ModuleItem> = Vec::with_capacity(n.body.len() + 1);

        if !has_export_assign && !export.is_empty() {
            // keep module env
            stmts.push(ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(
                NamedExport::dummy(),
            )));

            let exports = self.exports();

            let export_obj_prop_list = export.into_iter().map(Into::into).collect();

            stmts.extend(
                emit_export_stmts(exports, export_obj_prop_list)
                    .into_iter()
                    .map(Into::into),
            );

            if !self.export_decl_id.is_empty() {
                n.visit_mut_children_with(self);
            }
        }

        stmts.extend(export_all.into_iter().map(|id| self.export_all(id)));

        stmts.extend(n.body.take());

        n.body = stmts;
    }

    fn visit_mut_prop(&mut self, n: &mut Prop) {
        match n {
            Prop::Shorthand(ref_ident) => {
                if self.export_decl_id.contains(&ref_ident.to_id()) {
                    *n = KeyValueProp {
                        key: ref_ident.clone().into(),
                        value: Box::new(self.exports().make_member(ref_ident.take())),
                    }
                    .into()
                }
            }
            _ => n.visit_mut_children_with(self),
        }
    }

    fn visit_mut_expr(&mut self, n: &mut Expr) {
        match n {
            Expr::Ident(ref_ident) => {
                if self.export_decl_id.contains(&ref_ident.to_id()) {
                    *n = self.exports().make_member(ref_ident.take());
                }
            }

            _ => n.visit_mut_children_with(self),
        };
    }

    fn visit_mut_tagged_tpl(&mut self, n: &mut TaggedTpl) {
        let is_indirect = n
            .tag
            .as_ident()
            .map(|ident| self.export_decl_id.contains(&ident.to_id()))
            .unwrap_or_default();

        n.visit_mut_children_with(self);

        if is_indirect {
            *n = n.take().into_indirect()
        }
    }

    fn visit_mut_callee(&mut self, n: &mut Callee) {
        match n {
            Callee::Expr(e) if e.is_ident() => {
                let is_indirect_callee = e
                    .as_ident()
                    .map(|ident| self.export_decl_id.contains(&ident.to_id()))
                    .unwrap_or_default();

                e.visit_mut_with(self);

                if is_indirect_callee {
                    *n = n.take().into_indirect()
                }
            }

            _ => n.visit_mut_children_with(self),
        }
    }
}

impl TransformVisitor {
    pub fn new(unresolved_mark: Mark) -> Self {
        Self {
            unresolved_mark,
            export_decl_id: Default::default(),
        }
    }

    fn exports(&self) -> Ident {
        quote_ident!(DUMMY_SP.apply_mark(self.unresolved_mark), "exports")
    }

    /// ```JavaScript
    /// Object.keys(_mod).forEach(function (key) {
    ///     if (key === "default" || key === "__esModule") return;
    ///     if (key in exports && exports[key] === _mod[key]) return;
    ///     exports[key] = _mod[key];
    /// });
    /// ```
    fn export_all(&self, id: Id) -> ModuleItem {
        let mod_name = Ident::from(id);
        let key = private_ident!("key");

        member_expr!(DUMMY_SP, Object.keys)
            .as_call(DUMMY_SP, vec![mod_name.clone().as_arg()])
            .make_member(quote_ident!("forEach"))
            .as_call(
                DUMMY_SP,
                vec![Function {
                    params: vec![key.clone().into()],
                    decorators: vec![],
                    span: DUMMY_SP,
                    body: Some(BlockStmt {
                        span: DUMMY_SP,
                        stmts: vec![
                            // if (key === "default" || key === "__esModule") return;
                            IfStmt {
                                span: DUMMY_SP,
                                test: BinExpr {
                                    span: DUMMY_SP,
                                    op: op!("||"),
                                    left: BinExpr {
                                        span: DUMMY_SP,
                                        op: op!("==="),
                                        left: key.clone().into(),
                                        right: quote_str!("default").into(),
                                    }
                                    .into(),
                                    right: BinExpr {
                                        span: DUMMY_SP,
                                        op: op!("==="),
                                        left: key.clone().into(),
                                        right: quote_str!("__esModule").into(),
                                    }
                                    .into(),
                                }
                                .into(),
                                cons: Box::new(
                                    ReturnStmt {
                                        span: DUMMY_SP,
                                        arg: None,
                                    }
                                    .into(),
                                ),
                                alt: None,
                            }
                            .into(),
                            // if (key in exports && exports[key] === _mod[key]) return;
                            IfStmt {
                                span: DUMMY_SP,
                                test: BinExpr {
                                    span: DUMMY_SP,
                                    op: op!("&&"),
                                    left: BinExpr {
                                        span: DUMMY_SP,
                                        op: op!("in"),
                                        left: key.clone().into(),
                                        right: self.exports().into(),
                                    }
                                    .into(),
                                    right: BinExpr {
                                        span: DUMMY_SP,
                                        op: op!("==="),
                                        left: self
                                            .exports()
                                            .computed_member(quote_ident!("key"))
                                            .into(),
                                        right: mod_name
                                            .clone()
                                            .computed_member(quote_ident!("key"))
                                            .into(),
                                    }
                                    .into(),
                                }
                                .into(),
                                cons: Box::new(
                                    ReturnStmt {
                                        span: DUMMY_SP,
                                        arg: None,
                                    }
                                    .into(),
                                ),
                                alt: None,
                            }
                            .into(),
                            // exports[key] = _mod[key];
                            mod_name
                                .clone()
                                .computed_member(quote_ident!("key"))
                                .make_assign_to(
                                    op!("="),
                                    self.exports()
                                        .computed_member(quote_ident!("key"))
                                        .as_pat_or_expr(),
                                )
                                .into_stmt(),
                        ],
                    }),
                    is_generator: false,
                    is_async: false,
                    type_params: None,
                    return_type: None,
                }
                .as_arg()],
            )
            .into_stmt()
            .into()
    }
}

#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::new(
        metadata.unresolved_mark,
    )))
}
