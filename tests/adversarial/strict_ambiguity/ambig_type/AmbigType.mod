MODULE AmbigType;
FROM SA_A IMPORT Status, GetStatus;
FROM SA_B IMPORT Status, GetStatus;
VAR s: Status;
BEGIN
  s := GetStatus()
END AmbigType.
