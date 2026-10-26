<script setup lang="ts">
import { computed } from "vue";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useAuthStore } from "../../store/auth";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const auth = useAuthStore();

const name = computed(() => auth.user?.nickname || auth.user?.username || "");
const roles = computed(() => auth.user?.roles ?? []);
</script>

<template>
  <section class="ui-profile">
    <h1 class="ui-profile__title">{{ t("system.profile.title") }}</h1>
    <Card>
      <dl class="ui-profile__list">
        <div class="ui-profile__row">
          <dt>{{ t("system.common.nickname") }}</dt>
          <dd>{{ name }}</dd>
        </div>
        <div class="ui-profile__row">
          <dt>{{ t("system.common.email") }}</dt>
          <dd>{{ auth.user?.email || "—" }}</dd>
        </div>
        <div class="ui-profile__row">
          <dt>{{ t("system.profile.roles") }}</dt>
          <dd class="ui-profile__roles">
            <Badge v-for="role in roles" :key="role" variant="accent">{{
              role
            }}</Badge>
          </dd>
        </div>
      </dl>
    </Card>
  </section>
</template>

<style scoped>
.ui-profile {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-profile__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

.ui-profile__list {
  margin: 0;
  display: flex;
  flex-direction: column;
}

.ui-profile__row {
  display: flex;
  align-items: center;
  gap: var(--space-6);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.ui-profile__row dt {
  width: 96px;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.ui-profile__row dd {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ui-profile__roles {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
</style>
