#ifndef BURST_FRAME_DEDUPLICATOR_H
#define BURST_FRAME_DEDUPLICATOR_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*bfd_progress_callback)(const char *progress_json, void *context);

uint32_t bfd_api_version(void);
uint64_t bfd_scan_control_create(void);
uint8_t bfd_scan_control_cancel(uint64_t control_id);
void bfd_scan_control_release(uint64_t control_id);

/* Returned JSON strings are UTF-8 and must be released with bfd_free_string. */
char *bfd_default_options(void);
char *bfd_scan(const char *request_json, bfd_progress_callback callback, void *context);
char *bfd_scan_controlled(const char *request_json, bfd_progress_callback callback, void *context, uint64_t control_id);
char *bfd_load_run(const char *request_json);
char *bfd_load_run_with_progress(const char *request_json, bfd_progress_callback callback, void *context);
char *bfd_set_decision(const char *request_json);
char *bfd_prepare_preview(const char *request_json);
char *bfd_export_run(const char *request_json);
char *bfd_move_rejects(const char *request_json);
char *bfd_restore_rejects(const char *request_json);
char *bfd_plan_counterparts(const char *request_json);
char *bfd_apply_counterparts(const char *request_json);
char *bfd_restore_counterparts(const char *request_json);
char *bfd_relocate_run(const char *request_json, bfd_progress_callback callback, void *context);
void bfd_free_string(char *value);

#ifdef __cplusplus
}
#endif

#endif
