# Modula-2 Grammar Reference — mx Compiler

Concise EBNF grammar for PIM4 Modula-2 as accepted by the mx compiler.

## Notation

```
{ x }     zero or more repetitions
[ x ]     optional
x | y     alternative
"kw"      keyword or literal
```

## PIM4 Core

### Terminals

```ebnf
number            = integer | real .
string            = '"' { character } '"' | "'" { character } "'" .
CharConst         = integer "C" .          (* e.g. 37C, 0C *)
ConstExpr         = Expr .                 (* must be compile-time evaluable *)
```

### Compilation Units

```ebnf
CompilationUnit      = ProgramModule | DefinitionModule | ImplementationModule .
ProgramModule        = "MODULE" ident [ "[" ConstExpr "]" ] ";"
                       { Import } Block ident "." .
DefinitionModule     = "DEFINITION" "MODULE" ident ";"
                       { Import } { Export } { Definition } "END" ident "." .
ImplementationModule = "IMPLEMENTATION" "MODULE" ident ";"
                       { Import } Block ident "." .
```

### Imports and Exports

```ebnf
Import            = "IMPORT" IdentList ";"
                  | "FROM" ident "IMPORT" IdentList ";" .
Export            = "EXPORT" [ "QUALIFIED" ] IdentList ";" .
```

### Definitions (definition modules only)

```ebnf
Definition        = "CONST" { ConstDecl ";" }
                  | "TYPE" { TypeDeclDef ";" }
                  | "VAR" { VarDecl ";" }
                  | ProcedureHeading ";" .       (* heading only, no body *)

TypeDeclDef       = ident "=" Type
                  | ident .                      (* opaque, .def only *)
```

### Declarations (implementation / program modules)

```ebnf
Block             = { Declaration } [ "BEGIN" StatementSequence ] "END" .

Declaration       = "CONST" { ConstDecl ";" }
                  | "TYPE" { TypeDecl ";" }
                  | "VAR" { VarDecl ";" }
                  | ProcedureDecl ";"
                  | LocalModule ";" .            (* nested modules *)

ConstDecl         = ident "=" ConstExpr .
TypeDecl          = ident "=" Type .             (* no bare ident — see TypeDeclDef *)
VarDecl           = IdentList ":" Type .

LocalModule       = "MODULE" ident [ "[" ConstExpr "]" ] ";"
                    { Import } { Export } Block ident .

ProcedureDecl     = ProcedureHeading ";" Block ident .
ProcedureHeading  = "PROCEDURE" ident [ FormalParams ] .
FormalParams      = "(" [ FPSection { ";" FPSection } ] ")" [ ":" QualIdent ] .
FPSection         = [ "VAR" ] IdentList ":" FormalType .
FormalType        = [ "ARRAY" "OF" ] QualIdent .  (* PIM4: single ARRAY OF level *)
```

### Types

```ebnf
Type              = SimpleType | ArrayType | RecordType | SetType
                  | PointerType | ProcedureType | EnumType | SubrangeType .

SimpleType        = QualIdent | SubrangeType | EnumType .
EnumType          = "(" IdentList ")" .
SubrangeType      = "[" ConstExpr ".." ConstExpr "]" .

ArrayType         = "ARRAY" SimpleType { "," SimpleType } "OF" Type .
RecordType        = "RECORD" FieldList "END" .
FieldList         = { IdentList ":" Type ";"
                    | "CASE" [ ident ":" ] QualIdent "OF" [ "|" ]
                      Variant { "|" Variant }
                      [ "ELSE" FieldList ]
                      "END" ";" } .
Variant           = CaseLabelList ":" FieldList .

SetType           = "SET" "OF" SimpleType .
PointerType       = "POINTER" "TO" Type .
ProcedureType     = "PROCEDURE" [ FormalTypeList ] .
FormalTypeList    = "(" [ [ "VAR" ] FormalType { "," [ "VAR" ] FormalType } ] ")"
                    [ ":" QualIdent ] .
```

### Statements

```ebnf
StatementSequence = Statement { ";" Statement } .
Statement         = [ Assignment | ProcedureCall | IfStatement | CaseStatement
                    | WhileStatement | RepeatStatement | ForStatement
                    | LoopStatement | WithStatement | "EXIT"
                    | "RETURN" [ Expr ] ] .

Assignment        = Designator ":=" Expr .
ProcedureCall     = Designator [ ActualParams ] .
ActualParams      = "(" [ Expr { "," Expr } ] ")" .

IfStatement       = "IF" Expr "THEN" StatementSequence
                    { "ELSIF" Expr "THEN" StatementSequence }
                    [ "ELSE" StatementSequence ] "END" .

CaseStatement     = "CASE" Expr "OF" [ "|" ] Case { "|" Case }
                    [ "ELSE" StatementSequence ] "END" .
Case              = CaseLabelList ":" StatementSequence .
CaseLabelList     = CaseLabel { "," CaseLabel } .
CaseLabel         = ConstExpr [ ".." ConstExpr ] .

WhileStatement    = "WHILE" Expr "DO" StatementSequence "END" .
RepeatStatement   = "REPEAT" StatementSequence "UNTIL" Expr .
ForStatement      = "FOR" ident ":=" Expr "TO" Expr [ "BY" ConstExpr ]
                    "DO" StatementSequence "END" .
LoopStatement     = "LOOP" StatementSequence "END" .
WithStatement     = "WITH" Designator "DO" StatementSequence "END" .
```

