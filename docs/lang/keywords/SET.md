# SET

Set type. The base type must be an ordinal type with a small range (typically
0..15 or 0..31). Supports union (+), difference (-), intersection (*), and
symmetric difference (/).

```modula2
TYPE S = SET OF BaseType;
```

## Example

```modula2
TYPE CharSet = SET OF CHAR;
VAR vowels, consonants: CharSet;

vowels := CharSet{'a','e','i','o','u'};
IF ch IN vowels THEN
  WriteString("vowel");
END;
```
