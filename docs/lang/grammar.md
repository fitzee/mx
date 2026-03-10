# Modula-2 PIM4 Grammar Reference

Concise EBNF grammar for PIM4 Modula-2 as implemented by mx. Modula-2+
extensions are marked with **(M2+)**.

## Notation

```
{ x }     zero or more repetitions
[ x ]     optional
x | y     alternative
"kw"      keyword or literal
```

## Compilation Units

```ebnf
CompilationUnit   = ProgramModule | DefinitionModule | ImplementationModule .
ProgramModule     = "MODULE" ident ";" { Import } Block ident "." .
DefinitionModule  = "DEFINITION" "MODULE" ident ";" { Import } { Definition } "END" ident "." .
ImplementationModule = "IMPLEMENTATION" "MODULE" ident ";" { Import } Block ident "." .

(* M2+ foreign module *)
ForeignDefModule  = "DEFINITION" "MODULE" "FOR" string ident ";" { Definition } "END" ident "." .
```

## Imports and Exports

```ebnf
Import            = "IMPORT" IdentList ";"
                  | "FROM" ident "IMPORT" IdentList ";" .
Export            = "EXPORT" [ "QUALIFIED" ] IdentList ";" .
```

## Declarations

```ebnf
Block             = { Declaration } [ "BEGIN" StatementSequence ] "END" .
Declaration       = "CONST" { ConstDecl ";" }
                  | "TYPE" { TypeDecl ";" }
                  | "VAR" { VarDecl ";" }
                  | ProcedureDecl ";" .

ConstDecl         = ident "=" ConstExpr .
TypeDecl          = ident "=" Type | ident .    (* opaque in .def *)
VarDecl           = IdentList ":" Type .

ProcedureDecl     = ProcedureHeading ";" Block ident .
ProcedureHeading  = "PROCEDURE" ident [ FormalParams ] .
FormalParams      = "(" [ FPSection { ";" FPSection } ] ")" [ ":" QualIdent ] .
FPSection         = [ "VAR" ] IdentList ":" FormalType .
FormalType        = [ "ARRAY" "OF" ] QualIdent .
```

## Types

```ebnf
Type              = SimpleType | ArrayType | RecordType | SetType | PointerType
                  | ProcedureType | EnumType | SubrangeType
                  | RefType | ObjectType .       (* M2+ *)

SimpleType        = QualIdent | SubrangeType | EnumType .
EnumType          = "(" IdentList ")" .
SubrangeType      = "[" ConstExpr ".." ConstExpr "]"
                  | QualIdent "[" ConstExpr ".." ConstExpr "]" .

ArrayType         = "ARRAY" SimpleType { "," SimpleType } "OF" Type .
RecordType        = "RECORD" FieldList "END" .
FieldList         = { IdentList ":" Type ";"
                    | "CASE" [ ident ":" ] QualIdent "OF" [ "|" ]
                      Variant { "|" Variant } [ "ELSE" FieldList ] "END" ";" } .
Variant           = CaseLabelList ":" FieldList .

SetType           = "SET" "OF" SimpleType .
PointerType       = "POINTER" "TO" Type .
ProcedureType     = "PROCEDURE" [ FormalTypeList ] .
FormalTypeList    = "(" [ [ "VAR" ] FormalType { "," [ "VAR" ] FormalType } ] ")"
                    [ ":" QualIdent ] .
```

## Statements

```ebnf
StatementSequence = Statement { ";" Statement } .
Statement         = [ Assignment | ProcedureCall | IfStatement | CaseStatement
                    | WhileStatement | RepeatStatement | ForStatement
                    | LoopStatement | WithStatement | "EXIT" | "RETURN" [ Expr ]
                    | TryStatement | LockStatement | TypecaseStatement ] .

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

## Expressions

```ebnf
Expr              = SimpleExpr [ Relation SimpleExpr ] .
Relation          = "=" | "#" | "<" | "<=" | ">" | ">=" | "IN" .
SimpleExpr        = [ "+" | "-" ] Term { AddOp Term } .
AddOp             = "+" | "-" | "OR" .
Term              = Factor { MulOp Factor } .
MulOp             = "*" | "/" | "DIV" | "MOD" | "AND" .
Factor            = number | string | CharConst | SetValue
                  | Designator [ ActualParams ]
                  | "(" Expr ")" | "NOT" Factor .
Designator        = QualIdent { "." ident | "[" Expr { "," Expr } "]" | "^" } .
SetValue          = QualIdent "{" [ Element { "," Element } ] "}" .
Element           = Expr [ ".." Expr ] .
QualIdent         = [ ident "." ] ident .
IdentList         = ident { "," ident } .
```

## Modula-2+ Extensions (M2+)

Enabled with the `--m2plus` compiler flag.

### Exception Handling

```ebnf
TryStatement      = "TRY" StatementSequence
                    { "EXCEPT" ident "DO" StatementSequence }
                    [ "EXCEPT" StatementSequence ]
                    [ "FINALLY" StatementSequence ] "END" .
ExceptionDecl     = "EXCEPTION" ident ";" .
RaiseStatement    = "RAISE" QualIdent .
```

### REF Types

```ebnf
RefType           = [ "BRANDED" [ string ] ] "REF" Type | "REFANY" .
```

### OBJECT Types

```ebnf
ObjectType        = QualIdent "OBJECT" FieldList { MethodDecl } [ "OVERRIDES" { MethodDecl } ] "END" .
MethodDecl        = ident FormalParams ";" .
```

### Concurrency

```ebnf
LockStatement     = "LOCK" Designator "DO" StatementSequence "END" .
```

### TYPECASE

```ebnf
TypecaseStatement = "TYPECASE" Expr "OF" TypeCase { "|" TypeCase }
                    [ "ELSE" StatementSequence ] "END" .
TypeCase          = QualIdent { "," QualIdent } [ "(" ident ")" ] ":" StatementSequence .
```

### Module Safety Annotations

```ebnf
SafetyAnnotation  = "SAFE" | "UNSAFE" .
(* Parsed but not currently enforced *)
```
