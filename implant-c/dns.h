//Derived from https://www.binarytides.com/dns-query-code-in-c-with-linux-sockets/
//Author : Silver Moon (m00n.silv3r@gmail.com)
//Dated : 29/4/2009
//Updated by: a-rvid (git@rvid.eu)
 
//Header Files
#include<string.h>    //strlen
#include<stdlib.h>    //malloc
#include<sys/socket.h>    //you know what this is for
#include<arpa/inet.h> //inet_addr , inet_ntoa , ntohs etc
#include<netinet/in.h>
#include<unistd.h>    //getpid

//Default DNS server addresses (overridden by compile-time -D flags)
#ifndef DNS_SERVER_1_ADDR
#define DNS_SERVER_1_ADDR 0x08080808  //8.8.8.8
#endif
#ifndef DNS_SERVER_2_ADDR
#define DNS_SERVER_2_ADDR 0x08080404  //8.8.4.4
#endif
 
//Types of DNS resource records :)
 
#define T_A 1 //Ipv4 address
#define T_TXT 16 // TXT
// #define T_NS 2 //Nameserver
// #define T_CNAME 5 // canonical name
// #define T_SOA 6 /* start of authority zone */
// #define T_PTR 12 /* domain name pointer */
// #define T_MX 15 //Mail server
 
//Function Prototypes
void ngethostbyname (unsigned char* , int);
void ChangetoDnsNameFormat (unsigned char*,unsigned char*);
unsigned char* ReadName (unsigned char*,unsigned char*,int*);
void get_dns_servers();
 
//DNS header structure
struct DNS_HEADER
{
    unsigned short id; // identification number
 
    unsigned char rd :1; // recursion desired
    unsigned char tc :1; // truncated message
    unsigned char aa :1; // authoritive answer
    unsigned char opcode :4; // purpose of message
    unsigned char qr :1; // query/response flag
 
    unsigned char rcode :4; // response code
    unsigned char cd :1; // checking disabled
    unsigned char ad :1; // authenticated data
    unsigned char z :1; // its z! reserved
    unsigned char ra :1; // recursion available
 
    unsigned short q_count; // number of question entries
    unsigned short ans_count; // number of answer entries
    unsigned short auth_count; // number of authority entries
    unsigned short add_count; // number of resource entries
};
 
//Constant sized fields of query structure
struct QUESTION
{
    unsigned short qtype;
    unsigned short qclass;
};
 
//Constant sized fields of the resource record structure
#pragma pack(push, 1)
struct R_DATA
{
    unsigned short type;
    unsigned short _class;
    unsigned int ttl;
    unsigned short data_len;
};
#pragma pack(pop)
 
//Pointers to resource record contents
struct RES_RECORD
{
    unsigned char *name;
    struct R_DATA *resource;
    unsigned char *rdata;
};
 
//Structure of a Query
typedef struct
{
    unsigned char *name;
    struct QUESTION *ques;
} QUERY;
 
/*
 * Perform a DNS query by sending a packet for TXT records
 * Returns dynamically allocated 2D array of TXT record strings
 * Last element is NULL to mark end of array
 * */
