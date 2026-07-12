import { defineStore } from "pinia";
import { ref } from "vue";
import {
  type Project,
  type Requirement,
  type UserStory,
  type AcceptanceCriteria,
  getProject,
  getRequirement,
  listStories,
  listCriteria
} from "@/api/pm";
import {
  type KbDocument,
  type KbDocumentVersion,
  getDocument,
  listVersions
} from "@/api/kb";

export const usePmStore = defineStore("pm", () => {
  // current selection
  const currentProject = ref<Project | null>(null);
  const currentRequirement = ref<Requirement | null>(null);
  const currentDocument = ref<KbDocument | null>(null);

  const stories = ref<UserStory[]>([]);
  const criteria = ref<AcceptanceCriteria[]>([]);
  const versions = ref<KbDocumentVersion[]>([]);

  async function loadProject(id: number) {
    const { data } = await getProject(id);
    currentProject.value = data;
    return data;
  }

  async function loadRequirement(id: number) {
    const { data } = await getRequirement(id);
    currentRequirement.value = data;
    stories.value = [];
    criteria.value = [];
    return data;
  }

  async function loadStories(requirementId: number) {
    const { data } = await listStories(requirementId);
    stories.value = data;
    return data;
  }

  async function loadCriteria(storyId: number) {
    const { data } = await listCriteria(storyId);
    criteria.value = data;
    return data;
  }

  async function loadDocument(id: number) {
    const { data } = await getDocument(id);
    currentDocument.value = data;
    versions.value = [];
    return data;
  }

  async function loadVersions(documentId: number) {
    const { data } = await listVersions(documentId);
    versions.value = data;
    return data;
  }

  function clearAll() {
    currentProject.value = null;
    currentRequirement.value = null;
    currentDocument.value = null;
    stories.value = [];
    criteria.value = [];
    versions.value = [];
  }

  return {
    currentProject,
    currentRequirement,
    currentDocument,
    stories,
    criteria,
    versions,
    loadProject,
    loadRequirement,
    loadStories,
    loadCriteria,
    loadDocument,
    loadVersions,
    clearAll
  };
});
