#include "dns.h"
#include <stdio.h>
#include <signal.h>

int main() {
    signal(SIGINT, SIG_IGN);
    signal(SIGHUP, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGQUIT, SIG_IGN);

    unsigned char hostname[] = "example.com";
    unsigned char **txt_results;
    unsigned short int *a_results;
    int i;

    txt_results = query_txt(hostname);

    if(txt_results) {
        printf("TXT records for %s:\n\n", hostname);

        for(i = 0; txt_results[i] != NULL; i++) {
            printf("TXT %d\n", i);
            printf("Data: %s\n", txt_results[i]);

            //Free each string
            free(txt_results[i]);
        }

        //Free the array itself
        free(txt_results);
    } else {
        printf("Failed to query TXT record for %s\n", hostname);
    }

    a_results = query_a(hostname);

    if(a_results) {
        printf("A records for %s:\n", hostname);
        printf("%d.%d.%d.%d\n", a_results[0], a_results[1], a_results[2], a_results[3]);

        //Free the array itself
        free(a_results);
    } else {
        printf("Failed to query A record for %s\n", hostname);
    }

    while(1) {
        sleep(1);
    };

    return 0;
}