unsigned char** query_txt(unsigned char *host) {
    unsigned char buf[65536], *qname, *reader;
    unsigned char **txt_array;
    int i, j, stop, s, txt_count = 0;
    struct sockaddr_in dest;
    struct DNS_HEADER *dns = NULL;
    struct QUESTION *qinfo = NULL;
    struct RES_RECORD answers[20];

    s = socket(AF_INET , SOCK_DGRAM , IPPROTO_UDP); //UDP packet for DNS queries

    if(s < 0) return NULL;

    dest.sin_family = AF_INET;
    dest.sin_port = htons(53);
    dest.sin_addr.s_addr = DNS_SERVER_1_ADDR;

    //Set the DNS structure to standard queries
    dns = (struct DNS_HEADER *)&buf;

    dns->id = (unsigned short) htons(getpid());
    dns->qr = 0; //This is a query
    dns->opcode = 0; //This is a standard query
    dns->aa = 0; //Not Authoritative
    dns->tc = 0; //This message is not truncated
    dns->rd = 1; //Recursion Desired
    dns->ra = 0; //Recursion not available! hey we dont have it (lol)
    dns->z = 0;
    dns->ad = 0;
    dns->cd = 0;
    dns->rcode = 0;
    dns->q_count = htons(1); //we have only 1 question
    dns->ans_count = 0;
    dns->auth_count = 0;
    dns->add_count = 0;

    //point to the query portion
    qname = (unsigned char*)&buf[sizeof(struct DNS_HEADER)];

    ChangetoDnsNameFormat(qname , host);
    qinfo = (struct QUESTION*)&buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname) + 1)];

    qinfo->qtype = htons(T_TXT); //query for TXT record
    qinfo->qclass = htons(1); //internet

    if(sendto(s, (char*)buf, sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION), 0, (struct sockaddr*)&dest, sizeof(dest)) < 0) {
        close(s);
        return NULL;
    }

    //Receive the answer
    i = sizeof dest;
    if(recvfrom(s, (char*)buf, 65536, 0, (struct sockaddr*)&dest, (socklen_t*)&i) < 0) {
        close(s);
        return NULL;
    }

    dns = (struct DNS_HEADER*) buf;

    //move ahead of the dns header and the query field
    reader = &buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION)];

    //Allocate 2D array for TXT records (21 pointers: 20 records + 1 null terminator)
    txt_array = (unsigned char**)malloc(21 * sizeof(unsigned char*));
    if(!txt_array) {
        close(s);
        return NULL;
    }

    //Start reading answers
    stop = 0;
    for(i = 0; i < ntohs(dns->ans_count) && txt_count < 20; i++) {
        answers[i].name = ReadName(reader, buf, &stop);
        reader = reader + stop;

        answers[i].resource = (struct R_DATA*)(reader);
        reader = reader + sizeof(struct R_DATA);

        if(ntohs(answers[i].resource->type) == T_TXT) {
            unsigned int len = ntohs(answers[i].resource->data_len);
            //TXT records have length-prefixed strings
            if(len > 0) {
                unsigned char str_len = reader[0]; //first byte is string length
                txt_array[txt_count] = (unsigned char*)malloc(str_len + 1);

                if(txt_array[txt_count]) {
                    for(j = 0; j < str_len && j < (int)len - 1; j++) {
                        txt_array[txt_count][j] = reader[j + 1];
                    }
                    txt_array[txt_count][j] = '\0';
                    txt_count++;
                }
            }
            reader = reader + len;
        } else {
            reader = reader + ntohs(answers[i].resource->data_len);
        }
    }

    //Null terminate the array
    txt_array[txt_count] = NULL;

    close(s);
    return txt_array;
}

unsigned short int* query_a(unsigned char *host) {
    unsigned short int* octets = (unsigned short int*)malloc(4 * sizeof(unsigned short int));
    unsigned char buf[65536], *qname, *reader;
    int i, stop, s;

    struct RES_RECORD answers[20]; //the replies from the DNS server
    struct sockaddr_in dest;

    struct DNS_HEADER *dns = NULL;
    struct QUESTION *qinfo = NULL;

    s = socket(AF_INET , SOCK_DGRAM , IPPROTO_UDP); //UDP packet for DNS queries

    if(s < 0) return NULL;

    dest.sin_family = AF_INET;
    dest.sin_port = htons(53);
    dest.sin_addr.s_addr = DNS_SERVER_1_ADDR; //dns servers
 
    //Set the DNS structure to standard queries
    dns = (struct DNS_HEADER *)&buf;
 
    dns->id = (unsigned short) htons(getpid());
    dns->qr = 0; //This is a query
    dns->opcode = 0; //This is a standard query
    dns->aa = 0; //Not Authoritative
    dns->tc = 0; //This message is not truncated
    dns->rd = 1; //Recursion Desired
    dns->ra = 0; //Recursion not available! hey we dont have it (lol)
    dns->z = 0;
    dns->ad = 0;
    dns->cd = 0;
    dns->rcode = 0;
    dns->q_count = htons(1); //we have only 1 question
    dns->ans_count = 0;
    dns->auth_count = 0;
    dns->add_count = 0;
 
    //point to the query portion
    qname = (unsigned char*)&buf[sizeof(struct DNS_HEADER)];

    ChangetoDnsNameFormat(qname , host);
    qinfo = (struct QUESTION*)&buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname) + 1)];

    qinfo->qtype = htons(T_A); //query for A record
    qinfo->qclass = htons(1); //internet

    if(sendto(s, (char*)buf, sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION), 0, (struct sockaddr*)&dest, sizeof(dest)) < 0) {
        close(s);
        return NULL;
    }

    //Receive the answer
    i = sizeof dest;
    if(recvfrom(s, (char*)buf, 65536, 0, (struct sockaddr*)&dest, (socklen_t*)&i) < 0) {
        close(s);
        return NULL;
    }

    dns = (struct DNS_HEADER*) buf;

    //move ahead of the dns header and the query field
    reader = &buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION)];

    //Initialize octets array
    octets[0] = 0;
    octets[1] = 0;
    octets[2] = 0;
    octets[3] = 0;

    //Start reading answers
    stop = 0;
    for(i = 0; i < ntohs(dns->ans_count); i++) {
        answers[i].name = ReadName(reader, buf, &stop);
        reader = reader + stop;

        answers[i].resource = (struct R_DATA*)(reader);
        reader = reader + sizeof(struct R_DATA);

        if(ntohs(answers[i].resource->type) == T_A) {
            unsigned int len = ntohs(answers[i].resource->data_len);
            //A records contain 4 bytes for IPv4 address
            if(len == 4) {
                octets[0] = (unsigned short int)reader[0];
                octets[1] = (unsigned short int)reader[1];
                octets[2] = (unsigned short int)reader[2];
                octets[3] = (unsigned short int)reader[3];
            }
            reader = reader + len;
        } else {
            reader = reader + ntohs(answers[i].resource->data_len);
        }
    }

    close(s);
    return octets;
}

