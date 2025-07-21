# Restrict Language EBNF (Extended Backus-Naur Form)

## 1. Lexical Elements

```ebnf
(* Whitespace and Comments *)
whitespace      = " " | "\t" | "\n" | "\r" ;
line_comment    = "//" { any_char - "\n" } "\n" ;
block_comment   = "/*" { any_char - "*/" } "*/" ;
comment         = line_comment | block_comment ;

(* Identifiers and Keywords *)
letter          = "a".."z" | "A".."Z" ;
digit           = "0".."9" ;
ident_start     = letter | "_" ;
ident_continue  = ident_start | digit ;
identifier      = ident_start { ident_continue } ;

keyword         = "fun" | "val" | "mut" | "record" | "context" | "enum"
                | "match" | "then" | "else" | "temporal" | "where"
                | "within" | "spawn" | "await" | "clone" | "freeze"
                | "impl" | "pub" | "import" | "as" | "fatal" ;

(* Literals *)
decimal_digit   = "0".."9" ;
hex_digit       = decimal_digit | "a".."f" | "A".."F" ;
underscore      = "_" ;

int_literal     = decimal_digit { decimal_digit | underscore }
                | "0x" hex_digit { hex_digit | underscore } ;

float_literal   = decimal_digit { decimal_digit | underscore } "." 
                  decimal_digit { decimal_digit | underscore }
                  [ ("e" | "E") [ "+" | "-" ] decimal_digit { decimal_digit } ] ;

escape_seq      = "\\" ( "n" | "r" | "t" | "\\" | "\"" | "'" ) ;
string_literal  = "\"" { any_char - "\"" - "\\" | escape_seq } "\"" ;
char_literal    = "'" ( any_char - "'" - "\\" | escape_seq ) "'" ;

boolean_literal = "true" | "false" ;

(* Operators *)
binary_op       = "+" | "-" | "*" | "/" | "%" 
                | "<" | "<=" | ">" | ">=" | "==" | "!="
                | "&&" | "||" ;

unary_op        = "!" | "-" ;
pipe_op         = "|>" ;
assign_op       = "=" ;
```

## 2. Types

```ebnf
(* Basic Types *)
type            = simple_type | generic_type | temporal_type | function_type ;

simple_type     = identifier ;

generic_type    = identifier "<" type_list ">" ;

temporal_type   = type "<" temporal_var ">" ;

temporal_var    = "~" identifier ;

function_type   = "|" [ param_type_list ] "|" "->" type ;

param_type_list = type { "," type } ;

type_list       = type { "," type } ;

(* Type Constraints *)
temporal_constraint = temporal_var "within" temporal_var ;

where_clause    = "where" temporal_constraint { "," temporal_constraint } ;
```

## 3. Expressions

