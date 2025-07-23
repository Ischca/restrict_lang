# Restrict Language EBNF (Extended Backus-Naur Form)
Version: v-1.0 candidate

## 1. Lexical Elements

```ebnf
(* Basic Characters *)
any_char        = [\u0001-\u10FFFF] ;  (* any Unicode char except NUL *)
                                       (* lexer implements Unicode categories: L*, N*, etc. *)

(* Whitespace and Comments *)
space           = " " | "\t" | "\r" | "\n" ;
line_comment    = "//" { any_char ^ "\n" } "\n" ;
block_comment   = "/*" { any_char ^ "*/" } "*/" ;  (* no nested comments *)
lexeme_gap      = { space | line_comment | block_comment } ;  (* ZeroOrMore *)

(* Identifiers and Keywords *)
letter          = "a".."z" | "A".."Z" ;
digit           = "0".."9" ;
ident_start     = letter | "_" ;
ident_continue  = ident_start | digit ;
identifier      = ident_start { ident_continue } ;

keyword         = "fun" | "val" | "mut" | "record" | "context" | "enum"
                | "match" | "then" | "else" | "temporal" | "where" | "within"
                | "spawn" | "await" | "clone" | "freeze" | "pub" | "import"
                | "as" | "fatal" ;
                (* reserved for future: "macro", "effect", "trait", "type" *)

(* Numeric Literals *)
decimal_digit   = "0".."9" ;
hex_digit       = decimal_digit | "a".."f" | "A".."F" ;
int_literal     = "0x" hex_digit { hex_digit | "_" }
                | decimal_digit { decimal_digit | "_" } ;

float_literal   = decimal_digit { decimal_digit | "_" } "." decimal_digit { decimal_digit | "_" }
                  [ ("e" | "E") [ "+" | "-" ] decimal_digit { decimal_digit } ] ;

(* String and Character Literals *)
escape_seq      = "\\" ( "n" | "r" | "t" | "\\" | "\"" | "'" ) ;
string_literal  = "\"" { any_char ^ "\"" ^ "\\" | escape_seq } "\"" ;
char_literal    = "'"  ( any_char ^ "'" ^ "\\" | escape_seq ) "'" ;

(* Boolean and Unit *)
boolean_literal = "true" | "false" ;
unit_literal    = "()" ;

(* Operators *)
multiplicative_op = "*" | "/" | "%" ;
additive_op       = "+" | "-" ;
relational_op     = "<" | "<=" | ">" | ">=" ;
equality_op       = "==" | "!=" ;
logical_and_op    = "&&" ;
logical_or_op     = "||" ;
unary_op          = "!" | "-" ;
pipe_op           = "|>" ;
assign_op         = "=" ;
```

## 2. Types and Constraints

```ebnf
(* Type Expressions *)
type                = simple_type | generic_type | temporal_type | func_type ;

simple_type         = identifier ;
generic_type        = identifier "<" type { "," type } ">" ;
temporal_var        = "~" identifier ;
temporal_type       = simple_type "<" temporal_var ">" ;
                      (* TAT: temporal types auto-cleanup when scope ends *)

func_type           = "|" [ type { "," type } ] "|" "->" type ;

(* Temporal Constraints *)
temporal_constraint = temporal_var "within" temporal_var ;
where_clause        = "where" temporal_constraint { "," temporal_constraint } ;
```

## 3. Expressions

```ebnf
(* Expression Hierarchy - Lowest to Highest Precedence *)
expression          = pipe_expr ;

pipe_expr           = logical_or_expr { pipe_op logical_or_expr } ;

logical_or_expr     = logical_and_expr { logical_or_op logical_and_expr } ;

logical_and_expr    = equality_expr { logical_and_op equality_expr } ;

equality_expr       = relational_expr { equality_op relational_expr } ;

relational_expr     = additive_expr { relational_op additive_expr } ;

additive_expr       = multiplicative_expr { additive_op multiplicative_expr } ;

multiplicative_expr = call_expr { multiplicative_op call_expr } ;

call_expr           = unary_expr [ call_expr ] ;  (* right associative for OSV *)
                                                  (* a b c parses as a (b c) *)

unary_expr          = [ unary_op ] postfix_expr ;

postfix_expr        = primary_expr { postfix_suffix } ;

postfix_suffix      = field_access | clone_suffix | await_suffix ;
                      (* precedence: field > clone > await *)

field_access        = "." identifier ;
clone_suffix        = "." "clone" [ record_literal ] [ "freeze" ] ;
await_suffix        = "await" ;

(* Primary Expressions *)
primary_expr        = literal
                    | identifier
                    | "(" expression ")"
                    | lambda_expr
                    | block_expr
                    | match_expr
                    | then_else_expr
                    | record_literal
                    | scope_expr ;

(* Literals *)
literal             = int_literal | float_literal | string_literal
                    | char_literal | boolean_literal | unit_literal
                    | list_literal | range_literal ;

list_literal        = "[" [ expression { "," expression } ] "]" ;
range_literal       = "[" expression ".." expression "]" ;  (* closed interval Range<T> *)

(* Lambda Expression *)
lambda_expr         = "|" [ param_list ] "|" ( expression | block_expr ) ;
param_list          = param { "," param } ;
param               = identifier [ ":" type ] ;

(* Block Expression *)
block_expr          = "{" { statement } [ expression ] "}" ;
                      (* implicit Unit if no final expression *)

(* Conditional Expression *)
then_else_expr      = expression lexeme_gap "then" lexeme_gap block_expr 
                      lexeme_gap "else" lexeme_gap block_expr ;

(* Match Expression *)
match_expr          = expression "match" "{" match_arm { match_arm } "}" ;
match_arm           = pattern "=>" ( expression | block_expr ) ;

(* Patterns *)
pattern             = "_"                    (* wildcard *)
                    | identifier             (* variable *)
                    | literal               (* literal *)
                    | record_pattern        (* record *)
                    | list_pattern ;        (* list *)

record_pattern      = identifier "{" [ field_pattern { "," field_pattern } ] "}" ;
field_pattern       = identifier [ "=" pattern ] ;

list_pattern        = "[" [ pattern { "," pattern } ] "]"
                    | "[" pattern "|" pattern "]" ;

(* Record Literal *)
record_literal      = identifier "{" [ field_init { "," field_init } ] "}" ;
field_init          = identifier "=" expression ;

(* Scope Expression *)
scope_expr          = scope_value block_expr ;
scope_value         = identifier                      (* context name *)
                    | identifier record_literal       (* context with init *)
                    | "temporal" temporal_var         (* temporal scope *)
                    | "Arena" ;                       (* arena scope *)

(* Reserved for future extensions *)
(* macro_expr       = "macro" "!" ... ; *)
(* effect_expr      = "effect" "{" ... "}" ; *)
```

