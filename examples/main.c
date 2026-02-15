#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <pthread.h>
#include <SDL2/SDL.h>
#include <ncurses.h>
#include <locale.h>
#include "../include/sc_player.h"

FILE *logfile = NULL;

void log_msg(const char *fmt, ...) {
	if (!logfile) return;
	va_list args;
	va_start(args, fmt);
	vfprintf(logfile, fmt, args);
	fprintf(logfile, "\n");
	fflush(logfile);
	va_end(args);
}

struct AppState {
	char track_url[512];
	char track_title[512];
	char last_error[256]; /* Buffer for UI error message */
	bool is_playing;
	bool is_paused;
	bool stop_flag;
	bool is_downloading;
	SDL_AudioDeviceID dev;
};

struct AppState app;

void audio_callback(const float *samples, uint32_t count)
{
	/* Debug logging first packet to ensure stream is alive */
	static bool logged_start = false;
	if (!logged_start) {
		log_msg("Audio Callback: received first chunk of %u samples", count);
		logged_start = true;
	}

	while (app.is_paused && !app.stop_flag) {
		SDL_Delay(50);
	}
	if (app.stop_flag) return;

	while (SDL_GetQueuedAudioSize(app.dev) > 176400 * sizeof(float)) {
		if (app.stop_flag) return;
		SDL_Delay(10);
	}

	int res = SDL_QueueAudio(app.dev, samples, count * sizeof(float));
	if (res < 0) {
		log_msg("SDL Error queueing audio: %s", SDL_GetError());
	}
}

void *thread_play(void *arg)
{
	(void)arg;
	app.is_playing = true;
	app.stop_flag = false;
	app.last_error[0] = '\0'; /* Clear previous errors */
	
	log_msg("Starting stream for: %s", app.track_url);

	/* LIB CALL */
	int res = sc_stream_track(app.track_url, audio_callback, &app.stop_flag);
	
	log_msg("Stream finished with code: %d", res);

	if (res != 0) {
		char *err = sc_get_last_error();
		if (err) {
			log_msg("Lib Error: %s", err);
			strncpy(app.last_error, err, 255);
			sc_free_string(err);
		} else {
			sprintf(app.last_error, "Unknown error code: %d", res);
		}
	}

	app.is_playing = false;
	return NULL;
}

void *thread_download(void *arg)
{
	(void)arg;
	app.is_downloading = true;
	app.last_error[0] = '\0';
	
	log_msg("Starting download...");
	int res = sc_download_track(app.track_url);
	log_msg("Download finished: %d", res);

	if (res != 0) {
		char *err = sc_get_last_error();
		if (err) {
			strncpy(app.last_error, err, 255);
			sc_free_string(err);
		}
	}
	
	app.is_downloading = false;
	return NULL;
}

void ui_search()
{
	timeout(-1); /* Blocking input */
	echo();
	curs_set(1);
	char query[128];
	memset(query, 0, 128);

	mvprintw(10, 2, "Search Query: ");
	getnstr(query, 127);
	noecho();
	curs_set(0);

	if (strlen(query) == 0) return;

	mvprintw(12, 2, "Searching...");
	refresh();
	log_msg("Searching for: %s", query);

	SearchContext *ctx = sc_search(query);
	if (!ctx) {
		char *err = sc_get_last_error();
		mvprintw(12, 2, "Search Error: %s", err ? err : "Unknown");
		if (err) sc_free_string(err);
		getch();
		return;
	}

	uint32_t count = sc_search_result_count(ctx);
	if (count == 0) {
		mvprintw(12, 2, "No results found.");
		sc_search_free(ctx);
		getch();
		return;
	}

	int choice = 0;
	while (1) {
		clear();
		mvprintw(1, 2, "Select Track (UP/DOWN/ENTER):");
		for (uint32_t i = 0; i < count; i++) {
			const char *title = sc_search_result_get_title(ctx, i);
			if (i == (uint32_t)choice) attron(A_REVERSE);
			mvprintw(3 + i, 4, "%d. %s", i + 1, title);
			if (i == (uint32_t)choice) attroff(A_REVERSE);
		}
		
		int ch = getch();
		if (ch == KEY_UP && choice > 0) choice--;
		if (ch == KEY_DOWN && choice < (int)count - 1) choice++;
		if (ch == 10) { // Enter
			const char *url = sc_search_result_get_url(ctx, choice);
			const char *title = sc_search_result_get_title(ctx, choice);
			strncpy(app.track_url, url, 511);
			strncpy(app.track_title, title, 511);
			log_msg("Selected: %s (%s)", title, url);
			break;
		}
		if (ch == 'q') break;
	}

	sc_search_free(ctx);
}

