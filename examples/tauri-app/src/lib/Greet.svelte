<script>
  import { invoke } from "@tauri-apps/api/core"

  let name = $state("");
  let greetMsg = $state("")

  async function greet(){
    // Learn more about Tauri commands at https://v2.tauri.app/develop/calling-rust/#commands
    greetMsg = await invoke("greet", { name })
  }
</script>

<div class="greet">
  <div class="greet-row">
    <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
    <button onclick={greet}>Greet</button>
  </div>
  <p>{greetMsg}</p>
</div>

<style>
  .greet-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  #greet-input {
    min-width: min(300px, 100%);
  }

  p {
    margin: 8px 0 0;
    color: #d0daf9;
  }
</style>
