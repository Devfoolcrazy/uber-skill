<script lang="ts">
  import { page } from "$app/state";

  // Nothing in the app navigates; landing here means a link slipped through.
  // A full reload of the root restores the main screen whatever happened.
</script>

<main>
  <h1>Page introuvable</h1>
  <p class="muted selectable">{page.status} · {page.url.pathname}</p>
  <p>Ce lien ne mène à aucun écran de l’application.</p>
  <a class="button" href="/" data-sveltekit-reload>Revenir à la bibliothèque</a>
</main>

<style>
  main { height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; text-align: center; }
  h1 { margin: 0; font-size: 20px; }
  p { margin: 0; }
  .button { margin-top: 12px; padding: 8px 14px; border-radius: var(--radius); background: var(--accent); color: #fff; text-decoration: none; }
</style>
