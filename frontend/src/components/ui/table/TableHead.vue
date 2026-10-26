<template>
  <thead class="ui-table__head">
    <slot />
  </thead>
</template>

<style scoped>
/* Phase 1R §6.7：表头沉在 sunken 灰带 */
.ui-table__head {
  background: var(--surface-sunken);
}
</style>
