
/* Crake's one change to openconnect on Windows (appended to main.c by
 * scripts/build-openconnect-windows.sh): stderr and stdout unbuffered.
 * Writing to a pipe makes them buffered there, so openconnect's questions
 * (trust this server? password?) stayed in its buffer and never reached
 * Crake's sign-in window, which waited for them. */
__attribute__((constructor)) static void crake_unbuffered(void)
{
	setvbuf(stderr, NULL, _IONBF, 0);
	setvbuf(stdout, NULL, _IONBF, 0);
}
