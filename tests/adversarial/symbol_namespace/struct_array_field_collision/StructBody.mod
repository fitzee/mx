IMPLEMENTATION MODULE StructBody;

PROCEDURE InitMsg(VAR m: Msg; t: CARDINAL; s: CARDINAL; sq: CARDINAL);
BEGIN
  m.body.tag := t;
  m.body.size := s;
  m.seq := sq
END InitMsg;

END StructBody.
