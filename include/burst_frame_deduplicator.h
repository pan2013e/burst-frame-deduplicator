#ifndef BURST_FRAME_DEDUPLICATOR_H
#define BURST_FRAME_DEDUPLICATOR_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*bfd_progress_callback)(const char *progress_json, void *context);

uint32_t bfd_api_version(void);

/* Returned JSON strings are UTF-8 and must be released with bfd_free_string. */
char *bfd_default_options(void);
char *bfd_scan(const char *request_json, bfd_progress_callback callback, void *context);
char *bfd_load_run(const char *request_json);
char *bfd_set_decision(const char *request_json);
char *bfd_prepare_preview(const char *request_json);
char *bfd_export_run(const char *request_json);
char *bfd_move_rejects(const char *request_json);
char *bfd_restore_rejects(const char *request_json);
void bfd_free_string(char *value);

#ifdef __cplusplus
}
#endif

#endif
