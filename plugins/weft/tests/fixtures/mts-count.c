#include <stdbool.h>
static _Thread_local int clients;
void MTS_RegisterClient(void) { ++clients; }
void MTS_DeregisterClient(void) { --clients; }
int MTS_GetNumClients(void) { return clients; }
bool MTS_HasMaster(void) { return false; }
