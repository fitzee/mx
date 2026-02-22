IMPLEMENTATION MODULE Http2ServerMetrics;

  FROM Log IMPORT Logger, Field, KVInt, LogKV, INFO;

  PROCEDURE MetricsInit(VAR m: Metrics);
  BEGIN
    m.connsAccepted := 0;
    m.connsActive := 0;
    m.connsClosed := 0;
    m.tlsHandshakeFail := 0;
    m.alpnReject := 0;
    m.streamsOpened := 0;
    m.reqTotal := 0;
    m.resp2xx := 0;
    m.resp4xx := 0;
    m.resp5xx := 0;
    m.protoErrors := 0;
    m.bytesIn := 0;
    m.bytesOut := 0;
  END MetricsInit;

  PROCEDURE IncConnsAccepted(VAR m: Metrics);
  BEGIN
    INC(m.connsAccepted);
  END IncConnsAccepted;

  PROCEDURE IncConnsActive(VAR m: Metrics);
  BEGIN
    INC(m.connsActive);
  END IncConnsActive;

  PROCEDURE DecConnsActive(VAR m: Metrics);
  BEGIN
    IF m.connsActive > 0 THEN
      DEC(m.connsActive);
    END;
  END DecConnsActive;

  PROCEDURE IncConnsClosed(VAR m: Metrics);
  BEGIN
    INC(m.connsClosed);
  END IncConnsClosed;

  PROCEDURE IncTLSFail(VAR m: Metrics);
  BEGIN
    INC(m.tlsHandshakeFail);
  END IncTLSFail;

  PROCEDURE IncALPNReject(VAR m: Metrics);
  BEGIN
    INC(m.alpnReject);
  END IncALPNReject;

  PROCEDURE IncStreamsOpened(VAR m: Metrics);
  BEGIN
    INC(m.streamsOpened);
  END IncStreamsOpened;

  PROCEDURE IncReqTotal(VAR m: Metrics);
  BEGIN
    INC(m.reqTotal);
  END IncReqTotal;

  PROCEDURE IncResp(VAR m: Metrics; statusCode: CARDINAL);
  BEGIN
    IF (statusCode >= 200) AND (statusCode <= 299) THEN
      INC(m.resp2xx);
    ELSIF (statusCode >= 400) AND (statusCode <= 499) THEN
      INC(m.resp4xx);
    ELSIF (statusCode >= 500) AND (statusCode <= 599) THEN
      INC(m.resp5xx);
    END;
  END IncResp;

  PROCEDURE IncProtoErrors(VAR m: Metrics);
  BEGIN
    INC(m.protoErrors);
  END IncProtoErrors;

  PROCEDURE AddBytesIn(VAR m: Metrics; n: CARDINAL);
  BEGIN
    INC(m.bytesIn, n);
  END AddBytesIn;

  PROCEDURE AddBytesOut(VAR m: Metrics; n: CARDINAL);
  BEGIN
    INC(m.bytesOut, n);
  END AddBytesOut;

  PROCEDURE MetricsLog(VAR m: Metrics; VAR lg: Logger);
  VAR
    fields: ARRAY [0..12] OF Field;
  BEGIN
    KVInt("connsAccepted", VAL(INTEGER, m.connsAccepted), fields[0]);
    KVInt("connsActive", VAL(INTEGER, m.connsActive), fields[1]);
    KVInt("connsClosed", VAL(INTEGER, m.connsClosed), fields[2]);
    KVInt("tlsHandshakeFail", VAL(INTEGER, m.tlsHandshakeFail), fields[3]);
    KVInt("alpnReject", VAL(INTEGER, m.alpnReject), fields[4]);
    KVInt("streamsOpened", VAL(INTEGER, m.streamsOpened), fields[5]);
    KVInt("reqTotal", VAL(INTEGER, m.reqTotal), fields[6]);
    KVInt("resp2xx", VAL(INTEGER, m.resp2xx), fields[7]);
    KVInt("resp4xx", VAL(INTEGER, m.resp4xx), fields[8]);
    KVInt("resp5xx", VAL(INTEGER, m.resp5xx), fields[9]);
    KVInt("protoErrors", VAL(INTEGER, m.protoErrors), fields[10]);
    KVInt("bytesIn", VAL(INTEGER, m.bytesIn), fields[11]);
    KVInt("bytesOut", VAL(INTEGER, m.bytesOut), fields[12]);
    LogKV(lg, INFO, "metrics", fields, 13);
  END MetricsLog;

END Http2ServerMetrics.