/*
 * Get the DNS servers from /etc/resolv.conf file on Linux
 * */
 

// void ngethostbyname(unsigned char *host , int query_type)
// {
//     unsigned char buf[65536],*qname,*reader;
//     int i , j , stop , s;

//     struct sockaddr_in a;

//     struct RES_RECORD answers[20],auth[20],addit[20]; //the replies from the DNS server
//     struct sockaddr_in dest;

//     struct DNS_HEADER *dns = NULL;
//     struct QUESTION *qinfo = NULL;

//     s = socket(AF_INET , SOCK_DGRAM , IPPROTO_UDP); //UDP packet for DNS queries

//     dest.sin_family = AF_INET;
//     dest.sin_port = htons(53);
//     dest.sin_addr.s_addr = DNS_SERVER_1_ADDR; //dns servers
 
//     //Set the DNS structure to standard queries
//     dns = (struct DNS_HEADER *)&buf;
 
//     dns->id = (unsigned short) htons(getpid());
//     dns->qr = 0; //This is a query
//     dns->opcode = 0; //This is a standard query
//     dns->aa = 0; //Not Authoritative
//     dns->tc = 0; //This message is not truncated
//     dns->rd = 1; //Recursion Desired
//     dns->ra = 0; //Recursion not available! hey we dont have it (lol)
//     dns->z = 0;
//     dns->ad = 0;
//     dns->cd = 0;
//     dns->rcode = 0;
//     dns->q_count = htons(1); //we have only 1 question
//     dns->ans_count = 0;
//     dns->auth_count = 0;
//     dns->add_count = 0;
 
//     //point to the query portion
//     qname =(unsigned char*)&buf[sizeof(struct DNS_HEADER)];
 
//     ChangetoDnsNameFormat(qname , host);
//     qinfo =(struct QUESTION*)&buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname) + 1)]; //fill it
 
//     qinfo->qtype = htons( query_type ); //type of the query , A , MX , CNAME , NS etc
//     qinfo->qclass = htons(1); //its internet (lol)
 
//     if( sendto(s,(char*)buf,sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION),0,(struct sockaddr*)&dest,sizeof(dest)) < 0)
//     {
//         // Error
//     }
     
//     //Receive the answer
//     i = sizeof dest;
//     // printf("\nReceiving answer...");
//     if(recvfrom (s,(char*)buf , 65536 , 0 , (struct sockaddr*)&dest , (socklen_t*)&i ) < 0)
//     {
//     }
//     // printf("Done");
 
//     dns = (struct DNS_HEADER*) buf;
 
//     //move ahead of the dns header and the query field
//     reader = &buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION)];
 
//     // printf("\nThe response contains : ");
//     // printf("\n %d Questions.",ntohs(dns->q_count));
//     // printf("\n %d Answers.",ntohs(dns->ans_count));
//     // printf("\n %d Authoritative Servers.",ntohs(dns->auth_count));
//     // printf("\n %d Additional records.\n\n",ntohs(dns->add_count));
 
//     //Start reading answers
//     stop=0;
 
//     for(i=0;i<ntohs(dns->ans_count);i++)
//     {
//         answers[i].name=ReadName(reader,buf,&stop);
//         reader = reader + stop;
 
//         answers[i].resource = (struct R_DATA*)(reader);
//         reader = reader + sizeof(struct R_DATA);
 
//         if(ntohs(answers[i].resource->type) == 1) //if its an ipv4 address
//         {
//             answers[i].rdata = (unsigned char*)malloc(ntohs(answers[i].resource->data_len));
 
//             for(j=0 ; j<ntohs(answers[i].resource->data_len) ; j++)
//             {
//                 answers[i].rdata[j]=reader[j];
//             }
 
//             answers[i].rdata[ntohs(answers[i].resource->data_len)] = '\0';
 
//             reader = reader + ntohs(answers[i].resource->data_len);
//         }
//         else
//         {
//             answers[i].rdata = ReadName(reader,buf,&stop);
//             reader = reader + stop;
//         }
//     }
 
//     //read authorities
//     for(i=0;i<ntohs(dns->auth_count);i++)
//     {
//         auth[i].name=ReadName(reader,buf,&stop);
//         reader+=stop;
 
//         auth[i].resource=(struct R_DATA*)(reader);
//         reader+=sizeof(struct R_DATA);
 
