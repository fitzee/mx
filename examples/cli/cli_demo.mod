MODULE CLIDemo;

FROM CLI IMPORT AddFlag, AddOption, Parse, HasFlag, GetOption, PrintHelp;
FROM Args IMPORT ArgCount, GetArg;
FROM InOut IMPORT WriteString, WriteLn;

VAR output: ARRAY [0..255] OF CHAR;

BEGIN
  AddFlag("v", "verbose", "Enable verbose output");
  AddFlag("h", "help", "Show help message");
  AddOption("o", "output", "Output file path");
  Parse(ArgCount(), GetArg);

  IF HasFlag("help") = 1 THEN
    WriteString("cli_demo - Example CLI application"); WriteLn;
    PrintHelp;
    RETURN
  END;

  IF HasFlag("verbose") = 1 THEN
    WriteString("verbose mode enabled"); WriteLn
  END;

  IF GetOption("output", output) = 1 THEN
    WriteString("output: "); WriteString(output); WriteLn
  END
END CLIDemo.
