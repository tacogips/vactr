#include "bindings/bindings.h"
#include <AVFoundation/AVFoundation.h>
#include <stdint.h>
#include <stdio.h>

extern "C" void vactr_audio_session_event(uint32_t kind);

static void logAudioSessionError(NSError *error) {
	if (error != nil) fprintf(stderr, "AVAudioSession: %s\n", error.localizedDescription.UTF8String);
}

static void configureAudioSession(void) {
	AVAudioSession *session = [AVAudioSession sharedInstance];
	NSError *error = nil;
	[session setCategory:AVAudioSessionCategoryPlayback error:&error];
	logAudioSessionError(error);
	error = nil;
	[session setActive:YES error:&error];
	logAudioSessionError(error);

	NSNotificationCenter *center = [NSNotificationCenter defaultCenter];
	[center addObserverForName:AVAudioSessionInterruptionNotification object:session queue:nil usingBlock:^(NSNotification *note) {
		const NSUInteger type = [note.userInfo[AVAudioSessionInterruptionTypeKey] unsignedIntegerValue];
		if (type == AVAudioSessionInterruptionTypeBegan) {
			vactr_audio_session_event(1);
		} else if (type == AVAudioSessionInterruptionTypeEnded) {
			const NSUInteger options = [note.userInfo[AVAudioSessionInterruptionOptionKey] unsignedIntegerValue];
			if ((options & AVAudioSessionInterruptionOptionShouldResume) != 0) {
				NSError *resumeError = nil;
				[[AVAudioSession sharedInstance] setActive:YES error:&resumeError];
				logAudioSessionError(resumeError);
				vactr_audio_session_event(2);
			}
		}
	}];
	[center addObserverForName:AVAudioSessionRouteChangeNotification object:session queue:nil usingBlock:^(__unused NSNotification *note) {
		vactr_audio_session_event(3);
	}];
}

int main(int argc, char * argv[]) {
	@autoreleasepool {
		configureAudioSession();
	ffi::start_app();
	}
	return 0;
}
