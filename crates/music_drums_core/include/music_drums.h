#ifndef MUSIC_DRUMS_H
#define MUSIC_DRUMS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Start HID++ engine. Returns 0 on success. */
int md_start(void);

/** Start engine; on failure writes malloc'd C string to *out_error (md_string_free). */
int md_start_with_error(char **out_error);

void md_stop(void);

void md_set_sensitivity(float value);

/** Update DSP sample rate to match the Process Tap (e.g. 44100 / 48000). */
void md_set_sample_rate(float value);

/** Push interleaved stereo float samples (L,R,L,R,...). count = number of floats. */
void md_push_audio_frames(const float *frames, unsigned int count);

/** Heap JSON status; free with md_string_free. */
char *md_status_json(void);

/** Current drums config (pretty JSON). Free with md_string_free. */
char *md_config_json(void);

/** Apply config JSON. Returns 0 on success. */
int md_set_config_json(const char *json, char **out_error);

/** Builtin + user presets catalog JSON. Free with md_string_free. */
char *md_list_presets_json(void);

/** Load preset by id ("classic", "house") or file path. */
int md_load_preset(const char *id, char **out_error);

/** Save current config into ~/Library/Application Support/MusicDrums/presets/. */
int md_save_preset(char **out_error);

/** Export current config to an absolute path. */
int md_export_config(const char *path, char **out_error);

/** Import config from path and activate it. */
int md_import_config(const char *path, char **out_error);

void md_string_free(char *s);

int md_test_pulse(unsigned int intensity);
int md_test_pulse_with_error(unsigned int intensity, char **out_error);

/** 0=none, 1=bolt, 2=bluetooth */
int md_link_kind(void);

const char *md_version(void);

#ifdef __cplusplus
}
#endif

#endif /* MUSIC_DRUMS_H */