### Expressions

The mx parser uses C-style operator precedence where OR and AND each occupy
their own level, rather than PIM4's grouping of OR with `+`/`-` and AND with
`*`/`/`. See **mx Accepted Differences** below.

```ebnf
Expr              = OrExpr .
OrExpr            = AndExpr { "OR" AndExpr } .         (* lowest precedence *)
AndExpr           = Relation { "AND" Relation } .
Relation          = SimpleExpr [ RelOp SimpleExpr ] .
RelOp             = "=" | "#" | "<" | "<=" | ">" | ">=" | "IN" .
SimpleExpr        = [ "+" | "-" ] Term { AddOp Term } .
AddOp             = "+" | "-" .
Term              = Factor { MulOp Factor } .
MulOp             = "*" | "/" | "DIV" | "MOD" .

Factor            = number | string | CharConst | SetValue
                  | Designator [ ActualParams ]
                  | "(" Expr ")" | "NOT" Factor .

Designator        = QualIdent { "." ident | "[" Expr { "," Expr } "]" | "^" } .

(* SetValue: QualIdent prefix names the set type; bare {...} is accepted — see below *)
SetValue          = [ QualIdent ] "{" [ Element { "," Element } ] "}" .
Element           = Expr [ ".." Expr ] .

QualIdent         = [ ident "." ] ident .
IdentList         = ident { "," ident } .
```

**Operator synonyms (PIM4-standard):**

| Synonym | Equivalent |
|---------|-----------|
| `&`     | `AND`     |
| `~`     | `NOT`     |
| `<>`    | `#` (not-equal) |

---

## mx Accepted Differences

These are intentional divergences from strict PIM4. They are accepted in all
compiler modes (no flag required).

| Divergence | PIM4 rule | mx behavior |
|-----------|-----------|-------------|
| **C-style operator precedence** | OR at additive level (+, -); AND at multiplicative level (*, /) | OR and AND each have their own lower-precedence levels: OR < AND < relational < additive < multiplicative. Matches C/Java/Python conventions and most practical M2 compilers. |
| **Base-typed subranges** | `SubrangeType = "[" ConstExpr ".." ConstExpr "]"` only | Also accepts `QualIdent "[" ConstExpr ".." ConstExpr "]"` (e.g. `INTEGER[0..255]`). |
| **Bare set constructors** | Set value requires a type prefix: `QualIdent "{" ... "}"` | Bare `{ elem, ... }` accepted; type defaults to BITSET. |
| **Recursive ARRAY OF in formal types** | `FormalType = ["ARRAY" "OF"] QualIdent` — one level only | Accepts `ARRAY OF ARRAY OF QualIdent` (multi-dimensional open arrays). |
| **`<>` as not-equal** | PIM4 uses `#` | `<>` accepted as synonym for `#`. |
| **`&` and `~` as synonyms** | PIM4 defines these but some compilers omit them | Always accepted: `&` for AND, `~` for NOT. |

---

## Modula-2+ Extensions (M2+)

Enabled with the `--m2plus` compiler flag. All constructs in this section are
extensions beyond PIM4.

### Exception Handling

```ebnf
TryStatement      = "TRY" StatementSequence
                    { "EXCEPT" ident "DO" StatementSequence }
                    [ "EXCEPT" StatementSequence ]
                    [ "FINALLY" StatementSequence ] "END" .
ExceptionDecl     = "EXCEPTION" ident ";" .
RaiseStatement    = "RAISE" QualIdent .
RetryStatement    = "RETRY" .
```

Module-level exception handling in Block:

```ebnf
Block             = { Declaration }
                    [ "BEGIN" StatementSequence ]
                    [ "EXCEPT" StatementSequence ]
                    [ "FINALLY" StatementSequence ]
                    "END" .
```

### REF Types

```ebnf
RefType           = [ "BRANDED" [ string ] ] "REF" Type | "REFANY" .
```

### OBJECT Types

```ebnf
ObjectType        = QualIdent "OBJECT" FieldList
                    { MethodDecl }
                    [ "OVERRIDES" { MethodDecl } ] "END" .
MethodDecl        = ident FormalParams ";" .
```

### LOCK Statement

```ebnf
LockStatement     = "LOCK" Designator "DO" StatementSequence "END" .
```

### TYPECASE Statement

```ebnf
TypecaseStatement = "TYPECASE" Expr "OF" TypeCase { "|" TypeCase }
                    [ "ELSE" StatementSequence ] "END" .
TypeCase          = QualIdent { "," QualIdent } [ "(" ident ")" ] ":" StatementSequence .
```

### Import Aliases

```ebnf
Import            = "FROM" ident "IMPORT" ImportItem { "," ImportItem } ";" .
ImportItem        = ident [ "AS" ident ] .
```

### Foreign Definition Modules

```ebnf
ForeignDefModule  = "DEFINITION" "MODULE" "FOR" string ident ";"
                    { Definition } "END" ident "." .
```

### Module Safety Annotations

```ebnf
SafetyAnnotation  = "SAFE" | "UNSAFE" .
(* Prefixes MODULE / DEFINITION MODULE / IMPLEMENTATION MODULE *)
(* Parsed but not currently enforced *)
```

### RAISES Clause

```ebnf
ProcedureHeading  = "PROCEDURE" ident [ FormalParams ] [ RaisesClause ] .
RaisesClause      = "RAISES" "{" [ QualIdent { "," QualIdent } ] "}" .
```