int main()
{
        setlocale(LC_ALL, "");
	logfile = fopen("sc_player.log", "w");
	log_msg("App started");

	if (SDL_Init(SDL_INIT_AUDIO) < 0) {
		log_msg("SDL Init failed: %s", SDL_GetError());
		return 1;
	}

	SDL_AudioSpec want = {0};
	want.freq = 44100;
	want.format = AUDIO_F32SYS;
	want.channels = 2;
	want.samples = 4096;
	
	/* Open device and log result */
	app.dev = SDL_OpenAudioDevice(NULL, 0, &want, NULL, 0);
	if (app.dev == 0) {
		log_msg("SDL OpenAudioDevice failed: %s", SDL_GetError());
	} else {
		log_msg("Audio device opened. ID: %d", app.dev);
		SDL_PauseAudioDevice(app.dev, 0);
	}

	initscr();
	cbreak();
	noecho();
	keypad(stdscr, TRUE);
	curs_set(0);

	pthread_t t_worker;

	while (1) {
		clear();
		mvprintw(1, 2, "=== SC NATIVE PLAYER ===");
		mvprintw(3, 2, "Current: %s", strlen(app.track_title) ? app.track_title : "None");
		
		/* Display Logic: Show Error if exists, otherwise show status */
		if (strlen(app.last_error) > 0) {
			attron(A_BOLD);
			mvprintw(4, 2, "ERROR: %s", app.last_error);
			attroff(A_BOLD);
		} else {
			mvprintw(4, 2, "Status: %s", app.is_playing ? (app.is_paused ? "[PAUSED]" : "[PLAYING]") : "[IDLE]");
		}
		
		if (app.is_downloading) mvprintw(5, 2, ">> DOWNLOADING <<");

		mvprintw(7, 2, "[S] Search tracks");
		mvprintw(8, 2, "[P] Play/Pause");
		mvprintw(9, 2, "[D] Download current");
		mvprintw(10, 2, "[X] Stop");
		mvprintw(11, 2, "[Q] Quit");

		timeout(100);
		int ch = getch();

		if (ch == 'q' || ch == 'Q') {
			app.stop_flag = true;
			if (app.is_playing) pthread_join(t_worker, NULL);
			break;
		}
		if (ch == 's' || ch == 'S') {
			if (!app.is_playing) ui_search();
		}
		if (ch == 'p' || ch == 'P') {
			/* Clear error when taking action */
			app.last_error[0] = '\0';
			
			if (app.is_playing) {
				app.is_paused = !app.is_paused;
				SDL_PauseAudioDevice(app.dev, app.is_paused);
			} else if (strlen(app.track_url) > 0) {
				pthread_create(&t_worker, NULL, thread_play, NULL);
			}
		}
		if (ch == 'x' || ch == 'X') {
			if (app.is_playing) {
				app.stop_flag = true;
				pthread_join(t_worker, NULL);
			}
		}
		if (ch == 'd' || ch == 'D') {
			app.last_error[0] = '\0';
			if (strlen(app.track_url) > 0 && !app.is_downloading) {
				pthread_t t_dl;
				pthread_create(&t_dl, NULL, thread_download, NULL);
				pthread_detach(t_dl);
			}
		}
	}

	endwin();
	SDL_CloseAudioDevice(app.dev);
	SDL_Quit();
	if (logfile) fclose(logfile);
	return 0;
}