IMPLEMENTATION MODULE FieldDef;
FROM Strings IMPORT CompareStr;

PROCEDURE Resolve(VAR name: ARRAY OF CHAR): FieldId;
BEGIN
  IF CompareStr(name, "name") = 0 THEN RETURN FiName
  ELSIF CompareStr(name, "value") = 0 THEN RETURN FiValue
  ELSIF CompareStr(name, "type") = 0 THEN RETURN FiType
  ELSE RETURN FiUnknown
  END
END Resolve;

END FieldDef.
