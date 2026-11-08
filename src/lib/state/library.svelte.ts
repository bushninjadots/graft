import {
  api,
  type Collection,
  type CollectionDetail,
  type CollectionRepo,
} from "../api";
import { notify } from "./ui.svelte";

type LibraryState = {
  collections: Collection[];
  loading: boolean;
  error: string | null;
};

export const library = $state<LibraryState>({
  collections: [],
  loading: false,
  error: null,
});

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

export async function loadCollections(force = false): Promise<void> {
  if (library.loading || (library.collections.length > 0 && !force)) return;
  library.loading = true;
  try {
    library.collections = await api.collections();
    library.error = null;
  } catch (cause) {
    library.error = message(cause);
  } finally {
    library.loading = false;
  }
}

export async function createCollection(
  name: string,
  description: string,
): Promise<Collection | null> {
  try {
    const created = await api.createCollection(name, description);
    await loadCollections(true);
    notify(`Created “${created.name}”`, "ok");
    return created;
  } catch (cause) {
    notify(message(cause), "danger");
    return null;
  }
}

export async function deleteCollection(collection: Collection): Promise<boolean> {
  try {
    await api.deleteCollection(collection.id);
    await loadCollections(true);
    notify(`Deleted “${collection.name}”`, "ok");
    return true;
  } catch (cause) {
    notify(message(cause), "danger");
    return false;
  }
}

export async function addRepoToCollection(
  collectionId: number,
  owner: string,
  repo: string,
): Promise<CollectionRepo | null> {
  try {
    const saved = await api.addCollectionRepo(collectionId, owner, repo);
    await loadCollections(true);
    notify(`${saved.fullName} added to the collection`, "ok");
    return saved;
  } catch (cause) {
    notify(message(cause), "danger");
    return null;
  }
}

export async function loadCollection(id: number): Promise<CollectionDetail | null> {
  try {
    return await api.collectionDetail(id);
  } catch (cause) {
    notify(message(cause), "danger");
    return null;
  }
}

export async function updateCollectionRepo(
  repo: CollectionRepo,
  status: string,
  notes: string,
): Promise<CollectionRepo | null> {
  try {
    const saved = await api.updateCollectionRepo(repo.id, status, notes);
    notify(`Updated ${saved.fullName}`, "ok");
    return saved;
  } catch (cause) {
    notify(message(cause), "danger");
    return null;
  }
}

export async function removeCollectionRepo(repo: CollectionRepo): Promise<boolean> {
  try {
    await api.removeCollectionRepo(repo.id);
    notify(`Removed ${repo.fullName} from the collection`, "ok");
    return true;
  } catch (cause) {
    notify(message(cause), "danger");
    return false;
  }
}
