MODULE StressMain;
FROM InOut IMPORT WriteInt, WriteLn;
IMPORT EventLoop;
IMPORT Sockets;
IMPORT TLS;
IMPORT Promise;
IMPORT Scheduler;
IMPORT Stream;
BEGIN
  WriteInt(EventLoop.Check(), 0); WriteLn;
  WriteInt(Sockets.Check(), 0); WriteLn;
  WriteInt(TLS.Check(), 0); WriteLn;
  WriteInt(Promise.Check(), 0); WriteLn;
  WriteInt(Scheduler.Check(), 0); WriteLn;
  WriteInt(Stream.Check(), 0); WriteLn
END StressMain.
