/* SPDX-License-Identifier: GPL-2.0-only */

#import <Foundation/Foundation.h>
#import <vmnet/vmnet.h>
#include <stdint.h>
#include <sys/types.h>
#include <sys/uio.h>
#include <unistd.h>
#include <pthread.h>

#define MAC_STR_LENGTH 17

NSMutableDictionary* start_semaphores  = nil;
NSMutableDictionary* finish_semaphores = nil;
pthread_rwlock_t lock = PTHREAD_RWLOCK_INITIALIZER;

vmnet_return_t
vmnet_read_blocking(interface_ref interface, struct vmpktdesc *packets, int *pktcnt) {
    @autoreleasepool {
        dispatch_semaphore_t start;
        dispatch_semaphore_t finish;
        
        pthread_rwlock_rdlock(&lock);
        NSString* ref_key = [NSString stringWithFormat:@"%p", interface];
        start = [start_semaphores objectForKey:ref_key];
        finish = [finish_semaphores objectForKey:ref_key];
        pthread_rwlock_unlock(&lock);
        
        dispatch_wait(start, DISPATCH_TIME_FOREVER);
        
        vmnet_return_t ret = vmnet_read(interface, packets, pktcnt);
        
        dispatch_semaphore_signal(finish);
        
        return ret;
    }
}


// This function is inspired by vmn_create() from
// https://github.com/machyve/xhyve/blob/master/src/pci_virtio_net_vmnet.c
uint32_t create_interface(interface_ref* ref_p, char* mac, uint16_t* mtu) {
    @autoreleasepool {
        pthread_rwlock_wrlock(&lock);
        if (start_semaphores == nil) {
            start_semaphores = [[NSMutableDictionary alloc] init];        }
        if (finish_semaphores == nil) {
            finish_semaphores = [[NSMutableDictionary alloc] init];
        }
        pthread_rwlock_unlock(&lock);
        
        xpc_object_t desc = xpc_dictionary_create(NULL, NULL, 0);
        xpc_dictionary_set_uint64(desc, vmnet_operation_mode_key, VMNET_SHARED_MODE);
        
        dispatch_semaphore_t s = dispatch_semaphore_create(0);
        __block vmnet_return_t ret;
        interface_ref ref = vmnet_start_interface(desc, dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_HIGH, 0), ^(vmnet_return_t status, xpc_object_t  _Nullable interface_param) {
            ret = status;
            if (status == VMNET_SUCCESS) {
                memcpy(mac, xpc_dictionary_get_string(interface_param, vmnet_mac_address_key), MAC_STR_LENGTH);
                *mtu = (uint16_t)xpc_dictionary_get_uint64(interface_param, vmnet_mtu_key);
            }
            dispatch_semaphore_signal(s);
        });
        dispatch_semaphore_wait(s, DISPATCH_TIME_FOREVER);
        if (ref == NULL || ret != VMNET_SUCCESS) {
            return -1;
        }
        
        dispatch_semaphore_t start = dispatch_semaphore_create(0);
        dispatch_semaphore_t finish = dispatch_semaphore_create(0);
        
        pthread_rwlock_wrlock(&lock);
        NSString* ref_key = [NSString stringWithFormat:@"%p", ref];
        [start_semaphores setValue:start forKey:ref_key];
        [finish_semaphores setValue:finish forKey:ref_key];
        pthread_rwlock_unlock(&lock);
        
        vmnet_interface_set_event_callback(ref, VMNET_INTERFACE_PACKETS_AVAILABLE, dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^(__attribute__((unused)) interface_event_t event_mask, __attribute__((unused)) xpc_object_t  _Nonnull event) {
            dispatch_semaphore_signal(start);
            dispatch_semaphore_wait(finish, DISPATCH_TIME_FOREVER);
        });
        
        *ref_p = ref;
        return 0;
    }
}