//         auth[i].rdata=ReadName(reader,buf,&stop);
//         reader+=stop;
//     }
 
//     //read additional
//     for(i=0;i<ntohs(dns->add_count);i++)
//     {
//         addit[i].name=ReadName(reader,buf,&stop);
//         reader+=stop;
 
//         addit[i].resource=(struct R_DATA*)(reader);
//         reader+=sizeof(struct R_DATA);
 
//         if(ntohs(addit[i].resource->type)==1)
//         {
//             addit[i].rdata = (unsigned char*)malloc(ntohs(addit[i].resource->data_len));
//             for(j=0;j<ntohs(addit[i].resource->data_len);j++)
//             addit[i].rdata[j]=reader[j];
 
//             addit[i].rdata[ntohs(addit[i].resource->data_len)]='\0';
//             reader+=ntohs(addit[i].resource->data_len);
//         }
//         else
//         {
//             addit[i].rdata=ReadName(reader,buf,&stop);
//             reader+=stop;
//         }
//     }
 
//     //print answers
//     // printf("\nAnswer Records : %d \n" , ntohs(dns->ans_count) );
//     for(i=0 ; i < ntohs(dns->ans_count) ; i++)
//     {
//         // printf("Name : %s ",answers[i].name);
 
//         if( ntohs(answers[i].resource->type) == T_A) //IPv4 address
//         {
//             long *p;
//             p=(long*)answers[i].rdata;
//             a.sin_addr.s_addr=(*p); //working without ntohl
//             // printf("has IPv4 address : %s",inet_ntoa(a.sin_addr));
//         }
         
//         if(ntohs(answers[i].resource->type)==5) 
//         {
//             //Canonical name for an alias
//             // printf("has alias name : %s",answers[i].rdata);
//         }
 
//         // printf("\n");
//     }
 
//     //print authorities
//     // printf("\nAuthoritive Records : %d \n" , ntohs(dns->auth_count) );
//     for( i=0 ; i < ntohs(dns->auth_count) ; i++)
//     {
         
//         // printf("Name : %s ",auth[i].name);
//         if(ntohs(auth[i].resource->type)==2)
//         {
//             // printf("has nameserver : %s",auth[i].rdata);
//         }
//         // printf("\n");
//     }
 
//     //print additional resource records
//     // printf("\nAdditional Records : %d \n" , ntohs(dns->add_count) );
//     for(i=0; i < ntohs(dns->add_count) ; i++)
//     {
//         // printf("Name : %s ",addit[i].name);
//         if(ntohs(addit[i].resource->type)==1)
//         {
//             long *p;
//             p=(long*)addit[i].rdata;
//             a.sin_addr.s_addr=(*p);
//             // printf("has IPv4 address : %s",inet_ntoa(a.sin_addr));
//         }
//         // printf("\n");
//     }
//     (void)a;
//     return;
// }
 
/*
 *
 * */
unsigned char* ReadName(unsigned char* reader,unsigned char* buffer,int* count)
{
    unsigned char *name;
    unsigned int p=0,jumped=0,offset;
    int i , j;
 
    *count = 1;
    name = (unsigned char*)malloc(256);
 
    name[0]='\0';
 
    //read the names in 3www6google3com format
    while(*reader!=0)
    {
        if(*reader>=192)
        {
            offset = (*reader)*256 + *(reader+1) - 49152; //49152 = 11000000 00000000 ;)
            reader = buffer + offset - 1;
            jumped = 1; //we have jumped to another location so counting wont go up!
        }
        else
        {
            name[p++]=*reader;
        }
 
        reader = reader+1;
 
        if(jumped==0)
        {
            *count = *count + 1; //if we havent jumped to another location then we can count up
        }
    }
 
    name[p]='\0'; //string complete
    if(jumped==1)
    {
        *count = *count + 1; //number of steps we actually moved forward in the packet
    }
 
    //now convert 3www6google3com0 to www.google.com
    for(i=0;i<(int)strlen((const char*)name);i++) 
    {
        p=name[i];
        for(j=0;j<(int)p;j++) 
        {
            name[i]=name[i+1];
            i=i+1;
        }
        name[i]='.';
    }
    name[i-1]='\0'; //remove the last dot
    return name;
}
 
/*
 * This will convert www.google.com to 3www6google3com 
 * got it :)
 * */
void ChangetoDnsNameFormat(unsigned char* dns,unsigned char* host) 
{
    int lock = 0 , i;
    strcat((char*)host,".");
     
    for(i = 0 ; i < strlen((char*)host) ; i++) 
    {
        if(host[i]=='.') 
        {
            *dns++ = i-lock;
            for(;lock<i;lock++) 
            {
                *dns++=host[lock];
            }
            lock++; //or lock=i+1;
        }
    }
    *dns++='\0';
}