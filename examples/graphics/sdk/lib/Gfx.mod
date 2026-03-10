IMPLEMENTATION MODULE Gfx;

FROM MtlWrap IMPORT mtl_create_device, mtl_create_queue,
                     mtl_attach_layer, mtl_load_default_library,
                     mtl_compile_library, mtl_create_pipeline,
                     mtl_create_buffer, mtl_draw, mtl_release;

PROCEDURE CreateDevice(): Device;
BEGIN
  RETURN mtl_create_device()
END CreateDevice;

PROCEDURE CreateQueue(dev: Device): Queue;
BEGIN
  RETURN mtl_create_queue(dev)
END CreateQueue;

PROCEDURE AttachLayer(dev: Device; view: ADDRESS;
                      w, h: CARDINAL): Layer;
BEGIN
  RETURN mtl_attach_layer(dev, view, w, h)
END AttachLayer;

PROCEDURE LoadDefaultLibrary(dev: Device): Library;
BEGIN
  RETURN mtl_load_default_library(dev)
END LoadDefaultLibrary;

PROCEDURE CompileLibrary(dev: Device; source: ADDRESS): Library;
BEGIN
  RETURN mtl_compile_library(dev, source)
END CompileLibrary;

PROCEDURE CreatePipeline(dev: Device; lib: Library;
                         vertexFn, fragmentFn: ADDRESS): Pipeline;
BEGIN
  RETURN mtl_create_pipeline(dev, lib, vertexFn, fragmentFn)
END CreatePipeline;

PROCEDURE CreateBuffer(dev: Device; data: ADDRESS;
                       size: CARDINAL): Buffer;
BEGIN
  RETURN mtl_create_buffer(dev, data, size)
END CreateBuffer;

PROCEDURE Draw(q: Queue; l: Layer; p: Pipeline; vb: Buffer;
               vertexCount: CARDINAL; clearRGBA: ADDRESS);
BEGIN
  mtl_draw(q, l, p, vb, vertexCount, clearRGBA)
END Draw;

PROCEDURE Release(handle: ADDRESS);
BEGIN
  mtl_release(handle)
END Release;

END Gfx.
