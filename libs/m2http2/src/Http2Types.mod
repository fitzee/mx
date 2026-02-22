IMPLEMENTATION MODULE Http2Types;

PROCEDURE InitDefaultSettings(VAR s: Settings);
BEGIN
  s.headerTableSize     := DefaultHeaderTableSize;
  s.enablePush          := DefaultEnablePush;
  s.maxConcurrentStreams := DefaultMaxConcurrentStreams;
  s.initialWindowSize   := DefaultInitialWindowSize;
  s.maxFrameSize        := DefaultMaxFrameSize;
  s.maxHeaderListSize   := DefaultMaxHeaderListSize
END InitDefaultSettings;

END Http2Types.
