// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Basic html highlighting functionality
//!
//! This module uses libsyntax's lexer to provide token-based highlighting for
//! the HTML documentation generated by rustdoc.

use html::escape::Escape;

use std::old_io;
use syntax::parse::lexer;
use syntax::parse::token;
use syntax::parse;

/// Highlights some source code, returning the HTML output.
pub fn highlight(src: &str, class: Option<&str>, id: Option<&str>) -> String {
    debug!("highlighting: ================\n{}\n==============", src);
    let sess = parse::new_parse_sess();
    let fm = parse::string_to_filemap(&sess,
                                      src.to_string(),
                                      "<stdin>".to_string());

    let mut out = Vec::new();
    doit(&sess,
         lexer::StringReader::new(&sess.span_diagnostic, fm),
         class,
         id,
         &mut out).unwrap();
    String::from_utf8_lossy(&out[]).into_owned()
}

/// Exhausts the `lexer` writing the output into `out`.
///
/// The general structure for this method is to iterate over each token,
/// possibly giving it an HTML span with a class specifying what flavor of token
/// it's used. All source code emission is done as slices from the source map,
/// not from the tokens themselves, in order to stay true to the original
/// source.
fn doit(sess: &parse::ParseSess, mut lexer: lexer::StringReader,
        class: Option<&str>, id: Option<&str>,
        out: &mut Writer) -> old_io::IoResult<()> {
    use syntax::parse::lexer::Reader;

    try!(write!(out, "<pre "));
    match id {
        Some(id) => try!(write!(out, "id='{}' ", id)),
        None => {}
    }
    try!(write!(out, "class='rust {}'>\n", class.unwrap_or("")));
    let mut is_attribute = false;
    let mut is_macro = false;
    let mut is_macro_nonterminal = false;
    loop {
        let next = lexer.next_token();

        let snip = |sp| sess.span_diagnostic.cm.span_to_snippet(sp).unwrap();

        if next.tok == token::Eof { break }

        let klass = match next.tok {
            token::Whitespace => {
                try!(write!(out, "{}", Escape(&snip(next.sp))));
                continue
            },
            token::Comment => {
                try!(write!(out, "<span class='comment'>{}</span>",
                            Escape(&snip(next.sp))));
                continue
            },
            token::Shebang(s) => {
                try!(write!(out, "{}", Escape(s.as_str())));
                continue
            },
            // If this '&' token is directly adjacent to another token, assume
            // that it's the address-of operator instead of the and-operator.
            // This allows us to give all pointers their own class (`Box` and
            // `@` are below).
            token::BinOp(token::And) if lexer.peek().sp.lo == next.sp.hi => "kw-2",
            token::At | token::Tilde => "kw-2",

            // consider this as part of a macro invocation if there was a
            // leading identifier
            token::Not if is_macro => { is_macro = false; "macro" }

            // operators
            token::Eq | token::Lt | token::Le | token::EqEq | token::Ne | token::Ge | token::Gt |
                token::AndAnd | token::OrOr | token::Not | token::BinOp(..) | token::RArrow |
                token::BinOpEq(..) | token::FatArrow => "op",

            // miscellaneous, no highlighting
            token::Dot | token::DotDot | token::DotDotDot | token::Comma | token::Semi |
                token::Colon | token::ModSep | token::LArrow | token::OpenDelim(_) |
                token::CloseDelim(token::Brace) | token::CloseDelim(token::Paren) |
                token::Question => "",
            token::Dollar => {
                if lexer.peek().tok.is_ident() {
                    is_macro_nonterminal = true;
                    "macro-nonterminal"
                } else {
                    ""
                }
            }

            // This is the start of an attribute. We're going to want to
            // continue highlighting it as an attribute until the ending ']' is
            // seen, so skip out early. Down below we terminate the attribute
            // span when we see the ']'.
            token::Pound => {
                is_attribute = true;
                try!(write!(out, r"<span class='attribute'>#"));
                continue
            }
            token::CloseDelim(token::Bracket) => {
                if is_attribute {
                    is_attribute = false;
                    try!(write!(out, "]</span>"));
                    continue
                } else {
                    ""
                }
            }

            token::Literal(lit, _suf) => {
                match lit {
                    // text literals
                    token::Byte(..) | token::Char(..) |
                        token::Binary(..) | token::BinaryRaw(..) |
                        token::Str_(..) | token::StrRaw(..) => "string",

                    // number literals
                    token::Integer(..) | token::Float(..) => "number",
                }
            }

            // keywords are also included in the identifier set
            token::Ident(ident, _is_mod_sep) => {
                match token::get_ident(ident).get() {
                    "ref" | "mut" => "kw-2",

                    "self" => "self",
                    "false" | "true" => "boolval",

                    "Option" | "Result" => "prelude-ty",
                    "Some" | "None" | "Ok" | "Err" => "prelude-val",

                    _ if next.tok.is_any_keyword() => "kw",
                    _ => {
                        if is_macro_nonterminal {
                            is_macro_nonterminal = false;
                            "macro-nonterminal"
                        } else if lexer.peek().tok == token::Not {
                            is_macro = true;
                            "macro"
                        } else {
                            "ident"
                        }
                    }
                }
            }

            // Special macro vars are like keywords
            token::SpecialVarNt(_) => "kw-2",

            token::Lifetime(..) => "lifetime",
            token::DocComment(..) => "doccomment",
            token::Underscore | token::Eof | token::Interpolated(..) |
                token::MatchNt(..) | token::SubstNt(..) => "",
        };

        // as mentioned above, use the original source code instead of
        // stringifying this token
        let snip = sess.span_diagnostic.cm.span_to_snippet(next.sp).unwrap();
        if klass == "" {
            try!(write!(out, "{}", Escape(&snip)));
        } else {
            try!(write!(out, "<span class='{}'>{}</span>", klass,
                          Escape(&snip)));
        }
    }

    write!(out, "</pre>\n")
}
