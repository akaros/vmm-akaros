#include <stdint.h>

struct PML4E {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t resv1 : 6;
  uint64_t pdpt_base : 40;
  uint64_t resv2 : 11;
  uint64_t xd : 1;
};

struct PDPTE_1GB {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t dirty : 1;
  uint64_t ps : 1;
  uint64_t global : 1;
  uint64_t ign1 : 3;
  uint64_t pat : 1;
  uint64_t resv : 17;
  uint64_t pg_base : 22;
  uint64_t ign2 : 7;
  uint64_t prot_key : 4;
  uint64_t xd : 1;
};

struct PDPTE {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t dirty : 1;
  uint64_t ps : 1;
  uint64_t global : 1;
  uint64_t ign1 : 3;
  uint64_t pd_base : 40;
  uint64_t ign2 : 11;
  uint64_t xd : 1;
};

struct PDE_2MB {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t dirty : 1;
  uint64_t ps : 1;
  uint64_t global : 1;
  uint64_t ign1 : 3;
  uint64_t pat : 1;
  uint64_t resv2 : 8;
  uint64_t pg_base : 31;
  uint64_t ign2 : 7;
  uint64_t prot_key : 4;
  uint64_t xd : 1;
};

struct PDE {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t dirty : 1;
  uint64_t ps : 1;
  uint64_t global : 1;
  uint64_t ign1 : 3;
  uint64_t pt_base : 40;
  uint64_t ign2 : 11;
  uint64_t xd : 1;
};

struct PTE {
  uint64_t pres : 1;  // present
  uint64_t rw : 1;    // writable?
  uint64_t us : 1;    // use level protection
  uint64_t pwt : 1;   //
  uint64_t pcd : 1;
  uint64_t a : 1;
  uint64_t dirty : 1;
  uint64_t pat : 1;
  uint64_t global : 1;
  uint64_t ign1 : 3;
  uint64_t pg_base : 40;
  uint64_t ign2 : 11;
  uint64_t xd : 1;
};
