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

/** Push interleaved stereo float samples (L,R,L,R,...). count = number of floats. */
void md_push_audio_frames(const float *frames, unsigned int count);

/** Heap JSON status; free with md_string_free. */
char *md_status_json(void);

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
