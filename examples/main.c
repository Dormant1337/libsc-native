#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <pthread.h>
#include <SDL2/SDL.h>
#include <ncurses.h>
#include "../include/sc_player.h"

struct PlayerState {
	char *url;
	bool is_paused;
	bool should_quit;
	bool is_downloading;
	SDL_AudioDeviceID dev;
};

struct PlayerState ctx;

void audio_cb(const float *data, uint32_t len)
{
	while (ctx.is_paused && !ctx.should_quit) {
		SDL_Delay(50);
	}
	if (ctx.should_quit) return;

	while (SDL_GetQueuedAudioSize(ctx.dev) > 176400 && !ctx.should_quit) {
		SDL_Delay(10);
	}

	SDL_QueueAudio(ctx.dev, data, len * sizeof(float));
}

void *worker_thread(void *arg)
{
	(void)arg;
	
	/* LIBRARY CALL: Stream track and invoke callback for chunks */
	sc_stream_track(ctx.url, audio_cb);
	
	ctx.should_quit = true;
	return NULL;
}

void *download_thread(void *arg)
{
	(void)arg;
	ctx.is_downloading = true;

	/* LIBRARY CALL: Download track to file system */
	sc_download_track(ctx.url);

	ctx.is_downloading = false;
	return NULL;
}

int main(int argc, char *argv[])
{
	if (argc < 2) {
		printf("Usage: %s <soundcloud_url>\n", argv[0]);
		return 1;
	}

	ctx.url = argv[1];
	ctx.is_paused = false;
	ctx.should_quit = false;
	ctx.is_downloading = false;

	if (SDL_Init(SDL_INIT_AUDIO) < 0) return 1;

	SDL_AudioSpec want = {0};
	want.freq = 44100;
	want.format = AUDIO_F32SYS;
	want.channels = 1;
	want.samples = 4096;

	ctx.dev = SDL_OpenAudioDevice(NULL, 0, &want, NULL, 0);
	SDL_PauseAudioDevice(ctx.dev, 0);

	initscr();
	noecho();
	curs_set(0);
	nodelay(stdscr, TRUE);

	pthread_t t_stream;
	pthread_create(&t_stream, NULL, worker_thread, NULL);

	while (!ctx.should_quit) {
		int ch = getch();
		if (ch == 'q' || ch == 'Q') {
			ctx.should_quit = true;
		} else if (ch == 'p' || ch == 'P') {
			ctx.is_paused = !ctx.is_paused;
			SDL_PauseAudioDevice(ctx.dev, ctx.is_paused);
		} else if (ch == 'd' || ch == 'D') {
			if (!ctx.is_downloading) {
				pthread_t t_dl;
				pthread_create(&t_dl, NULL, download_thread, NULL);
				pthread_detach(t_dl);
			}
		}

		clear();
		mvprintw(1, 1, "SC Native Player");
		mvprintw(3, 1, "Track: %s", ctx.url);
		mvprintw(5, 1, "[P] Play/Pause  [D] Download  [Q] Quit");
		mvprintw(7, 1, "State: %s", ctx.is_paused ? "PAUSED" : "PLAYING");
		if (ctx.is_downloading) {
			mvprintw(8, 1, ">> DOWNLOADING IN BACKGROUND...");
		}
		refresh();
		SDL_Delay(50);
	}

	endwin();
	SDL_CloseAudioDevice(ctx.dev);
	SDL_Quit();
	return 0;
}