## 4. Statements

```ebnf
statement           = val_decl 
                    | assignment 
                    | temporal_decl
                    | expression [ ";" ] ;  (* semicolon optional *)
                                           (* type checker enforces purity *)

val_decl            = "val" [ "mut" ] identifier [ ":" type ] "=" expression ;
                      (* affine: each binding used at most once *)

assignment          = identifier "=" expression ;

temporal_decl       = "temporal" temporal_var [ where_clause ] ;
                      (* standalone declaration only *)
                      (* for scope use: temporal ~t { ... } *)
```

## 5. Declarations

```ebnf
(* Program Structure *)
program             = { top_decl } ;

top_decl            = function_decl 
                    | record_decl 
                    | context_decl
                    | enum_decl 
                    | import_decl ;
                    (* reserved: macro_decl, trait_decl, type_decl *)

(* Function Declaration *)
function_decl       = { context_ann } [ "pub" ] "fun" identifier ":" 
                      function_signature "=" block_expr ;

function_signature  = [ type_params ] param_block [ "->" type ] [ where_clause ] ;

type_params         = "<" type_param { "," type_param } ">" ;
type_param          = identifier | temporal_var ;

param_block         = "(" [ param_def { "," param_def } ] ")" ;
param_def           = identifier ":" type ;

context_ann         = "@" identifier ;  (* multiple @Context on separate lines *)
                                       (* type checker handles set semantics *)

(* Record Declaration *)
record_decl         = "record" identifier [ type_params ] "{" 
                      field_decl { field_decl } "}" ;

field_decl          = identifier ":" type [ "," | "\n" ] ;

(* Context Declaration *)
context_decl        = "context" identifier [ type_params ] "{" 
                      field_decl { field_decl } "}" ;

(* Enum Declaration *)
enum_decl           = "enum" identifier [ type_params ] "{" 
                      variant { variant } "}" ;

variant             = identifier [ variant_data ] ;
variant_data        = "(" type { "," type } ")"
                    | "{" field_decl { field_decl } "}" ;

(* Import Declaration *)
import_decl         = "import" string_literal "as" identifier ;
```

## 6. Complete Grammar Entry Point

```ebnf
restrict_program    = program ;
```

## Notes and Design Decisions

### Operator Precedence (Highest to Lowest)
1. Postfix operations: `.field`, `.clone`, `await`
2. Unary: `!`, `-`
3. OSV call: `a b` (right associative)
4. Multiplicative: `*`, `/`, `%`
5. Additive: `+`, `-`
6. Relational: `<`, `<=`, `>`, `>=`
7. Equality: `==`, `!=`
8. Logical AND: `&&`
9. Logical OR: `||`
10. Pipe: `|>` (left associative)

### Language Philosophy
- **OSV (Object-Subject-Verb)**: Natural function composition
- **Affine Types**: Each value used at most once
- **TAT (Temporal Affine Types)**: Automatic resource cleanup
- **No Side Effects**: Expression statements must be pure
- **Simplicity**: Reduce cognitive load for developers

### Implementation Notes
- Lexer handles Unicode categories (L*, N*, etc.)
- Parser uses precedence climbing or similar for operators
- Type checker enforces affine constraints
- Code generator handles temporal cleanup

### Reserved for Future Extensions
- `macro`: Macro system
- `effect`: Effect handlers
- `trait`: Trait system
- `type`: Type aliases

### Edge Cases and Clarifications
- Nested comments not supported: `/* /* */ */` is invalid
- OSV precedence: `a + b c` parses as `(a + b) c`
- Postfix chain: `x.field.clone await` applies left-to-right
- Pure expressions: `x + y;` allowed only if type checker confirms no side effects

### Testing Considerations
- Complex OSV chains: `Database { temporal ~t { a |> b c } }`
- Multiple contexts: `@Database @Logger @Audit fun f: ...`
- Deep nesting: Scope expressions within match arms
- Unicode identifiers: Future consideration

## Resolved Issues (Q1-Q19)

All previous issues (Q1-Q15) resolved in v-0.6.

Additional clarifications:
- Q16: OSV vs binary precedence clarified with annotations
- Q17: temporal_decl limited to standalone declarations
- Q18: Future extensions reserved in comments
- Q19: Affine/TAT semantics noted in relevant sections