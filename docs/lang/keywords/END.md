# END

Marks the end of a block, module, procedure, or structured statement. Often
paired with a name for modules and procedures.

```modula2
END;            (* statement block *)
END name;       (* procedure *)
END name.       (* module *)
```

## Contexts

END closes: MODULE, PROCEDURE, IF, WHILE, FOR, LOOP, CASE, WITH, RECORD.

```modula2
IF x > 0 THEN y := x END;
END MyModule.
```