```ebnf
(* Primary Expressions *)
expression      = binary_expr | primary_expr ;

primary_expr    = literal
                | identifier
                | "(" expression ")"
                | block_expr
                | lambda_expr
                | match_expr
                | then_else_expr
                | record_literal
                | field_access
                | method_call
                | function_call
                | clone_expr
                | environment_expr ;

literal         = int_literal | float_literal | string_literal 
                | char_literal | boolean_literal | "()" | list_literal ;

list_literal    = "[" [ expression { "," expression } ] "]" ;

(* Binary and Unary Expressions *)
binary_expr     = primary_expr [ binary_op primary_expr | pipe_expr ] ;

pipe_expr       = pipe_op primary_expr [ pipe_expr ] ;

unary_expr      = unary_op primary_expr ;

(* Block Expression *)
block_expr      = "{" { statement } [ expression ] "}" ;

(* Lambda Expression *)
lambda_expr     = "|" [ param_list ] "|" ( expression | block_expr ) ;

param_list      = param { "," param } ;

param           = identifier [ ":" type ] ;

(* Match Expression *)
match_expr      = expression "match" "{" match_arm { match_arm } "}" ;

match_arm       = pattern "=>" ( expression | block_expr ) ;

pattern         = literal_pattern
                | ident_pattern
                | record_pattern
                | list_pattern
                | wildcard_pattern ;

literal_pattern = literal ;
ident_pattern   = identifier ;
wildcard_pattern = "_" ;

record_pattern  = identifier "{" [ field_pattern { "," field_pattern } ] "}" ;
field_pattern   = identifier [ "=" pattern ] ;

list_pattern    = "[" [ pattern { "," pattern } ] "]"
                | "[" pattern "|" pattern "]" ;

(* Conditional Expression *)
then_else_expr  = expression "then" block_expr "else" block_expr ;

(* Record and Field Access *)
record_literal  = identifier "{" [ field_init { "," field_init } ] "}" ;

field_init      = identifier "=" expression ;

field_access    = expression "." identifier ;

(* Method and Function Call *)
method_call     = expression "." identifier "(" [ arg_list ] ")" ;

function_call   = expression expression  (* OSV syntax *)
                | identifier "(" [ arg_list ] ")" ;  (* traditional syntax *)

arg_list        = expression { "," expression } ;

(* Clone Expression *)
clone_expr      = expression ".clone" [ record_literal ] [ "freeze" ] ;

(* Environment Expression *)
environment_expr = environment_value block_expr ;

environment_value = identifier [ record_literal ]
                  | "temporal" temporal_var
                  | "Arena"
                  | "AsyncRuntime" ;
```

## 4. Statements

```ebnf
statement       = val_decl | assignment | expression ";" ;

val_decl        = [ "mut" ] "val" identifier [ ":" type ] "=" expression ;

assignment      = identifier "=" expression ;
```

## 5. Declarations

```ebnf
(* Top Level *)
program         = { top_level_decl } ;

top_level_decl  = function_decl
                | record_decl
                | context_decl
                | enum_decl
                | impl_decl
                | import_decl ;

(* Function Declaration *)
function_decl   = { context_annotation } [ "pub" ] "fun" identifier ":" 
                  function_signature "=" block_expr ;

function_signature = [ type_params ] "(" [ param_def_list ] ")" 
                    [ "->" type ] [ where_clause ] ;

type_params     = "<" type_param_list ">" ;

type_param_list = type_param { "," type_param } ;

type_param      = identifier | temporal_var ;

param_def_list  = param_def { "," param_def } ;

param_def       = identifier ":" type ;

context_annotation = "@" identifier ;

(* Record Declaration *)
record_decl     = "record" identifier [ type_params ] "{" 
                  { field_decl } "}" ;

field_decl      = identifier ":" type ;

(* Context Declaration *)
context_decl    = "context" identifier [ type_params ] "{" 
                  { field_decl } "}" ;

(* Enum Declaration *)
enum_decl       = "enum" identifier [ type_params ] "{" 
                  variant { variant } "}" ;

variant         = identifier [ variant_data ] ;

variant_data    = "(" type { "," type } ")"
                | "{" field_decl { field_decl } "}" ;

(* Implementation Block - deprecated but still parsed *)
impl_decl       = "impl" identifier [ type_params ] "{" 
                  { function_decl } "}" ;

(* Import Declaration *)
import_decl     = "import" string_literal "as" identifier ;
```

## 6. Special Forms

```ebnf
(* Temporal Declarations *)
temporal_decl   = "temporal" temporal_var [ where_clause ] ;

(* Range Literal *)
range_literal   = "[" expression ".." expression "]" ;

(* Fatal Expression *)
fatal_expr      = "fatal" expression ;

(* Spawn and Await *)
spawn_expr      = "spawn" block_expr ;
await_expr      = "await" expression ;
```

## 7. Complete Grammar Entry Point

```ebnf
restrict_program = program ;
```

## Notes

1. **OSV Syntax**: Function calls use Object-Subject-Verb order: `object function` or with pipe: `object |> function`
2. **Environment Scopes**: `X { ... }` creates an environment where X is present
3. **Temporal Variables**: Prefixed with `~` and represent lifetimes
4. **Context Annotations**: Functions use `@Context` to require specific contexts
5. **No `with` keyword**: Environments are created directly with `Context { ... }`