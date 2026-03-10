MODULE AmbigEnum;
FROM SE_A IMPORT State, OK, GetA;
FROM SE_B IMPORT State, OK, GetB;
VAR s: State;
BEGIN
  s := OK
END AmbigEnum.
