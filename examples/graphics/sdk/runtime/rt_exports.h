/*
 * rt_exports.h — ABI contract between the ObjC++ runtime and M2 game code.
 *
 * arm64 macOS, C calling convention, no C++ or ObjC types.
 * The game module must provide these three symbols with external linkage.
 */

#ifndef RT_EXPORTS_H
#define RT_EXPORTS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void Game_Init(void *nsview, uint32_t width, uint32_t height);
void Game_Tick(int32_t dt_ms);
void Game_Shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* RT_EXPORTS_H */
